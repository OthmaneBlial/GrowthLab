//! Shareable summaries omit private evidence unless context export is explicit.
//! Prompts, raw logs, product names and paths are never included.
use super::archive;
use super::battle_model::ValidationRecord;
use super::evaluation::SeoRubric;
use super::model::{Confidence, Provenance};
use super::redaction::{contains_secret, redact};
use super::selection::{self, SelectionStatus};
use crate::error::{anyhow, Result};
use crate::store::{now_ms, Store};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Default)]
pub struct ReportOptions {
    pub include_context: bool,
    pub public_goal: Option<String>,
    pub without_attribution: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportCheck {
    pub label: String,
    pub status: String,
    pub exit_code: Option<i64>,
    pub termination: Option<String>,
    pub log_truncated: bool,
    pub source_commit: String,
    pub source_digest: String,
    pub started_at: i64,
    pub ended_at: i64,
    pub provenance: Provenance,
    pub confinement: Option<String>,
    pub confinement_policy_digest: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportVariant {
    pub number: usize,
    pub label: String,
    pub status: String,
    pub selected: bool,
    pub eligible: bool,
    pub passed_commands: usize,
    pub required_commands: usize,
    pub checks: Vec<ReportCheck>,
    pub implementation_provenance: Provenance,
    pub check_provenance: Provenance,
    pub outcome_provenance: Provenance,
    pub confidence: Confidence,
    pub archive_digest: Option<String>,
    pub candidate_commit: Option<String>,
    pub harness: Option<String>,
    pub files_changed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
    pub implementation_summary: Option<String>,
    pub rubric: Option<SeoRubric>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BattleReport {
    pub version: u32,
    pub battle_id: String,
    pub created_at: i64,
    pub goal: String,
    pub context_disclosed: bool,
    pub source_commit: String,
    pub source_digest: String,
    pub contract_digest: String,
    pub evaluator: String,
    pub calculation: String,
    pub limitations: Vec<String>,
    pub variants: Vec<ReportVariant>,
    pub selected_candidate: Option<usize>,
    pub selection_action: Option<String>,
    pub attribution: bool,
}
fn check(check: &ValidationRecord, index: usize, include_context: bool) -> ReportCheck {
    ReportCheck {
        label: if include_context {
            redact(&check.command)
        } else {
            format!("Command {:02}", index + 1)
        },
        status: check.status.clone(),
        exit_code: check.exit_code,
        termination: check.termination_reason.as_deref().map(|reason| {
            match reason {
                "timeout" => "Timeout",
                "cancelled" => "Cancelled",
                "log_limit" => "Log limit exceeded",
                _ => "Command failed or process crashed",
            }
            .into()
        }),
        log_truncated: check.log_truncated,
        source_commit: check.source_commit.clone(),
        source_digest: check.source_digest.clone(),
        started_at: check.started_at,
        ended_at: check.ended_at,
        provenance: check.provenance,
        confinement: check.confinement.as_ref().map(|record| {
            match record.backend.as_str() {
                "macos-seatbelt-v1" => "macOS filesystem restrictions; network denied",
                "linux-bubblewrap-v1" => {
                    "Linux filesystem/process namespaces; host network isolated"
                }
                _ => "Unrecognized confinement metadata; isolation is unverified",
            }
            .into()
        }),
        confinement_policy_digest: check.confinement.as_ref().and_then(|record| {
            (matches!(
                record.backend.as_str(),
                "macos-seatbelt-v1" | "linux-bubblewrap-v1"
            ) && record.policy_digest.len() == 64
                && record
                    .policy_digest
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit()))
            .then(|| record.policy_digest.clone())
        }),
    }
}
fn diff_stats(diff: &str) -> (usize, usize, usize) {
    let (mut files, mut added, mut removed) = (0, 0, 0);
    let mut in_hunk = false;
    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            files += 1;
            in_hunk = false;
        } else if line.starts_with("@@ ") {
            in_hunk = true;
        } else if in_hunk && line.starts_with('+') {
            added += 1;
        } else if in_hunk && line.starts_with('-') {
            removed += 1;
        }
    }
    (files, added, removed)
}
pub fn build(store: &Store, id: &str, options: &ReportOptions) -> Result<BattleReport> {
    if options
        .public_goal
        .as_ref()
        .is_some_and(|goal| goal.trim().is_empty() || goal.len() > 4096 || contains_secret(goal))
    {
        return Err(anyhow!(
            "Public report goal must be 1–4096 bytes without credentials"
        ));
    }
    let comparison = super::evaluation::compare(store, id)?;
    let battle = store
        .get_growth_battle(id)?
        .ok_or_else(|| anyhow!("Battle not found"))?;
    let sealed_runs = store.sealed_growth_runs(id)?;
    let selections = store.growth_selections(id)?;
    let selected = selections
        .iter()
        .rev()
        .find(|record| record.status == SelectionStatus::Done);
    if let Some(record) = selected {
        if !sealed_runs.iter().any(|sealed| {
            sealed.run.id == record.run_id
                && sealed.run.variant_id == record.variant_id
                && sealed.archive_digest == record.archive_digest
                && sealed.run.candidate_commit.as_ref() == Some(&record.candidate_commit)
        }) {
            return Err(anyhow!(
                "Selected candidate receipt does not match sealed report evidence"
            ));
        }
    }
    let mut variants = Vec::new();
    for (index, row) in comparison.rows.iter().enumerate() {
        let sealed = sealed_runs
            .iter()
            .find(|sealed| sealed.run.variant_id == row.variant_id);
        let files = sealed
            .map(|sealed| archive::verify(store.data_root(), &sealed.archive_digest))
            .transpose()?;
        let diff = files
            .as_ref()
            .and_then(|files| files.get("implementation.diff"))
            .map(|bytes| String::from_utf8_lossy(bytes))
            .unwrap_or_default();
        let run = sealed.map(|sealed| &sealed.run);
        let (files_changed, lines_added, lines_removed) = diff_stats(&diff);
        variants.push(ReportVariant {
            number: index + 1,
            label: if options.include_context {
                redact(&row.title)
            } else {
                format!("Variant {:02}", index + 1)
            },
            status: row.status.clone(),
            selected: selected.is_some_and(|selected| selected.variant_id == row.variant_id),
            eligible: row.eligible,
            passed_commands: row.passed_commands,
            required_commands: row.required_commands,
            checks: row
                .checks
                .iter()
                .enumerate()
                .map(|(index, record)| check(record, index, options.include_context))
                .collect(),
            implementation_provenance: row.implementation_provenance,
            check_provenance: row.check_provenance,
            outcome_provenance: row.outcome_provenance,
            confidence: row.confidence.clone(),
            archive_digest: row.archive_digest.clone(),
            candidate_commit: run.and_then(|run| run.candidate_commit.clone()),
            harness: run.map(|run| {
                match run.agent.harness.as_str() {
                    "declared-replay" | "replay" => "Declared replay",
                    "claude-code" => "Claude Code",
                    "codex" => "Codex",
                    "opencode" => "OpenCode",
                    "cursor" => "Cursor",
                    _ => "Native adapter",
                }
                .into()
            }),
            files_changed,
            lines_added,
            lines_removed,
            implementation_summary: if options.include_context {
                run.and_then(|run| run.implementation.as_ref())
                    .map(|implementation| redact(&implementation.summary))
            } else {
                None
            },
            rubric: row.rubric.clone(),
        });
    }
    Ok(BattleReport {
        version:1,battle_id:id.into(),created_at:now_ms(),
        goal:options.public_goal.clone().unwrap_or_else(|| if options.include_context {
            redact(&battle.contract.goal)
        } else {"Private product goal withheld".into()}),
        context_disclosed:options.include_context, source_commit:battle.contract.source_snapshot_commit,
        source_digest:battle.contract.source_snapshot_digest,contract_digest:battle.contract_digest,
        evaluator:comparison.evaluator,calculation:comparison.calculation,
        limitations:vec![
            "Command checks are observations on immutable source snapshots. They do not establish conversion lift, accessibility, performance or market validation.".into(),
            "Eligibility requires every configured command to pass on a successful sealed candidate. Ties require user review; this report contains no combined growth score.".into(),
            "Diff counts summarize the archived Git diff; binary changes are not line-counted. A sealed run may include a local desktop PNG render when a compatible browser was available; that image is not a visual quality result or measured growth outcome.".into(),
            "Private product metadata, prompts, raw logs and model identifiers are withheld. Explicit context export discloses goal, variant titles, summaries and commands; that text may contain identifying names or paths.".into(),
            "Replay proposals are declared simulations. Actual configured checks are observed; growth outcomes remain untested. Inspect each check's isolation metadata. Archived checks without it predate confinement; their host isolation is unverified. Resource quotas are not provided.".into(),
        ],
        selected_candidate:variants.iter().find(|variant|variant.selected).map(|variant|variant.number),
        selection_action:selected.map(|record|match record.action {
            selection::SelectionAction::Select=>"Selected for review",
            selection::SelectionAction::Apply=>"Copied to product working tree",
            selection::SelectionAction::Export=>"Exported as a local patch",
        }.into()), variants,attribution:!options.without_attribution,
    })
}
fn html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn md(text: &str) -> String {
    html(text)
        .replace('|', "&#124;")
        .replace('*', "&#42;")
        .replace('_', "&#95;")
        .replace(char::from(96), "&#96;")
        .replace('[', "&#91;")
        .replace(']', "&#93;")
        .replace('\\', "&#92;")
        .replace('\n', " ")
}
fn provenance(value: Provenance) -> &'static str {
    match value {
        Provenance::Measured => "MEASURED",
        Provenance::Observed => "OBSERVED",
        Provenance::Estimated => "ESTIMATED",
        Provenance::Simulated => "SIMULATED",
        Provenance::Untested => "UNTESTED",
    }
}
pub fn markdown(report: &BattleReport) -> String {
    let mut out = format!("# Growth Battle report\n\n{}\n\n**Recommended candidates**, not measured growth winners.\n\n",md(&report.goal));
    match report.selected_candidate {
        Some(number) => out.push_str(&format!(
            "Selected: **Variant {number:02}** — {}.\n\n",
            md(report
                .selection_action
                .as_deref()
                .unwrap_or("Selected for review"))
        )),
        None => out.push_str("No candidate has been selected.\n\n"),
    }
    out.push_str("| Variant | Status | Configured checks | SEO page hygiene | Eligible | Proposal | Checks | Outcome |\n|---|---|---|---|---|---|---|---|\n");
    for row in &report.variants {
        out.push_str(&format!(
            "| {} | {} | {} / {} | {} | {} | {} | {} | {} |\n",
            md(&row.label),
            md(&row.status),
            row.passed_commands,
            row.required_commands,
            row.rubric
                .as_ref()
                .map(|rubric| format!("{}/{} ESTIMATED", rubric.score, rubric.max_score))
                .unwrap_or_else(|| "—".into()),
            if row.eligible { "Yes" } else { "No" },
            provenance(row.implementation_provenance),
            provenance(row.check_provenance),
            provenance(row.outcome_provenance)
        ));
    }
    out.push_str(&format!(
        "\n## Evaluation\n\n{}\n\nEvaluator: {}.\n\n",
        md(&report.calculation),
        md(&report.evaluator)
    ));
    for row in &report.variants {
        out.push_str(&format!("## {}\n\n{} file changes; +{} / −{} text lines in the archived diff.\n\nConfidence: **Low** — {}.\n\n",
            md(&row.label),row.files_changed,row.lines_added,row.lines_removed,md(&row.confidence.rationale)));
        if let Some(summary) = &row.implementation_summary {
            out.push_str(&format!("{}\n\n", md(summary)));
        }
        if let Some(rubric) = &row.rubric {
            out.push_str(&format!(
                "SEO page hygiene: **{}/{}** (**ESTIMATED**). {}\n\n",
                rubric.score,
                rubric.max_score,
                md(&rubric.calculation)
            ));
            for dimension in &rubric.dimensions {
                out.push_str(&format!(
                    "- **{}**: {}/{} ({}) — {}\n",
                    md(&dimension.label),
                    dimension.score,
                    dimension.max_score,
                    md(&dimension.status),
                    dimension
                        .evidence
                        .iter()
                        .map(|evidence| md(evidence))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            out.push('\n');
            out.push_str("### Suggested next steps\n\n");
            if rubric.recommendations.is_empty() {
                out.push_str("- No structural gaps were found by this local rubric.\n\n");
            } else {
                for recommendation in &rubric.recommendations {
                    out.push_str(&format!("- {}\n", md(recommendation)));
                }
                out.push('\n');
            }
            out.push_str("### Limits\n\n");
            for limitation in &rubric.limitations {
                out.push_str(&format!("- {}\n", md(limitation)));
            }
            out.push('\n');
        }
        for check in &row.checks {
            out.push_str(&format!(
                "- **{}**: {} (exit {}), {}. UTC Unix milliseconds: {}–{}.{}\n",
                md(&check.label),
                md(&check.status),
                check
                    .exit_code
                    .map(|code| code.to_string())
                    .unwrap_or("unavailable".into()),
                provenance(check.provenance),
                check.started_at,
                check.ended_at,
                check
                    .termination
                    .as_ref()
                    .map(|reason| format!(" {}", md(reason)))
                    .unwrap_or_default()
            ));
            out.push_str(&format!(
                "  Isolation: {}. Policy SHA-256: {}.\n",
                md(check
                    .confinement
                    .as_deref()
                    .unwrap_or("Not recorded; host isolation is unverified")),
                md(check
                    .confinement_policy_digest
                    .as_deref()
                    .unwrap_or("unavailable"))
            ));
        }
        if row.checks.is_empty() {
            out.push_str("No command results recorded.\n");
        }
        out.push_str(&format!(
            "\nRun seal: {}. Candidate commit: {}.\n\n",
            row.archive_digest.as_deref().unwrap_or("not sealed"),
            row.candidate_commit.as_deref().unwrap_or("unavailable")
        ));
    }
    out.push_str(&format!("\n## Reproducibility\n\nBattle: {}  \nSource commit: {}  \nSource archive SHA-256: {}  \nContract SHA-256: {}  \nGenerated UTC Unix milliseconds: {}\n\n",
        report.battle_id,report.source_commit,report.source_digest,report.contract_digest,report.created_at));
    out.push_str("Verify the private local evidence with **growthlab compare &lt;battle-id&gt;**. This report is a summary, not the complete source/prompts/logs needed to rerun the product.\n\n## Limits and privacy\n\n");
    for limitation in &report.limitations {
        out.push_str(&format!("- {}\n", md(limitation)));
    }
    if report.attribution {
        out.push_str("\nBuilt with GrowthLab · Open source · Local first\n");
    }
    out
}
const REPORT_CSS: &str = include_str!("report.css");
pub fn document(report: &BattleReport) -> String {
    let selection = match report.selected_candidate {
        Some(number) => format!(
            "Variant {number:02} <span>{}</span>",
            html(
                report
                    .selection_action
                    .as_deref()
                    .unwrap_or("Selected for review")
            )
        ),
        None => "Selection open <span>Review the evidence and choose a candidate.</span>".into(),
    };
    let mut out = format!(
        r##"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'"><title>Growth Battle · Evidence report</title><link rel="icon" href="data:,"><style>{REPORT_CSS}</style></head><body><main>
<header><a class="wordmark" href="#comparison" aria-label="GrowthLab report, jump to comparison"><span class="monogram" aria-hidden="true">GL</span> GrowthLab</a><span class="edition">LOCAL EVIDENCE REPORT / 01</span></header>
<section class="intro"><div><p class="eyebrow">GROWTH BATTLE</p><h1>Three strategies.<br>Evidence you can inspect.</h1><p class="goal">{}</p></div><aside class="decision"><p class="eyebrow">SELECTED CANDIDATE</p><p class="selection">{selection}</p><p class="decision-limit">A decision to review or ship an implementation. Growth outcomes remain <strong>UNTESTED</strong>.</p></aside></section>
<div class="notice"><strong>Recommended candidates</strong><span>Configured checks are observations. They do not establish growth lift.</span></div>
<section id="comparison" aria-labelledby="comparison-title"><div class="section-heading"><h2 id="comparison-title">The competing approaches</h2><span>Expand checks to inspect their inputs</span></div><div class="variants">"##,
        html(&report.goal)
    );
    for row in &report.variants {
        let state = if row.eligible {
            "eligible"
        } else {
            "ineligible"
        };
        out.push_str(&format!(r#"<article class="variant {}"><div class="variant-top"><span class="number">{:02}</span><span class="state {state}">{}</span></div><h3>{}</h3><p class="status">Execution: <strong>{}</strong>{}</p><details class="checks"><summary><span>Configured checks</span><strong>{} <small>/ {}</small></strong></summary><div class="check-content"><p class="explanation">{}</p>"#,
            if row.selected {"is-selected"} else {""},row.number,if row.eligible {"Eligible candidate"} else {"Not eligible"},
            html(&row.label),html(&row.status),if row.selected {" · Selected"} else {""},row.passed_commands,row.required_commands,html(&report.calculation)));
        for check in &row.checks {
            out.push_str(&format!(r#"<div class="check"><p><strong>{}</strong><span>{} · exit {}</span></p><p>{}{} · {}</p><p class="timestamp">UTC Unix ms {}–{}</p><dl><dt>Commit</dt><dd>{}</dd><dt>Source SHA-256</dt><dd>{}</dd><dt>Isolation</dt><dd>{}</dd><dt>Policy SHA-256</dt><dd>{}</dd></dl></div>"#,
                html(&check.label),html(&check.status),check.exit_code.map(|code|code.to_string()).unwrap_or("unavailable".into()),
                html(check.termination.as_deref().unwrap_or("Recorded command result")),if check.log_truncated {" · log truncated"} else {""},
                provenance(check.provenance),check.started_at,check.ended_at,html(&check.source_commit),html(&check.source_digest),html(check.confinement.as_deref().unwrap_or("Not recorded; host isolation is unverified")),html(check.confinement_policy_digest.as_deref().unwrap_or("unavailable"))));
        }
        if row.checks.is_empty() {
            out.push_str("<p>No command results recorded.</p>");
        }
        let rubric = row.rubric.as_ref().map(|rubric| {
            let dimensions = rubric
                .dimensions
                .iter()
                .map(|dimension| {
                    format!(
                        "<li><strong>{}</strong> {}/{} · {}<span>{}</span></li>",
                        html(&dimension.label),
                        dimension.score,
                        dimension.max_score,
                        html(&dimension.status),
                        html(&dimension.evidence.join(" "))
                    )
                })
                .collect::<String>();
            let limitations = rubric
                .limitations
                .iter()
                .map(|limitation| format!("<li>{}</li>", html(limitation)))
                .collect::<String>();
            let recommendations = if rubric.recommendations.is_empty() {
                "<li>No structural gaps were found by this local rubric.</li>".into()
            } else {
                rubric
                    .recommendations
                    .iter()
                    .map(|recommendation| format!("<li>{}</li>", html(recommendation)))
                    .collect::<String>()
            };
            format!(
                r#"<details class="rubric"><summary><span>{}</span><strong>{}/{} · {}</strong></summary><p>{}</p><ul>{}</ul><p class="rubric-recommendations">Suggested next steps</p><ul>{}</ul><p class="rubric-limits">{}</p><ul>{}</ul></details>"#,
                html(&rubric.label),
                rubric.score,
                rubric.max_score,
                provenance(rubric.provenance),
                html(&rubric.calculation),
                dimensions,
                recommendations,
                "Limits",
                limitations
            )
        }).unwrap_or_default();
        out.push_str(&format!(r#"</div></details>{}<dl class="provenance"><dt>Proposal</dt><dd>{}</dd><dt>Checks</dt><dd>{}</dd><dt>Growth outcome</dt><dd>{}</dd></dl><p class="diff-summary"><strong>{}</strong> files changed <span>+{} / −{} text lines</span></p>"#,
            rubric,provenance(row.implementation_provenance),provenance(row.check_provenance),provenance(row.outcome_provenance),row.files_changed,row.lines_added,row.lines_removed));
        if let Some(summary) = &row.implementation_summary {
            out.push_str(&format!(
                "<p class=\"implementation\">{}</p>",
                html(summary)
            ));
        }
        out.push_str(&format!(r#"<p class="confidence"><strong>Low confidence</strong>{}</p><details class="seal"><summary>Inspect run seal</summary><dl><dt>Run SHA-256</dt><dd>{}</dd><dt>Candidate commit</dt><dd>{}</dd><dt>Adapter</dt><dd>{}</dd></dl></details></article>"#,
            html(&row.confidence.rationale),html(row.archive_digest.as_deref().unwrap_or("Not sealed")),html(row.candidate_commit.as_deref().unwrap_or("Unavailable")),html(row.harness.as_deref().unwrap_or("Not executed"))));
    }
    out.push_str(r#"</div></section><section class="method"><div><p class="eyebrow">HOW TO READ THIS REPORT</p><h2>Inspect the method.<br>Keep the uncertainty.</h2><p>Every candidate faces the same frozen contract. Passing it establishes what the commands checked, while product outcomes still need real evidence.</p></div><div><h3>Limits &amp; privacy</h3><ul>"#);
    for limitation in &report.limitations {
        out.push_str(&format!("<li>{}</li>", html(limitation)));
    }
    out.push_str(&format!(r#"</ul></div></section><details class="repro"><summary>Reproducibility record</summary><dl><dt>Battle</dt><dd>{}</dd><dt>Source commit</dt><dd>{}</dd><dt>Source SHA-256</dt><dd>{}</dd><dt>Contract SHA-256</dt><dd>{}</dd><dt>Evaluator</dt><dd>{}</dd><dt>Generated UTC Unix ms</dt><dd>{}</dd></dl><p>Verify private local evidence with <code>growthlab compare &lt;battle-id&gt;</code>. This report is a summary; source, prompts and raw logs are withheld.</p></details>"#,
        html(&report.battle_id),html(&report.source_commit),html(&report.source_digest),html(&report.contract_digest),html(&report.evaluator),report.created_at));
    if report.attribution {
        out.push_str("<footer><span>Built with <strong>GrowthLab</strong></span><span>Open source · Local first · Evidence before confidence</span></footer>");
    }
    out.push_str("</main></body></html>");
    out
}
pub fn export(
    store: &Store,
    id: &str,
    options: &ReportOptions,
    output: &Path,
    as_markdown: bool,
) -> Result<std::path::PathBuf> {
    let report = build(store, id, options)?;
    let battle = store
        .get_growth_battle(id)?
        .ok_or_else(|| anyhow!("Battle not found"))?;
    selection::refuse_product_output(store, &battle, output)?;
    let text = if as_markdown {
        markdown(&report)
    } else {
        document(&report)
    };
    selection::write_new_file(output, text.as_bytes())
}
