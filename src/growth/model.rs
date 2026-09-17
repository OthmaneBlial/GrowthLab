use serde::{Deserialize, Serialize};

use super::config::GrowthConfig;
use crate::error::{anyhow, Result};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Provenance {
    Measured,
    Observed,
    Estimated,
    Simulated,
    Untested,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceLabel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Confidence {
    pub label: ConfidenceLabel,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Reject,
    Revise,
    Candidate,
    Ship,
    Inconclusive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationMode {
    Proxy,
    Live,
    Replay,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Strategist,
    Researcher,
    Positioning,
    Conversion,
    Seo,
    Onboarding,
    Pricing,
    Launch,
    Evaluator,
    Skeptic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrowthWorkspace {
    /// Same ID as the inherited LocalProject.
    pub project_id: String,
    pub config: GrowthConfig,
    pub source_snapshot_commit: String,
    pub created_at: i64,
}

impl GrowthWorkspace {
    pub fn validate(&self) -> Result<()> {
        self.config.validate()?;
        if self.project_id.trim().is_empty() {
            return Err(anyhow!("Workspace requires a stable project ID"));
        }
        validate_commit(&self.source_snapshot_commit)
    }
}

fn validate_commit(commit: &str) -> Result<()> {
    if !matches!(commit.len(), 40 | 64) || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(anyhow!("A full source snapshot commit is required"));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceRecord {
    pub id: String,
    pub title: String,
    pub source: String,
    pub retrieved_at: i64,
    pub observation: String,
    pub publisher: Option<String>,
    pub evidence_type: String,
    pub supports_claims: Vec<String>,
    pub challenges_claims: Vec<String>,
    pub provenance: Provenance,
    pub confidence: Confidence,
    pub limitations: String,
}

impl EvidenceRecord {
    /// Validate the minimum provenance needed to make an evidence record
    /// inspectable. Evidence is user or external input, so credential-shaped
    /// text is refused before it can be persisted in a hypothesis/archive.
    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty()
            || self.title.trim().is_empty()
            || self.source.trim().is_empty()
            || self.observation.trim().is_empty()
            || self.evidence_type.trim().is_empty()
            || self.limitations.trim().is_empty()
            || self.confidence.rationale.trim().is_empty()
            || self
                .publisher
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
        {
            return Err(anyhow!(
                "Evidence requires an id, source, observation, type, limitations and confidence rationale"
            ));
        }
        if self.retrieved_at <= 0 {
            return Err(anyhow!("Evidence retrieval time must be positive"));
        }
        if self
            .supports_claims
            .iter()
            .chain(self.challenges_claims.iter())
            .next()
            .is_none()
            || self
                .supports_claims
                .iter()
                .chain(self.challenges_claims.iter())
                .any(|claim| claim.trim().is_empty())
        {
            return Err(anyhow!(
                "Evidence must support or challenge at least one non-empty claim"
            ));
        }
        for value in [
            self.title.as_str(),
            self.source.as_str(),
            self.observation.as_str(),
            self.evidence_type.as_str(),
            self.limitations.as_str(),
            self.confidence.rationale.as_str(),
            self.publisher.as_deref().unwrap_or_default(),
        ] {
            if super::redaction::contains_secret(value) {
                return Err(anyhow!(
                    "Evidence contains a possible credential; redact it before recording the source"
                ));
            }
        }
        for claim in self
            .supports_claims
            .iter()
            .chain(self.challenges_claims.iter())
        {
            if super::redaction::contains_secret(claim) {
                return Err(anyhow!(
                    "Evidence contains a possible credential; redact it before recording the source"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrowthHypothesis {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub goal: String,
    pub audience: String,
    pub channel: String,
    pub role: AgentRole,
    pub hypothesis: String,
    pub mechanism: String,
    pub baseline_definition: String,
    pub primary_metric: String,
    pub guardrail_metrics: Vec<String>,
    /// Explicitly absent until the user defines a measurable threshold.
    pub success_threshold: Option<String>,
    pub evaluation_mode: EvaluationMode,
    pub source_snapshot_commit: String,
    pub evidence: Vec<EvidenceRecord>,
    pub risks: Vec<String>,
    pub provenance: Provenance,
    pub confidence: Confidence,
    pub decision: Decision,
    pub created_at: i64,
}

impl GrowthHypothesis {
    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty()
            || self.project_id.trim().is_empty()
            || self.title.trim().is_empty()
            || self.hypothesis.trim().is_empty()
            || self.goal.trim().is_empty()
            || self.audience.trim().is_empty()
            || self.primary_metric.trim().is_empty()
            || self.mechanism.trim().is_empty()
            || self.baseline_definition.trim().is_empty()
            || self.channel.trim().is_empty()
            || self.confidence.rationale.trim().is_empty()
        {
            return Err(anyhow!(
                "Hypothesis context, metric, baseline and confidence rationale are required"
            ));
        }
        validate_commit(&self.source_snapshot_commit)?;
        for evidence in &self.evidence {
            evidence.validate()?;
        }
        if self.provenance == Provenance::Untested && self.decision == Decision::Ship {
            return Err(anyhow!("An untested hypothesis cannot be marked ship"));
        }
        Ok(())
    }
}

/// Deterministic starter portfolio. These are template proposals grounded only
/// in the user's configured context; no agent or external research is claimed.
pub fn starter_hypotheses(workspace: &GrowthWorkspace) -> Vec<GrowthHypothesis> {
    let config = &workspace.config;
    [
        ("Outcome-first positioning", AgentRole::Positioning,
         "Describe the first useful outcome in the hero and pair it with one clear action.",
         "A concrete outcome may make the product easier to understand before a visitor commits.",
         "Copy clarity does not establish qualified conversion lift."),
        ("Proof beside the promise", AgentRole::Conversion,
         "Place a verifiable product demonstration beside the primary call to action.",
         "Showing actual behavior may reduce uncertainty about the value proposition.",
         "Only product facts supplied or directly inspected may be used as proof."),
        ("Faster first success", AgentRole::Onboarding,
         "Provide a short, executable quick start next to the primary action.",
         "Reducing the steps to try the product may help qualified visitors reach first value.",
         "A shorter path may sacrifice context or attract less qualified visitors."),
    ].into_iter().map(|(title, role, hypothesis, mechanism, risk)| GrowthHypothesis {
        id: uuid::Uuid::new_v4().to_string(), project_id: workspace.project_id.clone(),
        title: title.into(), goal: config.goal.primary.clone(), audience: config.product.audience.clone(),
        channel: "landing_page".into(), role, hypothesis: hypothesis.into(), mechanism: mechanism.into(),
        baseline_definition: "Current product at the recorded source snapshot; no traffic baseline supplied.".into(),
        primary_metric: config.metrics.primary.clone(), guardrail_metrics: config.metrics.guardrails.clone(),
        success_threshold: None, evaluation_mode: EvaluationMode::Proxy,
        source_snapshot_commit: workspace.source_snapshot_commit.clone(),
        evidence: vec![EvidenceRecord {
            id: uuid::Uuid::new_v4().to_string(), title: "Committed product brief".into(),
            source: format!("growthlab.yaml@{}", workspace.source_snapshot_commit),
            retrieved_at: crate::store::now_ms(),
            observation: format!("Configured audience: {}. Configured goal: {}.", config.product.audience, config.goal.primary),
            publisher: Some("User-provided context".into()), evidence_type: "user_input".into(),
            supports_claims: vec!["configured_audience".into(), "configured_goal".into()], challenges_claims: Vec::new(),
            provenance: Provenance::Observed,
            confidence: Confidence { label: ConfidenceLabel::Medium, rationale: "Directly recorded committed user input; no independent market evidence is claimed.".into() },
            limitations: "Supports the configured brief, not product-market fit, the causal hypothesis, or any outcome lift.".into(),
        }],
        risks: vec![risk.into(), "External research and outcome telemetry are pending.".into()],
        provenance: Provenance::Untested,
        confidence: Confidence { label: ConfidenceLabel::Low, rationale: "Starter template based on user-provided context; the proposed mechanism has not been tested.".into() },
        decision: Decision::Inconclusive, created_at: crate::store::now_ms(),
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provenance_is_required_and_templates_do_not_claim_execution() {
        let workspace = GrowthWorkspace {
            project_id: "p".into(),
            config: super::super::config::fixture(),
            source_snapshot_commit: "a".repeat(40),
            created_at: 1,
        };
        let hypotheses = starter_hypotheses(&workspace);
        assert_eq!(hypotheses.len(), 3);
        for hypothesis in hypotheses {
            hypothesis.validate().unwrap();
            assert_eq!(hypothesis.provenance, Provenance::Untested);
            assert_eq!(hypothesis.decision, Decision::Inconclusive);
            assert_eq!(hypothesis.evidence.len(), 1);
            assert_eq!(hypothesis.evidence[0].evidence_type, "user_input");
            assert_eq!(hypothesis.evidence[0].provenance, Provenance::Observed);
            let mut json = serde_json::to_value(hypothesis).unwrap();
            json.as_object_mut().unwrap().remove("provenance");
            assert!(serde_json::from_value::<GrowthHypothesis>(json).is_err());
        }
        assert_eq!(
            serde_json::to_string(&Provenance::Estimated).unwrap(),
            "\"ESTIMATED\""
        );
    }

    #[test]
    fn hypothesis_validation_rejects_incomplete_evidence() {
        let workspace = GrowthWorkspace {
            project_id: "p".into(),
            config: super::super::config::fixture(),
            source_snapshot_commit: "a".repeat(40),
            created_at: 1,
        };
        let mut hypothesis = starter_hypotheses(&workspace).remove(0);
        hypothesis.evidence[0].observation.clear();
        let error = hypothesis.validate().unwrap_err().to_string();
        assert!(
            error.contains("Evidence requires"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn hypothesis_validation_rejects_evidence_without_claim_links() {
        let workspace = GrowthWorkspace {
            project_id: "p".into(),
            config: super::super::config::fixture(),
            source_snapshot_commit: "a".repeat(40),
            created_at: 1,
        };
        let mut hypothesis = starter_hypotheses(&workspace).remove(0);
        hypothesis.evidence[0].supports_claims.clear();
        let error = hypothesis.validate().unwrap_err().to_string();
        assert!(
            error.contains("at least one non-empty claim"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn hypothesis_validation_rejects_blank_evidence_claim_links() {
        let workspace = GrowthWorkspace {
            project_id: "p".into(),
            config: super::super::config::fixture(),
            source_snapshot_commit: "a".repeat(40),
            created_at: 1,
        };
        let mut hypothesis = starter_hypotheses(&workspace).remove(0);
        hypothesis.evidence[0].supports_claims = vec!["   ".into()];
        let error = hypothesis.validate().unwrap_err().to_string();
        assert!(
            error.contains("at least one non-empty claim"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn hypothesis_validation_rejects_credential_shaped_evidence() {
        let workspace = GrowthWorkspace {
            project_id: "p".into(),
            config: super::super::config::fixture(),
            source_snapshot_commit: "a".repeat(40),
            created_at: 1,
        };
        let mut hypothesis = starter_hypotheses(&workspace).remove(0);
        hypothesis.evidence[0].observation = "API_KEY=fixture-private-value".into();
        let error = hypothesis.validate().unwrap_err().to_string();
        assert!(
            error.contains("possible credential"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn hypothesis_validation_rejects_credential_shaped_confidence_rationale() {
        let workspace = GrowthWorkspace {
            project_id: "p".into(),
            config: super::super::config::fixture(),
            source_snapshot_commit: "a".repeat(40),
            created_at: 1,
        };
        let mut hypothesis = starter_hypotheses(&workspace).remove(0);
        hypothesis.evidence[0].confidence.rationale = "token=fixture-private-value".into();
        let error = hypothesis.validate().unwrap_err().to_string();
        assert!(
            error.contains("possible credential"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn hypothesis_validation_rejects_credential_shaped_claims() {
        let workspace = GrowthWorkspace {
            project_id: "p".into(),
            config: super::super::config::fixture(),
            source_snapshot_commit: "a".repeat(40),
            created_at: 1,
        };
        let mut hypothesis = starter_hypotheses(&workspace).remove(0);
        hypothesis.evidence[0].supports_claims = vec!["password=fixture-private-value".into()];
        let error = hypothesis.validate().unwrap_err().to_string();
        assert!(
            error.contains("possible credential"),
            "unexpected error: {error}"
        );
    }
}
