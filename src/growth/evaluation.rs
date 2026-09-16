//! Explainable deterministic comparison. Passing a command is an observation,
//! while the optional HTML rubric is an estimated structural SEO review. Neither
//! is proof of conversion lift, ranking, accessibility or performance.
use regex::Regex;
use serde::Serialize;

use crate::error::{anyhow, Result};
use crate::store::Store;

use super::archive;
use super::battle_model::{GrowthBattle, SealedBattleRun, ValidationRecord};
use super::model::{Confidence, ConfidenceLabel, Provenance};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationRow {
    pub variant_id: String,
    pub title: String,
    pub status: String,
    pub checks: Vec<ValidationRecord>,
    pub passed_commands: usize,
    pub required_commands: usize,
    pub pass_fraction: Option<f64>,
    pub eligible: bool,
    pub implementation_provenance: Provenance,
    pub check_provenance: Provenance,
    pub outcome_provenance: Provenance,
    pub confidence: Confidence,
    pub archive_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rubric: Option<SeoRubric>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<PageQualityRubric>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RubricDimension {
    pub key: String,
    pub label: String,
    pub score: u8,
    pub max_score: u8,
    pub status: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SeoRubric {
    pub id: String,
    pub label: String,
    pub score: u8,
    pub max_score: u8,
    pub provenance: Provenance,
    pub dimensions: Vec<RubricDimension>,
    pub recommendations: Vec<String>,
    pub calculation: String,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PageQualityRubric {
    pub id: String,
    pub label: String,
    pub score: u8,
    pub max_score: u8,
    pub provenance: Provenance,
    pub dimensions: Vec<RubricDimension>,
    pub recommendations: Vec<String>,
    pub calculation: String,
    pub limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub battle_id: String,
    pub label: String,
    pub evaluator: String,
    pub calculation: String,
    pub limitations: Vec<String>,
    pub recommended_candidates: Vec<String>,
    pub rows: Vec<EvaluationRow>,
}

fn openings(html: &str, tag: &str) -> Vec<String> {
    let pattern = format!(r"(?is)<{tag}\b[^>]*>");
    Regex::new(&pattern)
        .expect("static HTML tag pattern is valid")
        .find_iter(html)
        .map(|match_| match_.as_str().to_string())
        .collect()
}

fn paired(html: &str, tag: &str) -> Vec<String> {
    let pattern = format!(r"(?is)<{tag}\b[^>]*>(.*?)</{tag}\s*>");
    Regex::new(&pattern)
        .expect("static HTML paired-tag pattern is valid")
        .captures_iter(html)
        .filter_map(|capture| capture.get(1).map(|value| value.as_str().to_string()))
        .collect()
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)\b{}\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#,
        regex::escape(name)
    );
    Regex::new(&pattern)
        .expect("static HTML attribute pattern is valid")
        .captures(tag)
        .and_then(|capture| {
            (1..=3).find_map(|index| capture.get(index).map(|value| value.as_str().to_string()))
        })
}

fn text_content(value: &str) -> String {
    let without_blocks = Regex::new(r"(?is)<(?:script|style)\b[^>]*>.*?</(?:script|style)\s*>")
        .expect("static HTML block pattern is valid")
        .replace_all(value, " ");
    let without_tags = Regex::new(r"(?is)<[^>]+>")
        .expect("static HTML tag stripping pattern is valid")
        .replace_all(&without_blocks, " ");
    Regex::new(r"\s+")
        .expect("static whitespace pattern is valid")
        .replace_all(&without_tags, " ")
        .trim()
        .to_string()
}

fn dimension(
    key: &str,
    label: &str,
    score: u8,
    max_score: u8,
    evidence: impl IntoIterator<Item = String>,
) -> RubricDimension {
    RubricDimension {
        key: key.into(),
        label: label.into(),
        score,
        max_score,
        status: if score == max_score {
            "strong"
        } else if score > 0 {
            "partial"
        } else {
            "missing"
        }
        .into(),
        evidence: evidence.into_iter().collect(),
    }
}

/// Score basic, inspectable SEO page hygiene from an archived HTML candidate.
/// This is deliberately structural and returns `ESTIMATED` provenance: it does
/// not crawl the web, inspect rankings, or predict traffic.
pub fn seo_rubric(html: &str) -> SeoRubric {
    let titles = paired(html, "title");
    let title_length = titles
        .first()
        .map(|title| text_content(title).len())
        .unwrap_or(0);
    let title_score = match title_length {
        10..=60 => 20,
        1..=200 => 12,
        _ => 0,
    };
    let title_dimension = dimension(
        "title",
        "Search title",
        title_score,
        20,
        [if title_length == 0 {
            "No non-empty <title> element was found.".into()
        } else {
            format!("Title contains {title_length} characters; 10–60 is the rubric range.")
        }],
    );

    let description = openings(html, "meta").into_iter().find(|tag| {
        attribute(tag, "name").is_some_and(|name| name.eq_ignore_ascii_case("description"))
    });
    let description_length = description
        .as_deref()
        .and_then(|tag| attribute(tag, "content"))
        .map(|value| text_content(&value).len())
        .unwrap_or(0);
    let description_score = match description_length {
        70..=160 => 20,
        1..=300 => 12,
        _ => 0,
    };
    let description_dimension = dimension(
        "description",
        "Search description",
        description_score,
        20,
        [if description_length == 0 {
            "No non-empty meta description was found.".into()
        } else {
            format!(
                "Description contains {description_length} characters; 70–160 is the rubric range."
            )
        }],
    );

    let h1s = paired(html, "h1");
    let h2_count = paired(html, "h2").len();
    let non_empty_h1s = h1s
        .iter()
        .filter(|heading| !text_content(heading).is_empty())
        .count();
    let heading_score = match (non_empty_h1s, h2_count) {
        (1, count) if count > 0 => 20,
        (1, _) => 15,
        (count, _) if count > 1 => 10,
        _ => 0,
    };
    let heading_dimension = dimension(
        "headings",
        "Heading structure",
        heading_score,
        20,
        [format!(
            "Found {non_empty_h1s} non-empty h1 and {h2_count} h2 elements; one h1 is required."
        )],
    );

    let language = openings(html, "html")
        .first()
        .and_then(|tag| attribute(tag, "lang"))
        .filter(|value| !value.trim().is_empty());
    let language_dimension = dimension(
        "language",
        "Document language",
        u8::from(language.is_some()) * 10,
        10,
        [match language {
            Some(language) => format!("The document declares lang=\"{language}\"."),
            None => "The html element does not declare a language.".into(),
        }],
    );

    let visible_length = text_content(html).len();
    let content_score = match visible_length {
        240.. => 15,
        120..=239 => 10,
        1..=119 => 5,
        _ => 0,
    };
    let content_dimension = dimension(
        "content",
        "Useful page copy",
        content_score,
        15,
        [format!(
            "The archived HTML contains about {visible_length} visible text characters."
        )],
    );

    let canonical = openings(html, "link").into_iter().any(|tag| {
        attribute(&tag, "rel").is_some_and(|rel| {
            rel.split_ascii_whitespace()
                .any(|value| value.eq_ignore_ascii_case("canonical"))
        }) && attribute(&tag, "href").is_some_and(|href| !href.trim().is_empty())
    });
    let canonical_dimension = dimension(
        "canonical",
        "Canonical URL",
        u8::from(canonical) * 5,
        5,
        [if canonical {
            "A canonical link with a non-empty href is present.".into()
        } else {
            "No canonical link with a non-empty href was found.".into()
        }],
    );

    let links = openings(html, "a")
        .into_iter()
        .filter(|tag| {
            attribute(tag, "href").is_some_and(|href| !href.trim().is_empty() && href.trim() != "#")
        })
        .count();
    let links_dimension = dimension(
        "links",
        "Useful links",
        u8::from(links > 0) * 5,
        5,
        [format!(
            "Found {links} non-empty links that can lead a reader to the next action."
        )],
    );

    let images = openings(html, "img");
    let missing_alt = images
        .iter()
        .filter(|tag| attribute(tag, "alt").is_none_or(|alt| alt.trim().is_empty()))
        .count();
    let media_dimension = dimension(
        "media",
        "Image descriptions",
        u8::from(images.is_empty() || missing_alt == 0) * 5,
        5,
        [if images.is_empty() {
            "No images are present; there is no image text to audit.".into()
        } else {
            format!(
                "Found {} images; {missing_alt} are missing a useful alt attribute.",
                images.len()
            )
        }],
    );

    let dimensions = vec![
        title_dimension,
        description_dimension,
        heading_dimension,
        language_dimension,
        content_dimension,
        canonical_dimension,
        links_dimension,
        media_dimension,
    ];
    let recommendations = dimensions
        .iter()
        .filter(|dimension| dimension.status != "strong")
        .filter_map(|dimension| match dimension.key.as_str() {
            "title" => Some("Add one descriptive <title> between 10 and 60 characters.".into()),
            "description" => {
                Some("Add a useful meta description between 70 and 160 characters.".into())
            }
            "headings" => Some("Use one clear <h1> and at least one supporting <h2>.".into()),
            "language" => Some("Declare the document language with an html lang attribute.".into()),
            "content" => {
                Some("Add more visible, product-specific copy that explains the page.".into())
            }
            "canonical" => Some("Add a canonical link with a stable, non-empty URL.".into()),
            "links" => Some("Add at least one useful link to the next reader action.".into()),
            "media" => Some("Give every image a concise, useful alt attribute.".into()),
            _ => None,
        })
        .collect();
    let score = dimensions
        .iter()
        .map(|dimension| dimension.score as u16)
        .sum::<u16>() as u8;
    SeoRubric {
        id: "seo-page-hygiene-v1".into(),
        label: "SEO page hygiene (estimated)".into(),
        score,
        max_score: 100,
        provenance: Provenance::Estimated,
        dimensions,
        recommendations,
        calculation: "100-point structural rubric: title 20, description 20, headings 20, language 10, useful copy 15, canonical 5, links 5 and image descriptions 5.".into(),
        limitations: vec![
            "This reviews the archived HTML only; it does not crawl, index, rank or measure traffic.".into(),
            "Scores are estimated structural signals, not a predicted position or conversion result.".into(),
            "External links, search demand, backlinks, structured data and real user behavior are not evaluated here.".into(),
        ],
    }
}

fn non_empty_attribute(tag: &str, name: &str) -> bool {
    attribute(tag, name).is_some_and(|value| !value.trim().is_empty())
}

fn quality_control_counts(html: &str) -> (usize, usize, usize, usize) {
    let mut interactive = 0;
    let mut unnamed = 0;
    for tag in ["a", "button"] {
        let pattern = format!(r"(?is)<{tag}\b([^>]*)>(.*?)</{tag}\s*>");
        for capture in Regex::new(&pattern)
            .expect("static control pattern is valid")
            .captures_iter(html)
        {
            interactive += 1;
            let attrs = capture
                .get(1)
                .map(|value| value.as_str())
                .unwrap_or_default();
            let content = capture
                .get(2)
                .map(|value| value.as_str())
                .unwrap_or_default();
            if !non_empty_attribute(attrs, "aria-label")
                && !non_empty_attribute(attrs, "title")
                && text_content(content).is_empty()
            {
                unnamed += 1;
            }
        }
    }
    let mut forms = 0;
    let mut labelled = 0;
    for tag in ["input", "select", "textarea"] {
        for opening in openings(html, tag) {
            forms += 1;
            let id = attribute(&opening, "id");
            let labelled_by_for = id.as_deref().is_some_and(|id| {
                openings(html, "label")
                    .into_iter()
                    .any(|label| attribute(&label, "for").is_some_and(|target| target == id))
            });
            if non_empty_attribute(&opening, "aria-label")
                || non_empty_attribute(&opening, "title")
                || labelled_by_for
            {
                labelled += 1;
            }
        }
    }
    (interactive, unnamed, forms, labelled)
}

/// Score conservative page-quality hints from archived HTML. These are
/// structural checks only: no browser, screen reader or network is involved.
pub fn page_quality_rubric(html: &str) -> PageQualityRubric {
    let viewport = openings(html, "meta").into_iter().any(|tag| {
        attribute(&tag, "name").is_some_and(|name| name.eq_ignore_ascii_case("viewport"))
            && non_empty_attribute(&tag, "content")
    });
    let (interactive, unnamed, forms, labelled) = quality_control_counts(html);
    let external_styles = openings(html, "link")
        .into_iter()
        .filter(|tag| {
            attribute(tag, "rel").is_some_and(|rel| {
                rel.split_ascii_whitespace()
                    .any(|value| value.eq_ignore_ascii_case("stylesheet"))
            }) && non_empty_attribute(tag, "href")
        })
        .count();
    let blocking_scripts = openings(html, "script")
        .into_iter()
        .filter(|tag| {
            non_empty_attribute(tag, "src")
                && !non_empty_attribute(tag, "async")
                && !non_empty_attribute(tag, "defer")
                && !attribute(tag, "type").is_some_and(|value| value.eq_ignore_ascii_case("module"))
        })
        .count();
    let dimensions = vec![
        dimension(
            "viewport",
            "Mobile viewport",
            u8::from(viewport) * 5,
            5,
            [if viewport {
                "A non-empty viewport declaration is present.".into()
            } else {
                "No non-empty viewport declaration was found.".into()
            }],
        ),
        dimension(
            "controls",
            "Named interactive controls",
            if interactive == 0 || unnamed == 0 {
                10
            } else if unnamed < interactive {
                5
            } else {
                0
            },
            10,
            [format!(
                "Found {interactive} links or buttons; {unnamed} have no visible or explicit accessible name."
            )],
        ),
        dimension(
            "forms",
            "Form control labels",
            if forms == 0 || labelled == forms {
                5
            } else if labelled > 0 {
                2
            } else {
                0
            },
            5,
            [format!("Found {forms} form controls; {labelled} have an explicit label association or name.")],
        ),
        dimension(
            "loading",
            "Loading hints",
            if external_styles + blocking_scripts == 0 {
                5
            } else if external_styles + blocking_scripts <= 2 {
                3
            } else {
                0
            },
            5,
            [format!(
                "Found {external_styles} external stylesheets and {blocking_scripts} parser-blocking scripts; this is a source hint, not a timing measurement."
            )],
        ),
    ];
    let recommendations = dimensions
        .iter()
        .filter(|item| item.status != "strong")
        .filter_map(|item| match item.key.as_str() {
            "viewport" => Some("Add a viewport declaration so narrow screens get an intentional layout.".into()),
            "controls" => Some("Give every link and button visible text or an explicit accessible name.".into()),
            "forms" => Some("Associate each form control with a visible label or an accessible name.".into()),
            "loading" => Some("Review external styles and parser-blocking scripts; confirm timing in a real browser.".into()),
            _ => None,
        })
        .collect();
    PageQualityRubric {
        id: "page-quality-hints-v1".into(),
        label: "Page quality hints (estimated)".into(),
        score: dimensions.iter().map(|item| item.score as u16).sum::<u16>() as u8,
        max_score: 25,
        provenance: Provenance::Estimated,
        dimensions,
        recommendations,
        calculation: "25-point structural hint set: mobile viewport 5, named controls 10, form labels 5 and loading hints 5.".into(),
        limitations: vec![
            "This inspects archived HTML only; it does not run Lighthouse, a browser timing trace or a screen reader.".into(),
            "A strong hint is not an accessibility certification, Core Web Vital or performance result.".into(),
            "Review the rendered page on supported devices and run dedicated accessibility and performance tools before shipping.".into(),
        ],
    }
}

fn html_for_run<'a>(
    battle: &GrowthBattle,
    run: &'a super::battle_model::BattleRun,
) -> Option<&'a str> {
    let implementation = run.implementation.as_ref()?;
    let preferred = battle
        .contract
        .config
        .static_preview
        .as_ref()
        .map(|preview| format!("{}/{}", preview.root, preview.entry));
    implementation
        .files
        .iter()
        .find(|file| {
            file.contents.is_some() && preferred.as_deref().is_some_and(|path| path == file.path)
        })
        .or_else(|| {
            implementation.files.iter().find(|file| {
                file.contents.is_some() && file.path.to_ascii_lowercase().ends_with(".html")
            })
        })
        .and_then(|file| file.contents.as_deref())
}

pub trait BattleEvaluator {
    fn id(&self) -> &str;
    fn calculation(&self) -> &str;
    fn evaluate(
        &self,
        battle: &GrowthBattle,
        run: Option<&SealedBattleRun>,
        title: &str,
        variant_id: &str,
    ) -> EvaluationRow;
}

pub struct ConfiguredCommandEvaluator;
impl BattleEvaluator for ConfiguredCommandEvaluator {
    fn id(&self) -> &str {
        "configured-command-pass-v1+seo-page-hygiene-v1"
    }
    fn calculation(&self) -> &str {
        "Configured command pass fraction is shown alongside independent estimated SEO page-hygiene and page-quality hints. Eligibility requires a successful sealed run, the exact frozen command list and one immutable candidate commit; neither signal proves traffic, ranking or conversion lift."
    }
    fn evaluate(
        &self,
        battle: &GrowthBattle,
        sealed: Option<&SealedBattleRun>,
        title: &str,
        variant_id: &str,
    ) -> EvaluationRow {
        let run = sealed.map(|sealed| &sealed.run);
        let checks = run.map(|run| run.validations.clone()).unwrap_or_default();
        let required = battle.contract.config.validation.commands.len();
        let passed = checks
            .iter()
            .filter(|check| check.status == "done" && check.exit_code == Some(0))
            .count();
        let matches = run.is_some_and(|run| {
            run.candidate_commit.is_some()
                && checks.len() == required
                && checks
                    .iter()
                    .zip(&battle.contract.config.validation.commands)
                    .all(|(check, command)| {
                        &check.command == command
                            && Some(&check.source_commit) == run.candidate_commit.as_ref()
                    })
        });
        let html = run.and_then(|run| html_for_run(battle, run));
        let rubric = html.map(seo_rubric);
        let quality = html.map(page_quality_rubric);
        EvaluationRow { variant_id:variant_id.into(),title:title.into(),status:run.map(|run|run.status.clone()).unwrap_or("untested".into()),passed_commands:passed,required_commands:required,pass_fraction:(!checks.is_empty() && required>0).then_some(passed as f64 / required.max(1) as f64),eligible:required>0 && matches && passed==required && run.is_some_and(|run|run.status=="done"),checks,implementation_provenance:run.map(|run|run.provenance).unwrap_or(Provenance::Untested),check_provenance:if run.is_some_and(|run|!run.validations.is_empty()) {Provenance::Observed} else {Provenance::Untested},outcome_provenance:Provenance::Untested,confidence:Confidence { label:ConfidenceLabel::Low,rationale:"Command checks and structural SEO or page-quality signals cannot establish outcome lift; candidate choice needs user review and real product evidence.".into() },archive_digest:sealed.map(|sealed|sealed.archive_digest.clone()),rubric,quality }
    }
}

pub fn compare(store: &Store, id: &str) -> Result<Comparison> {
    compare_with(store, id, &ConfiguredCommandEvaluator)
}

pub fn compare_with(
    store: &Store,
    id: &str,
    evaluator: &dyn BattleEvaluator,
) -> Result<Comparison> {
    let battle = store
        .get_growth_battle(id)?
        .ok_or_else(|| anyhow!("Battle not found"))?;
    let variants = store.growth_variants(id)?;
    let runs = store.sealed_growth_runs(id)?;
    for sealed in &runs {
        let files = archive::verify(store.data_root(), &sealed.archive_digest)?;
        if files.get("run.json") != Some(&serde_json::to_vec(&sealed.run)?)
            || files.get("contract.json") != Some(&serde_json::to_vec(&battle.contract)?)
            || sealed.run.contract_digest != battle.contract_digest
        {
            return Err(anyhow!(
                "Sealed run does not match its frozen battle; comparison refused"
            ));
        }
        for (index, check) in sealed.run.validations.iter().enumerate() {
            if let Some(record) = &check.confinement {
                let policy = files
                    .get(&format!("validation-{index}.policy.json"))
                    .ok_or_else(|| anyhow!("Sealed validation confinement policy is missing"))?;
                super::confinement::verify_record(record, policy)?;
            }
        }
        super::preview::verify(&battle, &sealed.run, &files)?;
    }
    let rows: Vec<_> = variants
        .iter()
        .zip(&battle.contract.hypotheses)
        .map(|(variant, hypothesis)| {
            evaluator.evaluate(
                &battle,
                runs.iter()
                    .find(|sealed| sealed.run.variant_id == variant.id),
                &hypothesis.title,
                &variant.id,
            )
        })
        .collect();
    Ok(Comparison {
        battle_id: id.into(),
        label: "Recommended candidates".into(),
        evaluator: evaluator.id().into(),
        calculation: evaluator.calculation().into(),
        limitations: battle.contract.limitations.clone(),
        recommended_candidates: rows
            .iter()
            .filter(|row| row.eligible)
            .map(|row| row.variant_id.clone())
            .collect(),
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::{page_quality_rubric, seo_rubric, Provenance};

    #[test]
    fn seo_rubric_exposes_each_structural_signal_and_estimated_provenance() {
        let html = r#"<!doctype html><html lang="en"><head>
            <title>Invoice software for small teams</title>
            <meta name="description" content="Create, send, and track professional invoices in one calm workspace for small teams and independent businesses.">
            <link rel="canonical" href="https://example.test/invoices">
        </head><body><h1>Send invoices without the busywork</h1>
            <h2>Everything your team needs to get paid</h2>
            <p>Make invoices, share them with customers, and see what needs attention in one place. Keep your records clear and your follow-up simple.</p>
            <p>Start with a template, invite a teammate, and keep the whole process easy to understand from the first visit through the final payment.</p>
            <a href="/start">Try the workspace</a><img src="hero.png" alt="Invoice workspace overview">
        </body></html>"#;
        let rubric = seo_rubric(html);
        assert_eq!(rubric.provenance, Provenance::Estimated);
        assert_eq!(rubric.max_score, 100);
        assert_eq!(rubric.score, 100);
        assert_eq!(rubric.dimensions.len(), 8);
        assert!(rubric
            .dimensions
            .iter()
            .all(|dimension| dimension.score == dimension.max_score));
        assert!(rubric.recommendations.is_empty());
        assert!(rubric.calculation.contains("title 20"));
        assert_eq!(rubric.limitations.len(), 3);
    }

    #[test]
    fn seo_rubric_calls_out_missing_search_and_heading_signals() {
        let rubric = seo_rubric("<p>A short page without search metadata.</p>");
        assert!(rubric.score < 30);
        assert_eq!(rubric.dimensions[0].status, "missing");
        assert_eq!(rubric.dimensions[1].status, "missing");
        assert_eq!(rubric.dimensions[2].status, "missing");
        assert!(rubric.dimensions[0].evidence[0].contains("No non-empty"));
        assert!(rubric.dimensions[1].evidence[0].contains("No non-empty"));
        assert!(rubric
            .recommendations
            .iter()
            .any(|recommendation| recommendation.contains("<title>")));
    }

    #[test]
    fn page_quality_rubric_exposes_conservative_structural_hints() {
        let html = r#"<!doctype html><html lang="en"><head>
            <meta name="viewport" content="width=device-width, initial-scale=1">
            <link rel="stylesheet" href="app.css">
            <script src="app.js" defer></script>
        </head><body><a href="/start">Start</a><button aria-label="Close">×</button>
            <form><label for="email">Email</label><input id="email" type="email"></form>
        </body></html>"#;
        let rubric = page_quality_rubric(html);
        assert_eq!(rubric.provenance, Provenance::Estimated);
        assert_eq!(rubric.max_score, 25);
        assert_eq!(rubric.score, 23);
        assert_eq!(rubric.dimensions.len(), 4);
        assert!(rubric
            .dimensions
            .iter()
            .any(|dimension| dimension.key == "loading" && dimension.status == "partial"));
        assert!(rubric
            .limitations
            .iter()
            .any(|limitation| limitation.contains("Lighthouse")));
    }

    #[test]
    fn page_quality_rubric_flags_unnamed_controls_and_missing_viewport() {
        let rubric = page_quality_rubric(
            r#"<html><body><a href="/next"></a><button></button><input id="name"></body></html>"#,
        );
        assert!(rubric.score <= 10);
        assert_eq!(rubric.dimensions[0].status, "missing");
        assert_eq!(rubric.dimensions[1].status, "missing");
        assert_eq!(rubric.dimensions[2].status, "missing");
        assert!(rubric
            .recommendations
            .iter()
            .any(|recommendation| recommendation.contains("accessible name")));
    }
}
