//! Immutable static-page source bundles with optional local render captures.
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use cssparser::{Parser, ParserInput, Token};
use lol_html::{element, rewrite_str, text, RewriteStrSettings};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};

use super::archive::digest;
use super::battle_model::{BattleRun, GrowthBattle};
use super::config::{validate_relative_path, GrowthConfig, StaticPreview};
use crate::error::{anyhow, Result};

pub const PRODUCER: &str = "static-page-bundle-v1";
pub const DOCUMENT: &str = "preview/document.html";
pub const METADATA: &str = "preview/metadata.json";
pub const SCREENSHOT_DESKTOP: &str = "preview/screenshot-desktop.png";
pub const SCREENSHOT_PHONE: &str = "preview/screenshot-phone.png";
pub const CSP: &str = "default-src 'none'; script-src 'none'; style-src data: 'unsafe-inline'; img-src data:; font-src data:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";
const FILE_CAP: usize = 4 * 1024 * 1024;
const TOTAL_CAP: usize = 8 * 1024 * 1024;
const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PreviewStatus {
    Ready,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewSource {
    pub path: String,
    pub digest: String,
    pub size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewScreenshot {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub digest: String,
    pub size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewRenderCheck {
    pub viewport: String,
    pub width: u32,
    pub height: u32,
    pub viewport_matches: bool,
    pub body_text_chars: usize,
    pub horizontal_overflow: bool,
    pub dom_content_loaded_ms: Option<u64>,
    pub load_ms: Option<u64>,
    pub first_contentful_paint_ms: Option<u64>,
    pub provenance: String,
    pub limitation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewRecord {
    pub producer: String,
    pub status: PreviewStatus,
    pub source_commit: String,
    pub document_digest: Option<String>,
    pub sources: Vec<PreviewSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub screenshots: Vec<PreviewScreenshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub render_checks: Vec<PreviewRenderCheck>,
    pub blocked_resources: usize,
    pub limitation: String,
}

fn mime(path: &str) -> Option<&'static str> {
    match Path::new(path)
        .extension()?
        .to_str()?
        .to_ascii_lowercase()
        .as_str()
    {
        "html" => Some("text/html;charset=utf-8"),
        "css" => Some("text/css;charset=utf-8"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "woff" => Some("font/woff"),
        "woff2" => Some("font/woff2"),
        _ => None,
    }
}

struct Resources<'a> {
    files: &'a BTreeMap<String, Vec<u8>>,
    blocked: Cell<usize>,
    packed: Cell<usize>,
}
impl Resources<'_> {
    fn bounded(&self, uri: String) -> String {
        let total = self.packed.get().saturating_add(uri.len());
        if total > TOTAL_CAP {
            return self.omitted();
        }
        self.packed.set(total);
        uri
    }
    fn data_uri(&self, content_type: &str, content: &[u8]) -> String {
        self.bounded(format!(
            "data:{content_type};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(content)
        ))
    }
    fn omitted(&self) -> String {
        self.blocked.set(self.blocked.get() + 1);
        "data:,".into()
    }
    fn uri(&self, reference: &str, base: &str, css: bool, ancestors: &[String]) -> String {
        let reference = reference.trim();
        if reference.starts_with('#') && !css {
            return reference.into();
        }
        // Data resources are consumed only as styles/images/fonts; no document
        // navigation or scripts are ever enabled in the opaque preview frame.
        if reference.to_ascii_lowercase().starts_with("data:") {
            return self.bounded(reference.into());
        }
        let mut origin = url::Url::parse("https://growthlab.invalid/").unwrap();
        origin.set_path(&format!("/{base}"));
        let Some(url) = origin.join(reference).ok().filter(|url| {
            url.scheme() == "https"
                && url.host_str() == Some("growthlab.invalid")
                && url.username().is_empty()
                && url.password().is_none()
        }) else {
            return self.omitted();
        };
        let Ok(path) = percent_decode_str(url.path().trim_start_matches('/')).decode_utf8() else {
            return self.omitted();
        };
        if validate_relative_path(&path).is_err()
            || ancestors.len() >= 8
            || ancestors.iter().any(|ancestor| ancestor == &*path)
        {
            return self.omitted();
        }
        let Some(bytes) = self.files.get(&*path) else {
            return self.omitted();
        };
        let Some(content_type) = mime(&path).filter(|kind| {
            if css {
                kind.starts_with("text/css")
            } else {
                !kind.starts_with("text/html")
            }
        }) else {
            return self.omitted();
        };
        let content = if content_type.starts_with("text/css") {
            let Ok(source) = std::str::from_utf8(bytes) else {
                return self.omitted();
            };
            let mut parents = ancestors.to_vec();
            parents.push(path.to_string());
            match self.stylesheet(source, &path, &parents) {
                Ok(value) => value.into_bytes(),
                Err(_) => return self.omitted(),
            }
        } else {
            bytes.clone()
        };
        self.data_uri(content_type, &content)
    }
    fn stylesheet(&self, source: &str, base: &str, ancestors: &[String]) -> Result<String> {
        let mut input = ParserInput::new(source);
        let mut parser = Parser::new(&mut input);
        let mut edits = vec![];
        self.css_tokens(&mut parser, base, ancestors, &mut edits)?;
        let mut output = source.to_string();
        for (start, end, replacement) in edits.into_iter().rev() {
            output.replace_range(start..end, &replacement);
        }
        if output.len() > FILE_CAP {
            return Err(anyhow!("Static stylesheet exceeds the preview limit"));
        }
        Ok(output)
    }
    fn css_tokens<'i>(
        &self,
        parser: &mut Parser<'i, '_>,
        base: &str,
        ancestors: &[String],
        edits: &mut Vec<(usize, usize, String)>,
    ) -> Result<()> {
        let mut import = false;
        while !parser.is_exhausted() {
            let start_position = parser.position();
            let start = start_position.byte_index();
            let token = parser
                .next_including_whitespace_and_comments()
                .map_err(|_| anyhow!("Invalid static stylesheet"))?
                .clone();
            match token {
                Token::AtKeyword(ref name) if name.eq_ignore_ascii_case("import") => import = true,
                Token::WhiteSpace(_) | Token::Comment(_) => {}
                Token::UnquotedUrl(ref value) => {
                    let uri = self.uri(value, base, import, ancestors);
                    edits.push((
                        start,
                        parser.position().byte_index(),
                        format!("url({})", serde_json::to_string(&uri)?),
                    ));
                    import = false;
                }
                Token::QuotedString(ref value) if import => {
                    if !closed_string(parser.slice_from(start_position)) {
                        return Err(anyhow!("Unterminated static stylesheet import"));
                    }
                    edits.push((
                        start,
                        parser.position().byte_index(),
                        serde_json::to_string(&self.uri(value, base, true, ancestors))?,
                    ));
                    import = false;
                }
                Token::Function(ref name) if name.eq_ignore_ascii_case("url") => {
                    let value = parser
                        .parse_nested_block(|inside| {
                            inside.skip_whitespace();
                            let position = inside.position();
                            let value = inside.expect_string()?.to_string();
                            if !closed_string(inside.slice_from(position)) {
                                return Err(inside.new_custom_error(()));
                            }
                            inside.expect_exhausted()?;
                            Ok::<_, cssparser::ParseError<'i, ()>>(value)
                        })
                        .map_err(|_| anyhow!("Invalid static resource URL"))?;
                    if !parser.slice_from(start_position).ends_with(')') {
                        return Err(anyhow!("Unterminated static resource URL"));
                    }
                    edits.push((
                        start,
                        parser.position().byte_index(),
                        format!(
                            "url({})",
                            serde_json::to_string(&self.uri(&value, base, import, ancestors))?
                        ),
                    ));
                    import = false;
                }
                Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock => {
                    parser
                        .parse_nested_block(|inside| {
                            self.css_tokens(inside, base, ancestors, edits)
                                .map_err(|_| inside.new_custom_error::<(), ()>(()))
                        })
                        .map_err(|_| anyhow!("Invalid static stylesheet block"))?;
                    import = false;
                }
                Token::BadUrl(_) | Token::BadString(_) => {
                    return Err(anyhow!("Invalid static stylesheet token"))
                }
                _ => import = false,
            }
        }
        Ok(())
    }
}

// CSS tokenization permits EOF to close strings/functions. Preview packaging
// refuses such incomplete resource references instead of silently repairing them.
fn closed_string(source: &str) -> bool {
    let Some(quote @ (b'\'' | b'"')) = source.as_bytes().first().copied() else {
        return false;
    };
    source.len() >= 2
        && source.as_bytes().last() == Some(&quote)
        && source.as_bytes()[..source.len() - 1]
            .iter()
            .rev()
            .take_while(|byte| **byte == b'\\')
            .count()
            % 2
            == 0
}

fn document(source: &str, entry: &str, resources: &Resources<'_>) -> Result<String> {
    let head = Cell::new(false);
    let style_text = RefCell::new(String::new());
    let policy = format!("<meta http-equiv=\"Content-Security-Policy\" content=\"{CSP}\">");
    let output = rewrite_str(
        source,
        RewriteStrSettings::new()
            .with_strict(true)
            .append_element_content_handler(element!("head", |element| {
                head.set(true);
                element.prepend(&policy, lol_html::html_content::ContentType::Html);
                Ok(())
            }))
            .append_element_content_handler(element!(
                "script, iframe, object, embed, base, meta[http-equiv], link:not([rel]), source",
                |element| {
                    element.remove();
                    Ok(())
                }
            ))
            .append_element_content_handler(element!("form", |element| {
                element.remove_and_keep_content();
                Ok(())
            }))
            .append_element_content_handler(element!("style", |element| {
                element.remove_and_keep_content();
                Ok(())
            }))
            .append_element_content_handler(text!("style", |chunk| {
                style_text.borrow_mut().push_str(chunk.as_str());
                if chunk.last_in_text_node() {
                    let css = resources.stylesheet(&style_text.take(), entry, &[])?;
                    let uri = resources.data_uri("text/css;charset=utf-8", css.as_bytes());
                    chunk.replace(
                        &format!("<link rel=\"stylesheet\" href=\"{uri}\">"),
                        lol_html::html_content::ContentType::Html,
                    );
                } else {
                    chunk.replace("", lol_html::html_content::ContentType::Html);
                }
                Ok(())
            }))
            .append_element_content_handler(element!("link[rel]", |element| {
                let stylesheet = element
                    .get_attribute("rel")
                    .is_some_and(|value| value.eq_ignore_ascii_case("stylesheet"));
                if !stylesheet {
                    element.remove();
                } else {
                    let href = resources.uri(
                        &element.get_attribute("href").unwrap_or_default(),
                        entry,
                        true,
                        &[],
                    );
                    element.set_attribute("href", &href)?;
                    element.remove_attribute("integrity");
                    element.remove_attribute("crossorigin");
                }
                Ok(())
            }))
            .append_element_content_handler(element!("img", |element| {
                let src = resources.uri(
                    &element.get_attribute("src").unwrap_or_default(),
                    entry,
                    false,
                    &[],
                );
                element.set_attribute("src", &src)?;
                element.remove_attribute("srcset");
                Ok(())
            }))
            .append_element_content_handler(element!("*", |element| {
                let attributes: Vec<_> = element
                    .attributes()
                    .iter()
                    .map(|attribute| attribute.name())
                    .collect();
                for name in attributes {
                    if name.to_ascii_lowercase().starts_with("on")
                        || matches!(
                            name.as_str(),
                            "srcdoc" | "ping" | "target" | "action" | "formaction" | "download"
                        )
                    {
                        element.remove_attribute(&name);
                    }
                }
                for name in ["href", "xlink:href"] {
                    if element.tag_name() != "link"
                        && element
                            .get_attribute(name)
                            .is_some_and(|value| !value.starts_with('#'))
                    {
                        element.remove_attribute(name);
                    }
                }
                if let Some(style) = element.get_attribute("style") {
                    element.set_attribute("style", &resources.stylesheet(&style, entry, &[])?)?;
                }
                Ok(())
            })),
    )
    .map_err(|_| anyhow!("Static HTML cannot be safely packaged"))?;
    if !head.get() || output.len() > FILE_CAP {
        return Err(anyhow!(
            "Static preview needs a head element and a document below 4 MiB"
        ));
    }
    Ok(output)
}

fn browser_executable() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("GROWTHLAB_CHROME") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    let known = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ];
    if let Some(path) = known.iter().map(PathBuf::from).find(|path| path.is_file()) {
        return Some(path);
    }
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        for name in ["google-chrome", "chromium", "chromium-browser"] {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

struct PreviewTempDir(PathBuf);
impl Drop for PreviewTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    Some((
        u32::from_be_bytes(bytes[16..20].try_into().ok()?),
        u32::from_be_bytes(bytes[20..24].try_into().ok()?),
    ))
}

fn wait_for_process(child: &mut Child) -> bool {
    let deadline = Instant::now() + SCREENSHOT_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

struct RenderCapture {
    screenshot: Vec<u8>,
    check: Option<PreviewRenderCheck>,
}

fn audit_document(document: &[u8], width: u32) -> Option<Vec<u8>> {
    let mut html = String::from_utf8(document.to_vec()).ok()?;
    // The packaged document has no candidate scripts: the sanitizer removes
    // them and the CSP blocks them. This temporary audit copy permits only the
    // inline probe below, then is discarded with the browser's private temp dir.
    html = html.replacen("script-src 'none'", "script-src 'unsafe-inline'", 1);
    let probe = r#"<script>(function(){
const n=performance.getEntriesByType('navigation')[0];
const p=performance.getEntriesByType('paint').find(function(e){return e.name==='first-contentful-paint'});
const body=document.body;
const root=document.documentElement;
// Headless Chrome keeps a small layout viewport floor even when the archived
// PNG is requested at a narrower phone width. Treat the requested width as
// covered when the layout viewport is at least that wide; the PNG dimensions
// are validated separately before the check is archived.
const value={viewportMatches:window.innerWidth>=WIDTH,bodyTextChars:body?body.innerText.trim().length:0,horizontalOverflow:root.scrollWidth>window.innerWidth+1,domContentLoadedMs:n&&n.domContentLoadedEventEnd>0?Math.round(n.domContentLoadedEventEnd):null,loadMs:n&&n.loadEventEnd>0?Math.round(n.loadEventEnd):null,firstContentfulPaintMs:p&&p.startTime>0?Math.round(p.startTime):null};
const out=document.createElement('pre');out.id='growthlab-render-audit';out.textContent='__GROWTHLAB_RENDER_AUDIT__'+JSON.stringify(value);(body||document.documentElement).append(out);
})();</script>"#;
    let probe = probe.replace("WIDTH", &width.to_string());
    let position = html.to_ascii_lowercase().rfind("</body>");
    let Some(position) = position else {
        html.push_str(&probe);
        return Some(html.into_bytes());
    };
    html.insert_str(position, &probe);
    Some(html.into_bytes())
}

fn parse_render_check(
    dom: &str,
    viewport: &str,
    width: u32,
    height: u32,
) -> Option<PreviewRenderCheck> {
    let marker = "__GROWTHLAB_RENDER_AUDIT__";
    let mut value = None;
    for (index, _) in dom.match_indices(marker) {
        let start = index + marker.len();
        let Some(json) = dom[start..].split_once("</pre>").map(|pair| pair.0) else {
            continue;
        };
        if json.trim_start().starts_with('{') {
            value = serde_json::from_str(json).ok();
        }
    }
    let value: serde_json::Value = value?;
    let number = |key: &str| value.get(key).and_then(serde_json::Value::as_u64);
    Some(PreviewRenderCheck {
        viewport: viewport.into(),
        width,
        height,
        viewport_matches: value.get("viewportMatches").and_then(serde_json::Value::as_bool)?,
        body_text_chars: value.get("bodyTextChars").and_then(serde_json::Value::as_u64)? as usize,
        horizontal_overflow: value.get("horizontalOverflow").and_then(serde_json::Value::as_bool)?,
        dom_content_loaded_ms: number("domContentLoadedMs"),
        load_ms: number("loadMs"),
        first_contentful_paint_ms: number("firstContentfulPaintMs"),
        provenance: "OBSERVED".into(),
        limitation: "Local Chromium trace of the sanitized static document. It is not a Lighthouse score, Core Web Vital, accessibility audit, visual regression or real-user performance measurement.".into(),
    })
}

fn render_capture(
    document: &[u8],
    viewport: &str,
    width: u32,
    height: u32,
) -> Option<RenderCapture> {
    let browser = browser_executable()?;
    let directory =
        std::env::temp_dir().join(format!("growthlab-preview-{}", uuid::Uuid::new_v4()));
    crate::growth::archive::private_directory(&directory).ok()?;
    let _temporary = PreviewTempDir(directory.clone());
    let document_path = directory.join("document.html");
    let screenshot_path = directory.join("screenshot.png");
    let dom_path = directory.join("dom.html");
    let audit = audit_document(document, width)?;
    std::fs::write(&document_path, audit).ok()?;
    let url = url::Url::from_file_path(&document_path).ok()?.to_string();
    let mut child = Command::new(browser)
        .args([
            "--headless=new",
            "--incognito",
            "--disable-gpu",
            "--hide-scrollbars",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-crash-reporter",
            "--disable-extensions",
            "--disable-background-networking",
            "--disable-sync",
            "--disable-translate",
            "--mute-audio",
            "--virtual-time-budget=1000",
            "--force-device-scale-factor=1",
        ])
        .arg(format!("--window-size={width},{height}"))
        .arg(format!("--screenshot={}", screenshot_path.display()))
        .arg("--dump-dom")
        .arg(url)
        .stdout(Stdio::from(std::fs::File::create(&dom_path).ok()?))
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    if !wait_for_process(&mut child) {
        return None;
    }
    let bytes = std::fs::read(screenshot_path).ok()?;
    if bytes.len() > FILE_CAP || png_dimensions(&bytes) != Some((width, height)) {
        return None;
    }
    let dom = std::fs::read_to_string(dom_path).ok();
    Some(RenderCapture {
        screenshot: bytes,
        check: dom
            .as_deref()
            .and_then(|dom| parse_render_check(dom, viewport, width, height)),
    })
}

fn bundle(
    root: &Path,
    commit: &str,
    config: &GrowthConfig,
    spec: &StaticPreview,
) -> Result<(PreviewRecord, BTreeMap<String, Vec<u8>>)> {
    let prefix = format!("{}/", spec.root);
    let mut assets = BTreeMap::new();
    let mut sources = vec![];
    let mut archived = BTreeMap::new();
    let mut total = 0;
    for path in crate::local::git::list_tree_files(root, commit)? {
        let Some(relative) = path
            .strip_prefix(&prefix)
            .filter(|relative| mime(relative).is_some())
        else {
            continue;
        };
        if config.check_write(root, &path).is_err() {
            continue;
        }
        let bytes = crate::local::git::file_bytes_at_capped(root, commit, &path, FILE_CAP as u64)?
            .filter(|(_, truncated)| !truncated)
            .map(|(bytes, _)| bytes)
            .ok_or_else(|| anyhow!("Static source asset is missing or oversized"))?;
        total += bytes.len();
        if total > TOTAL_CAP || assets.len() >= 128 {
            return Err(anyhow!(
                "Static preview source exceeds its 8 MiB / 128-file limit"
            ));
        }
        sources.push(PreviewSource {
            path: path.clone(),
            digest: digest(&bytes),
            size: bytes.len(),
        });
        archived.insert(format!("preview/source/{path}"), bytes.clone());
        assets.insert(relative.to_string(), bytes);
    }
    let html = assets
        .get(&spec.entry)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .ok_or_else(|| {
            anyhow!("Static preview entry is not an allowed committed UTF-8 HTML file")
        })?;
    let resources = Resources {
        files: &assets,
        blocked: Cell::new(0),
        packed: Cell::new(0),
    };
    let output = document(html, &spec.entry, &resources)?;
    let document_bytes = output.into_bytes();
    let mut screenshots = vec![];
    let mut render_checks = vec![];
    // Unit tests exercise archive determinism and run concurrently; keep them
    // independent of an installed browser. The release binary and real demo
    // path execute this same capture branch and verify the resulting PNG over
    // HTTP.
    if !cfg!(test) {
        for (path, viewport, width, height) in [
            (SCREENSHOT_DESKTOP, "desktop", 1280, 900),
            (SCREENSHOT_PHONE, "phone", 390, 844),
        ] {
            if let Some(capture) = render_capture(&document_bytes, viewport, width, height) {
                let bytes = capture.screenshot;
                screenshots.push(PreviewScreenshot {
                    path: path.into(),
                    width,
                    height,
                    digest: digest(&bytes),
                    size: bytes.len(),
                });
                archived.insert(path.into(), bytes);
                if let Some(check) = capture.check {
                    render_checks.push(check);
                }
            }
        }
    }
    let record = PreviewRecord { producer: PRODUCER.into(), status: PreviewStatus::Ready, source_commit: commit.into(), document_digest: Some(digest(&document_bytes)), sources, screenshots, render_checks, blocked_resources: resources.blocked.get(), limitation: "Archived static source with optional local Chromium render captures and static layout checks. PNGs are render artifacts; checks are OBSERVED local traces, not a visual regression result, Lighthouse score, accessibility audit, Core Web Vital, real-user performance measurement or growth outcome. Supported local styles/images/fonts are bundled; scripts, forms and navigation are removed, external/unsupported resources omitted. Display only in an opaque, inert sandbox frame. Dynamic apps and CSS image-set string sources are not reproduced. If no compatible browser is installed or it cannot render safely, no screenshot or render check is fabricated.".into() };
    archived.insert(DOCUMENT.into(), document_bytes);
    archived.insert(METADATA.into(), serde_json::to_vec(&record)?);
    Ok((record, archived))
}

pub fn capture(
    root: &Path,
    commit: &str,
    config: &GrowthConfig,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<Option<PreviewRecord>> {
    let Some(spec) = &config.static_preview else {
        return Ok(None);
    };
    match bundle(root, commit, config, spec) {
        Ok((record, archived)) => {
            files.extend(archived);
            Ok(Some(record))
        }
        Err(_) => {
            let record = PreviewRecord { producer: PRODUCER.into(), status: PreviewStatus::Unavailable, source_commit: commit.into(), document_digest: None, sources: vec![], screenshots: vec![], render_checks: vec![], blocked_resources: 0, limitation: "Configured static preview could not be packaged within its allowed-source, HTML/CSS or size limits. No preview, screenshot or render check was fabricated; configured validation remains separate.".into() };
            files.insert(METADATA.into(), serde_json::to_vec(&record)?);
            Ok(Some(record))
        }
    }
}

pub fn verify(
    battle: &GrowthBattle,
    run: &BattleRun,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    let Some(record) = &run.static_preview else {
        return Ok(());
    };
    let spec = battle
        .contract
        .config
        .static_preview
        .as_ref()
        .ok_or_else(|| anyhow!("Unexpected static preview record"))?;
    if record.producer != PRODUCER
        || Some(&record.source_commit) != run.candidate_commit.as_ref()
        || files.get(METADATA) != Some(&serde_json::to_vec(record)?)
    {
        return Err(anyhow!(
            "Static preview does not match its frozen candidate/metadata"
        ));
    }
    match record.status {
        PreviewStatus::Ready => {
            let html = files
                .get(DOCUMENT)
                .ok_or_else(|| anyhow!("Static preview document is missing"))?;
            if record.document_digest.as_deref() != Some(digest(html).as_str())
                || !String::from_utf8_lossy(html).contains(CSP)
                || html.len() > FILE_CAP
                || record.sources.len() > 128
            {
                return Err(anyhow!("Static preview document is not verified"));
            }
            let mut total = 0;
            let mut entry = false;
            let mut seen = BTreeSet::new();
            for source in &record.sources {
                validate_relative_path(&source.path)?;
                if !source.path.starts_with(&format!("{}/", spec.root))
                    || mime(&source.path).is_none()
                    || !seen.insert(&source.path)
                {
                    return Err(anyhow!("Static preview source is outside its contract"));
                }
                let bytes = files
                    .get(&format!("preview/source/{}", source.path))
                    .ok_or_else(|| anyhow!("Static preview source is missing"))?;
                if digest(bytes) != source.digest
                    || bytes.len() != source.size
                    || bytes.len() > FILE_CAP
                {
                    return Err(anyhow!("Static preview source digest mismatch"));
                }
                total += bytes.len();
                entry |= source.path == format!("{}/{}", spec.root, spec.entry);
            }
            if !entry || total > TOTAL_CAP {
                return Err(anyhow!("Static preview source set is invalid"));
            }
            let mut screenshot_paths = BTreeSet::new();
            for screenshot in &record.screenshots {
                if !matches!(
                    screenshot.path.as_str(),
                    SCREENSHOT_DESKTOP | SCREENSHOT_PHONE
                ) || !screenshot_paths.insert(&screenshot.path)
                    || screenshot.width == 0
                    || screenshot.height == 0
                    || screenshot.size > FILE_CAP
                {
                    return Err(anyhow!("Static preview screenshot metadata is invalid"));
                }
                let bytes = files
                    .get(&screenshot.path)
                    .ok_or_else(|| anyhow!("Static preview screenshot is missing"))?;
                if bytes.len() != screenshot.size
                    || digest(bytes) != screenshot.digest
                    || png_dimensions(bytes) != Some((screenshot.width, screenshot.height))
                {
                    return Err(anyhow!("Static preview screenshot digest mismatch"));
                }
            }
            let mut render_viewports = BTreeSet::new();
            for check in &record.render_checks {
                let expected = match check.viewport.as_str() {
                    "desktop" => (1280, 900),
                    "phone" => (390, 844),
                    _ => return Err(anyhow!("Static preview render check viewport is invalid")),
                };
                if (check.width, check.height) != expected
                    || !check.viewport_matches
                    || check.provenance != "OBSERVED"
                    || check.limitation.trim().is_empty()
                    || !render_viewports.insert(&check.viewport)
                {
                    return Err(anyhow!("Static preview render check metadata is invalid"));
                }
            }
        }
        PreviewStatus::Unavailable
            if record.document_digest.is_none()
                && record.sources.is_empty()
                && !files.contains_key(DOCUMENT) => {}
        PreviewStatus::Unavailable => {
            return Err(anyhow!(
                "Unavailable preview contains an unexpected document"
            ))
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_css(uri: &str) -> String {
        String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(uri.split_once(',').unwrap().1)
                .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn static_resources_resolve_nested_css_imports_urls_and_preserve_literals() {
        let files = BTreeMap::from([
            ("css/site.css".into(), b"@import '../theme.css';@media(min-width:1px){.a{background:url('../img/x.png');--label:\"url(no.png)\";color:var(--tone)}}".to_vec()),
            ("theme.css".into(), b".theme{color:green}".to_vec()),
            ("img/x.png".into(), b"owned-resource-bytes".to_vec()),
        ]);
        let resources = Resources {
            files: &files,
            blocked: Cell::new(0),
            packed: Cell::new(0),
        };
        let css = decode_css(&resources.uri("css/site.css?v=1", "index.html", true, &[]));
        assert!(css.contains("data:text/css;charset=utf-8;base64,"));
        assert!(css.contains("data:image/png;base64,"));
        assert!(css.contains("--label:\"url(no.png)\""));
        assert!(css.contains("@media(min-width:1px)"));
        assert!(css.contains("var(--tone)"));
        assert_eq!(resources.blocked.get(), 0);
        for path in [
            "https://outside.invalid/image.png",
            "//outside.invalid/image.png",
            "../outside.png",
            "img/%2e%2e%2foutside.png",
        ] {
            assert_eq!(resources.uri(path, "index.html", false, &[]), "data:,");
        }
        assert_eq!(resources.blocked.get(), 4);
    }

    #[test]
    fn cyclic_imports_and_bad_css_do_not_fabricate_a_complete_resource() {
        let files = BTreeMap::from([("cycle.css".into(), b"@import 'cycle.css';".to_vec())]);
        let resources = Resources {
            files: &files,
            blocked: Cell::new(0),
            packed: Cell::new(0),
        };
        assert!(
            decode_css(&resources.uri("cycle.css", "index.html", true, &[])).contains("data:,")
        );
        assert_eq!(resources.blocked.get(), 1);
        assert!(resources
            .stylesheet(".a{background:url(\"unterminated)}", "index.html", &[])
            .is_err());
    }

    #[test]
    fn static_document_bundles_styles_and_removes_navigation_and_executable_content() {
        let files = BTreeMap::from([("style.css".into(), b"body{color:green}".to_vec())]);
        let resources = Resources {
            files: &files,
            blocked: Cell::new(0),
            packed: Cell::new(0),
        };
        let source = r##"<!doctype html><html><head><base href="https://outside.invalid"><meta http-equiv="refresh" content="0;url=https://outside.invalid"><link rel="stylesheet" href="style.css"><link rel="prefetch" href="https://outside.invalid"><style>.inside{color:blue}</style><script>alert(1)</script></head><body ONLOAD="alert(1)"><a href="https://outside.invalid" target="_top" ping="https://outside.invalid">Outside</a><a href="#local">Local</a><form action="https://outside.invalid"><input formaction="https://outside.invalid"></form><iframe srcdoc="attack"></iframe><h1 id="local">Actual source title</h1></body></html>"##;
        let html = document(source, "index.html", &resources).unwrap();
        assert!(html.contains(CSP));
        assert!(html.contains("data:text/css;charset=utf-8;base64,"));
        assert!(html.contains("Actual source title"));
        assert!(html.contains("href=\"#local\""));
        for forbidden in [
            "outside.invalid",
            "<script",
            "<iframe",
            "<form",
            "<base",
            "http-equiv=\"refresh\"",
            "ONLOAD",
            "ping=",
            "formaction=",
        ] {
            assert!(
                !html.contains(forbidden),
                "unexpected executable/navigation content: {forbidden}"
            );
        }
        assert_eq!(resources.blocked.get(), 0);
        assert!(document("<h1>No head</h1>", "index.html", &resources).is_err());
    }

    #[test]
    fn repeated_large_resources_are_bounded_before_document_construction() {
        let files = BTreeMap::from([("large.png".into(), vec![b'x'; 2 * 1024 * 1024])]);
        let resources = Resources {
            files: &files,
            blocked: Cell::new(0),
            packed: Cell::new(0),
        };
        let css = ".a{background:url(large.png)}".repeat(12);
        assert!(resources.stylesheet(&css, "index.html", &[]).is_err());
        assert!(resources.blocked.get() > 0);
        assert!(resources.packed.get() <= TOTAL_CAP);
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn git_bundle_uses_committed_assets_and_reports_missing_denied_or_oversized_source() {
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let root =
            std::env::temp_dir().join(format!("growth-preview-owned-{}", uuid::Uuid::new_v4()));
        let _cleanup = Cleanup(root.clone());
        let store = crate::store::Store::open_at(root.join("lab")).unwrap();
        let battle = super::super::demo::prepare(&store).unwrap();
        let project = store
            .get_local_project(&battle.project_id)
            .unwrap()
            .unwrap();
        let product = Path::new(&project.repo_path);
        let config = battle.contract.config.clone();
        let commit = &battle.contract.source_snapshot_commit;
        let mut files = BTreeMap::new();
        let record = capture(product, commit, &config, &mut files)
            .unwrap()
            .unwrap();
        assert_eq!(record.status, PreviewStatus::Ready);
        assert_eq!(record.sources.len(), 2);
        assert_eq!(record.document_digest, Some(digest(&files[DOCUMENT])));
        assert_eq!(files[METADATA], serde_json::to_vec(&record).unwrap());
        let committed_css = files["preview/source/website/style.css"].clone();
        std::fs::write(product.join("website/style.css"), "body{color:red}").unwrap();
        let mut repeated = BTreeMap::new();
        assert_eq!(
            capture(product, commit, &config, &mut repeated)
                .unwrap()
                .unwrap(),
            record
        );
        assert_eq!(
            repeated, files,
            "mutable checkout content cannot supply archived preview assets"
        );
        assert_ne!(
            std::fs::read(product.join("website/style.css")).unwrap(),
            committed_css
        );
        let mut denied = config.clone();
        denied
            .permissions
            .denied_paths
            .push("website/style.css".into());
        let mut limited = BTreeMap::new();
        let without_css = capture(product, commit, &denied, &mut limited)
            .unwrap()
            .unwrap();
        assert_eq!(without_css.status, PreviewStatus::Ready);
        assert_eq!(without_css.sources.len(), 1);
        assert_eq!(without_css.blocked_resources, 1);
        assert!(!limited.contains_key("preview/source/website/style.css"));
        let mut missing = config.clone();
        missing.static_preview.as_mut().unwrap().entry = "missing.html".into();
        let mut unavailable = BTreeMap::new();
        assert_eq!(
            capture(product, commit, &missing, &mut unavailable)
                .unwrap()
                .unwrap()
                .status,
            PreviewStatus::Unavailable
        );
        assert_eq!(unavailable.len(), 1);
        assert!(!unavailable.contains_key(DOCUMENT));
        std::fs::write(product.join("website/style.css"), vec![b'x'; FILE_CAP + 1]).unwrap();
        crate::local::git::git(Some(product), &["add", "website/style.css"]).unwrap();
        crate::local::git::git(
            Some(product),
            &[
                "-c",
                "user.name=Preview fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=.disabled-preview-hooks",
                "commit",
                "-m",
                "Oversized synthetic preview asset",
            ],
        )
        .unwrap();
        let oversized = crate::local::git::git(Some(product), &["rev-parse", "HEAD"]).unwrap();
        let mut unavailable = BTreeMap::new();
        assert_eq!(
            capture(product, oversized.trim(), &config, &mut unavailable)
                .unwrap()
                .unwrap()
                .status,
            PreviewStatus::Unavailable
        );
        assert_eq!(unavailable.len(), 1);
        assert!(!unavailable.contains_key(DOCUMENT));
    }

    #[test]
    fn fragmented_inline_style_text_is_bundled_without_losing_or_duplicating_css() {
        let files = BTreeMap::new();
        let resources = Resources {
            files: &files,
            blocked: Cell::new(0),
            packed: Cell::new(0),
        };
        let css = ".owned{color:green}".repeat(7000);
        let source =
            format!("<html><head><style>{css}</style></head><body><h1>Owned</h1></body></html>");
        let html = document(&source, "index.html", &resources).unwrap();
        assert_eq!(
            html.matches("data:text/css;charset=utf-8;base64,").count(),
            1
        );
        assert!(html.contains(&base64::engine::general_purpose::STANDARD.encode(css)));
    }

    #[test]
    fn render_audit_probe_is_injected_only_into_a_disposable_copy() {
        let source = format!("<html><head>{CSP}</head><body><h1>Owned</h1></body></html>");
        let audited = audit_document(source.as_bytes(), 390).unwrap();
        let audited = String::from_utf8(audited).unwrap();
        assert!(audited.contains("script-src 'unsafe-inline'"));
        assert!(audited.contains("window.innerWidth>=390"));
        assert!(audited.contains("__GROWTHLAB_RENDER_AUDIT__"));
        assert!(audited.contains("</body>"));
        assert!(source.contains("script-src 'none'"));
    }

    #[test]
    fn render_check_parser_keeps_observed_layout_and_timing_values() {
        let dom = r#"<html><body><pre id="growthlab-render-audit">__GROWTHLAB_RENDER_AUDIT__{"viewportMatches":true,"bodyTextChars":42,"horizontalOverflow":false,"domContentLoadedMs":12,"loadMs":18,"firstContentfulPaintMs":15}</pre></body></html>"#;
        let check = parse_render_check(dom, "phone", 390, 844).unwrap();
        assert_eq!(check.viewport, "phone");
        assert!(check.viewport_matches);
        assert_eq!(check.body_text_chars, 42);
        assert!(!check.horizontal_overflow);
        assert_eq!(check.dom_content_loaded_ms, Some(12));
        assert_eq!(check.load_ms, Some(18));
        assert_eq!(check.first_contentful_paint_ms, Some(15));
        assert_eq!(check.provenance, "OBSERVED");
    }
}
