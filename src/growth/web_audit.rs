//! Read-only public website analysis with conservative access boundaries.
//!
//! The fetcher performs one robots.txt request and one page request. It follows
//! no redirects, sends no credentials, and refuses non-HTTPS URLs. The page is
//! passed to the same structural SEO, page-quality and accessibility rubrics used for local
//! candidates; the result remains ESTIMATED and never represents ranking or
//! traffic evidence.

use futures::StreamExt;
use reqwest::{Client, StatusCode, Url};
use serde::Serialize;

use crate::error::{anyhow, Result};
use crate::store::now_ms;

use super::evaluation::{
    accessibility_rubric, page_quality_rubric, seo_rubric, PageQualityRubric, RenderRubric,
    SeoRubric,
};
use super::model::Provenance;

const MAX_HTML_BYTES: usize = 4 * 1024 * 1024;
const MAX_ROBOTS_BYTES: usize = 512 * 1024;
const REQUEST_TIMEOUT_SECONDS: u64 = 15;
const USER_AGENT: &str = "GrowthLab-public-audit/0.1 (+https://github.com/OthmaneBlial/GrowthLab)";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RobotsReview {
    pub url: String,
    pub status: u16,
    pub allowed: bool,
    pub matched_rule: Option<String>,
    pub retrieved_at: i64,
    pub limitation: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSeoAudit {
    pub url: String,
    pub retrieved_at: i64,
    pub http_status: u16,
    pub content_type: Option<String>,
    pub robots: RobotsReview,
    pub scope: String,
    pub provenance: Provenance,
    pub rubric: SeoRubric,
    pub quality: PageQualityRubric,
    pub accessibility: RenderRubric,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone)]
struct RobotsRules {
    disallow: Vec<String>,
    allow: Vec<String>,
}

fn public_url(raw: &str) -> Result<Url> {
    let url = Url::parse(raw.trim()).map_err(|_| anyhow!("Website URL is invalid"))?;
    if url.scheme() != "https" {
        return Err(anyhow!("Website analysis accepts HTTPS URLs only"));
    }
    if url.host_str().is_none() || url.username() != "" || url.password().is_some() {
        return Err(anyhow!(
            "Website URL must have a public host and cannot contain credentials"
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(anyhow!(
            "Website URL cannot contain a query string or fragment"
        ));
    }
    if url.port().is_some_and(|port| port != 443) {
        return Err(anyhow!(
            "Website analysis accepts the default HTTPS port only"
        ));
    }
    if let Some(host) = url.host_str() {
        let lower = host.to_ascii_lowercase();
        if matches!(lower.as_str(), "localhost" | "localhost.localdomain")
            || lower.ends_with(".local")
            || lower.ends_with(".internal")
        {
            return Err(anyhow!("Private or local website hosts are not allowed"));
        }
        if let Ok(address) = host.parse::<std::net::IpAddr>() {
            let private = match address {
                std::net::IpAddr::V4(address) => {
                    address.is_private()
                        || address.is_loopback()
                        || address.is_link_local()
                        || address.is_unspecified()
                        || address.is_broadcast()
                        || address.is_documentation()
                }
                std::net::IpAddr::V6(address) => {
                    address.is_loopback() || address.is_unspecified() || address.is_unique_local()
                }
            };
            if private {
                return Err(anyhow!("Private or local website hosts are not allowed"));
            }
        }
    }
    Ok(url)
}

fn rules(body: &str) -> RobotsRules {
    let mut active = false;
    let mut saw_user_agent = false;
    let mut disallow = Vec::new();
    let mut allow = Vec::new();
    for line in body.lines() {
        let line = line.split_once('#').map_or(line, |(value, _)| value).trim();
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match key.as_str() {
            "user-agent" => {
                saw_user_agent = true;
                active = value == "*";
            }
            "disallow" if active && !value.is_empty() => disallow.push(value.to_string()),
            "allow" if active && !value.is_empty() => allow.push(value.to_string()),
            _ => {}
        }
    }
    if !saw_user_agent {
        return RobotsRules {
            disallow: Vec::new(),
            allow: Vec::new(),
        };
    }
    RobotsRules { disallow, allow }
}

fn rule_matches(path: &str, rule: &str) -> bool {
    // A wildcard suffix is treated as its literal prefix. This is conservative
    // for access control: an unsupported robots extension can only narrow a
    // request, never widen it.
    let prefix = rule.split('*').next().unwrap_or(rule);
    path.starts_with(prefix)
}

fn robots_decision(path: &str, rules: &RobotsRules) -> (bool, Option<String>) {
    let denied = rules
        .disallow
        .iter()
        .filter(|rule| rule_matches(path, rule))
        .max_by_key(|rule| rule.len());
    let allowed = rules
        .allow
        .iter()
        .filter(|rule| rule_matches(path, rule))
        .max_by_key(|rule| rule.len());
    match (denied, allowed) {
        (Some(denied), Some(allowed)) if allowed.len() >= denied.len() => {
            (true, Some(format!("Allow: {allowed}")))
        }
        (Some(denied), _) => (false, Some(format!("Disallow: {denied}"))),
        (None, _) => (true, None),
    }
}

async fn bounded_body(response: reqwest::Response, limit: usize, what: &str) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(anyhow!("{what} response exceeds the {limit}-byte limit"));
    }
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| anyhow!("Could not read the {what} response"))?;
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(anyhow!("{what} response exceeds the {limit}-byte limit"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn client() -> Result<Client> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|_| anyhow!("Could not create the read-only website client"))
}

async fn robots(client: &Client, target: &Url) -> Result<RobotsReview> {
    let mut url = target.clone();
    url.set_path("/robots.txt");
    url.set_query(None);
    url.set_fragment(None);
    let retrieved_at = now_ms();
    let response = client
        .get(url.clone())
        .header(reqwest::header::ACCEPT, "text/plain")
        .send()
        .await
        .map_err(|_| anyhow!("Could not read robots.txt; website analysis was refused"))?;
    let status = response.status();
    if status == StatusCode::NOT_FOUND {
        return Ok(RobotsReview {
            url: url.to_string(),
            status: status.as_u16(),
            allowed: true,
            matched_rule: None,
            retrieved_at,
            limitation: "robots.txt was not found; only this single page request will be made."
                .into(),
        });
    }
    if !status.is_success() {
        return Err(anyhow!(
            "robots.txt returned HTTP {}; website analysis was refused",
            status.as_u16()
        ));
    }
    let body = bounded_body(response, MAX_ROBOTS_BYTES, "robots.txt").await?;
    let text = String::from_utf8(body)
        .map_err(|_| anyhow!("robots.txt is not UTF-8; website analysis was refused"))?;
    let rules = rules(&text);
    let path = target.path();
    let (allowed, matched_rule) = robots_decision(path, &rules);
    if !allowed {
        return Ok(RobotsReview {
            url: url.to_string(),
            status: status.as_u16(),
            allowed,
            matched_rule,
            retrieved_at,
            limitation:
                "The page is disallowed for the wildcard user-agent; no page request was made."
                    .into(),
        });
    }
    Ok(RobotsReview {
        url: url.to_string(),
        status: status.as_u16(),
        allowed,
        matched_rule,
        retrieved_at,
        limitation: "One page request is permitted; no crawl or repeated fetch is performed."
            .into(),
    })
}

pub async fn fetch(raw: &str) -> Result<RemoteSeoAudit> {
    let url = public_url(raw)?;
    let client = client()?;
    let robots = robots(&client, &url).await?;
    if !robots.allowed {
        return Err(anyhow!(
            "Website analysis is disallowed by robots.txt ({})",
            robots.matched_rule.as_deref().unwrap_or("unknown rule")
        ));
    }
    let response = client
        .get(url.clone())
        .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml")
        .send()
        .await
        .map_err(|_| anyhow!("Could not fetch the public website page"))?;
    let status = response.status();
    if status.is_redirection() {
        return Err(anyhow!(
            "Website redirects are not followed; analyze the final HTTPS URL explicitly"
        ));
    }
    if !status.is_success() {
        return Err(anyhow!(
            "Website page returned HTTP {}; analysis was refused",
            status.as_u16()
        ));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if content_type.as_deref().is_some_and(|value| {
        let value = value.to_ascii_lowercase();
        !value.contains("text/html") && !value.contains("application/xhtml+xml")
    }) {
        return Err(anyhow!("Website URL did not return an HTML document"));
    }
    let html = String::from_utf8(bounded_body(response, MAX_HTML_BYTES, "HTML").await?)
        .map_err(|_| anyhow!("Website HTML is not UTF-8; analysis was refused"))?;
    let retrieved_at = now_ms();
    Ok(RemoteSeoAudit {
        url: url.to_string(),
        retrieved_at,
        http_status: status.as_u16(),
        content_type,
        robots,
        scope: "One public HTTPS page fetched read-only after a same-origin robots.txt check; no redirects, credentials or crawl are used.".into(),
        provenance: Provenance::Estimated,
        rubric: seo_rubric(&html),
        quality: page_quality_rubric(&html),
        accessibility: accessibility_rubric(&html)
            .ok_or_else(|| anyhow!("Website HTML was empty; accessibility review was refused"))?,
        limitations: vec![
            "The structural rubric is ESTIMATED and reviews returned HTML only.".into(),
            "No ranking, traffic, backlink, search-demand, accessibility certification, performance or conversion result is measured.".into(),
            "The page may change after retrieval; the timestamp records this single observation.".into(),
        ],
    })
}

pub fn markdown(audit: &RemoteSeoAudit) -> String {
    let mut output = format!(
        "# Public website SEO review\n\n- URL: `{}`\n- Retrieved: `{}`\n- HTTP status: `{}`\n- Provenance: **{}**\n- Scope: {}\n\n",
        audit.url,
        audit.retrieved_at,
        audit.http_status,
        match audit.provenance {
            Provenance::Estimated => "ESTIMATED",
            Provenance::Measured => "MEASURED",
            Provenance::Observed => "OBSERVED",
            Provenance::Simulated => "SIMULATED",
            Provenance::Untested => "UNTESTED",
        },
        audit.scope
    );
    output.push_str("## robots.txt\n\n");
    output.push_str(&format!(
        "- Checked: `{}`\n- Allowed: `{}`\n- Rule: `{}`\n- {}\n\n",
        audit.robots.url,
        audit.robots.allowed,
        audit.robots.matched_rule.as_deref().unwrap_or("none"),
        audit.robots.limitation
    ));
    output.push_str(&format!(
        "## Structural rubric\n\n**{} / {}** — {}\n\n| Dimension | Score | Status | Evidence |\n| --- | ---: | --- | --- |\n",
        audit.rubric.score, audit.rubric.max_score, audit.rubric.label
    ));
    for dimension in &audit.rubric.dimensions {
        output.push_str(&format!(
            "| {} | {}/{} | {} | {} |\n",
            dimension.label,
            dimension.score,
            dimension.max_score,
            dimension.status,
            dimension.evidence.join(" ").replace('|', "\\|")
        ));
    }
    output.push_str(&format!(
        "\n## Page quality hints\n\n**{} / {}** — {}\n\n| Dimension | Score | Status | Evidence |\n| --- | ---: | --- | --- |\n",
        audit.quality.score, audit.quality.max_score, audit.quality.label
    ));
    for dimension in &audit.quality.dimensions {
        output.push_str(&format!(
            "| {} | {}/{} | {} | {} |\n",
            dimension.label,
            dimension.score,
            dimension.max_score,
            dimension.status,
            dimension.evidence.join(" ").replace('|', "\\|")
        ));
    }
    output.push_str(&format!("\n{}\n", audit.quality.calculation));
    if !audit.quality.recommendations.is_empty() {
        output.push_str("\n### Quality-hint next steps\n\n");
        for recommendation in &audit.quality.recommendations {
            output.push_str(&format!("- {recommendation}\n"));
        }
    }
    output.push_str("\n### Quality-hint limits\n\n");
    for limitation in &audit.quality.limitations {
        output.push_str(&format!("- {limitation}\n"));
    }
    output.push_str(&format!(
        "\n## Accessibility structure hints\n\n**{} / {}** — {}\n\n| Dimension | Score | Status | Evidence |\n| --- | ---: | --- | --- |\n",
        audit.accessibility.score, audit.accessibility.max_score, audit.accessibility.label
    ));
    for dimension in &audit.accessibility.dimensions {
        output.push_str(&format!(
            "| {} | {}/{} | {} | {} |\n",
            dimension.label,
            dimension.score,
            dimension.max_score,
            dimension.status,
            dimension.evidence.join(" ").replace('|', "\\|")
        ));
    }
    output.push_str(&format!("\n{}\n", audit.accessibility.calculation));
    if !audit.accessibility.recommendations.is_empty() {
        output.push_str("\n### Accessibility next steps\n\n");
        for recommendation in &audit.accessibility.recommendations {
            output.push_str(&format!("- {recommendation}\n"));
        }
    }
    output.push_str("\n### Accessibility limits\n\n");
    for limitation in &audit.accessibility.limitations {
        output.push_str(&format!("- {limitation}\n"));
    }
    output.push_str("\n## Limits\n\n");
    for limitation in &audit.limitations {
        output.push_str(&format!("- {limitation}\n"));
    }
    output.push_str("\nThis is a read-only structural review. It does not claim rankings, traffic, conversions or SEO performance.\n");
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_url_rejects_credentials_local_hosts_and_tracking_parameters() {
        for url in [
            "http://example.com/",
            "https://user:pass@example.com/",
            "https://localhost/",
            "https://127.0.0.1/",
            "https://example.com/?utm_source=test",
            "https://example.com/#section",
            "https://example.com:8443/",
        ] {
            assert!(public_url(url).is_err(), "accepted unsafe URL: {url}");
        }
        assert!(public_url("https://example.com/pricing").is_ok());
    }

    #[test]
    fn robots_rules_honor_longest_allow_and_disallow_prefixes() {
        let parsed =
            rules("User-agent: *\nDisallow: /private\nAllow: /private/public\nDisallow: /tmp*\n");
        assert!(!robots_decision("/private", &parsed).0);
        assert!(robots_decision("/private/public", &parsed).0);
        assert!(!robots_decision("/tmp/report", &parsed).0);
        assert!(robots_decision("/docs", &parsed).0);
    }
}
