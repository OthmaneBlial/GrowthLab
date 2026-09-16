//! Read-only metadata inspection for a public GitHub repository URL.
//!
//! This intentionally stops at public repository metadata. It does not clone,
//! execute, modify or register the repository. A user can use the result to
//! decide whether to prepare a separate, explicitly configured local product
//! workspace.

use futures::StreamExt;
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};

use crate::error::{anyhow, Result};
use crate::store::now_ms;

use super::model::Provenance;

const MAX_RESPONSE_BYTES: usize = 512 * 1024;
const REQUEST_TIMEOUT_SECONDS: u64 = 15;
const USER_AGENT: &str =
    "GrowthLab-public-repository-audit/0.1 (+https://github.com/OthmaneBlial/GrowthLab)";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PublicRepositoryAudit {
    pub url: String,
    pub api_url: String,
    pub owner: String,
    pub repository: String,
    pub full_name: String,
    pub html_url: String,
    pub description: Option<String>,
    pub default_branch: Option<String>,
    pub license: Option<String>,
    pub archived: bool,
    pub fork: bool,
    pub retrieved_at: i64,
    pub scope: String,
    pub provenance: Provenance,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct RepositoryTarget {
    owner: String,
    repository: String,
    canonical_url: String,
    api_url: String,
}

#[derive(Debug, Deserialize)]
struct GithubRepository {
    full_name: Option<String>,
    html_url: Option<String>,
    description: Option<String>,
    default_branch: Option<String>,
    license: Option<GithubLicense>,
    archived: Option<bool>,
    fork: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct GithubLicense {
    spdx_id: Option<String>,
    name: Option<String>,
}

fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && !value.starts_with('.')
        && !value.ends_with('.')
}

fn target(raw: &str) -> Result<RepositoryTarget> {
    let url = Url::parse(raw.trim()).map_err(|_| anyhow!("Public repository URL is invalid"))?;
    if url.scheme() != "https" {
        return Err(anyhow!(
            "Public repository analysis accepts HTTPS URLs only"
        ));
    }
    if url.username() != "" || url.password().is_some() {
        return Err(anyhow!("Public repository URL cannot contain credentials"));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(anyhow!(
            "Public repository URL cannot contain a query string or fragment"
        ));
    }
    if url.port().is_some_and(|port| port != 443) {
        return Err(anyhow!(
            "Public repository analysis accepts the default HTTPS port only"
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("Public repository URL must use github.com"))?
        .to_ascii_lowercase();
    if host != "github.com" && host != "www.github.com" {
        return Err(anyhow!("Public repository URL must point to github.com"));
    }
    let segments: Vec<_> = url
        .path()
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.len() != 2 || !valid_segment(segments[0]) || !valid_segment(segments[1]) {
        return Err(anyhow!(
            "Public repository URL must have the form https://github.com/owner/repository"
        ));
    }
    if segments[1].ends_with(".git") {
        return Err(anyhow!(
            "Use the public GitHub page URL without a .git suffix"
        ));
    }
    let owner = segments[0].to_string();
    let repository = segments[1].to_string();
    let canonical_url = format!("https://github.com/{owner}/{repository}");
    let api_url = format!("https://api.github.com/repos/{owner}/{repository}");
    Ok(RepositoryTarget {
        owner,
        repository,
        canonical_url,
        api_url,
    })
}

/// Validate a canonical public GitHub repository URL for a later, explicit
/// local checkout. This performs no network request and never clones source.
pub fn canonical_url(raw: &str) -> Result<String> {
    #[cfg(test)]
    if std::path::Path::new(raw.trim()).exists() {
        return Ok(raw.trim().to_owned());
    }
    Ok(target(raw)?.canonical_url)
}

fn client() -> Result<Client> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|_| anyhow!("Could not create the read-only repository client"))
}

async fn bounded_body(response: reqwest::Response) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(anyhow!(
            "GitHub metadata response exceeds the {MAX_RESPONSE_BYTES}-byte limit"
        ));
    }
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| anyhow!("Could not read GitHub repository metadata"))?;
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(anyhow!(
                "GitHub metadata response exceeds the {MAX_RESPONSE_BYTES}-byte limit"
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Inspect one public repository metadata endpoint. No token, cookie, clone,
/// checkout, file execution or local workspace registration is performed.
pub async fn fetch(raw: &str) -> Result<PublicRepositoryAudit> {
    let target = target(raw)?;
    let client = client()?;
    let response = client
        .get(&target.api_url)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| anyhow!("Could not read GitHub metadata; repository analysis was refused"))?;
    let status = response.status();
    if status.is_redirection() {
        return Err(anyhow!(
            "GitHub metadata returned a redirect; repository analysis was refused"
        ));
    }
    if status == StatusCode::NOT_FOUND {
        return Err(anyhow!("Public GitHub repository was not found"));
    }
    if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
        return Err(anyhow!(
            "GitHub metadata rate limit or access boundary refused the analysis"
        ));
    }
    if !status.is_success() {
        return Err(anyhow!(
            "GitHub metadata returned HTTP {}; repository analysis was refused",
            status.as_u16()
        ));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !content_type.contains("json") {
        return Err(anyhow!(
            "GitHub metadata returned a non-JSON response; repository analysis was refused"
        ));
    }
    let body = bounded_body(response).await?;
    let metadata: GithubRepository = serde_json::from_slice(&body).map_err(|_| {
        anyhow!("GitHub metadata was not valid JSON; repository analysis was refused")
    })?;
    let license = metadata.license.and_then(|license| {
        license
            .spdx_id
            .filter(|value| value != "NOASSERTION")
            .or(license.name)
    });
    let RepositoryTarget {
        owner,
        repository,
        canonical_url,
        api_url,
    } = target;
    let full_name = metadata
        .full_name
        .unwrap_or_else(|| format!("{owner}/{repository}"));
    let html_url = metadata.html_url.unwrap_or_else(|| canonical_url.clone());
    Ok(PublicRepositoryAudit {
        url: canonical_url,
        api_url,
        owner,
        repository,
        full_name,
        html_url,
        description: metadata.description,
        default_branch: metadata.default_branch,
        license,
        archived: metadata.archived.unwrap_or(false),
        fork: metadata.fork.unwrap_or(false),
        retrieved_at: now_ms(),
        scope: "One public GitHub repository metadata response; source files were not fetched".into(),
        provenance: Provenance::Observed,
        limitations: vec![
            "Metadata describes the public repository only; it is not product, market or growth evidence.".into(),
            "No source files were cloned or executed, and no growthlab.yaml workspace was created.".into(),
            "Stars, traffic, conversion, adoption and ranking claims are intentionally not inferred.".into(),
            "A local configured repository import is required before implementation battles.".into(),
        ],
    })
}

pub fn markdown(audit: &PublicRepositoryAudit) -> String {
    let mut output = format!(
        "# Public repository review\n\n- Repository: `{}`\n- URL: {}\n- Default branch: {}\n- License: {}\n- Archived: {}\n- Fork: {}\n- Provenance: **{}**\n\n{}\n\n## Limits\n\n",
        audit.full_name,
        audit.html_url,
        audit.default_branch.as_deref().unwrap_or("Not reported"),
        audit.license.as_deref().unwrap_or("Not reported"),
        audit.archived,
        audit.fork,
        serde_json::to_string(&audit.provenance)
            .unwrap_or_else(|_| "\"OBSERVED\"".into())
            .trim_matches('"'),
        audit.description.as_deref().unwrap_or("No public description reported."),
    );
    for limitation in &audit.limitations {
        output.push_str(&format!("- {limitation}\n"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_url_rejects_non_github_credentials_and_tracking() {
        for value in [
            "http://github.com/acme/product",
            "https://person:secret@github.com/acme/product",
            "https://github.com/acme/product?tab=readme",
            "https://gitlab.com/acme/product",
            "https://github.com/acme/product.git",
            "https://github.com/acme",
        ] {
            assert!(target(value).is_err(), "accepted unsafe URL: {value}");
        }
        let parsed = target("https://www.github.com/acme/product/").unwrap();
        assert_eq!(parsed.canonical_url, "https://github.com/acme/product");
        assert_eq!(parsed.api_url, "https://api.github.com/repos/acme/product");
    }

    #[test]
    fn markdown_labels_metadata_as_observed_and_keeps_limits_visible() {
        let audit = PublicRepositoryAudit {
            url: "https://github.com/acme/product".into(),
            api_url: "https://api.github.com/repos/acme/product".into(),
            owner: "acme".into(),
            repository: "product".into(),
            full_name: "acme/product".into(),
            html_url: "https://github.com/acme/product".into(),
            description: Some("A local product".into()),
            default_branch: Some("main".into()),
            license: Some("MIT".into()),
            archived: false,
            fork: false,
            retrieved_at: 1,
            scope: "metadata".into(),
            provenance: Provenance::Observed,
            limitations: vec!["No source files were fetched.".into()],
        };
        let rendered = markdown(&audit);
        assert!(rendered.contains("acme/product"));
        assert!(rendered.contains("Provenance: **OBSERVED**"));
        assert!(rendered.contains("No source files were fetched."));
    }
}
