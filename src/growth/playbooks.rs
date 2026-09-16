//! Growth playbook templates.
//!
//! These are reusable prompts and review contracts, not market research or
//! outcome claims. They give every role the same explicit questions, outputs
//! and safety boundaries before a user authorizes an experiment.

use serde::{Deserialize, Serialize};

use super::{
    config::PermissionMode,
    model::{AgentRole, Confidence, ConfidenceLabel, GrowthWorkspace, Provenance},
};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GrowthPlaybook {
    pub id: &'static str,
    pub title: &'static str,
    pub role: AgentRole,
    pub focus: &'static str,
    pub summary: &'static str,
    pub questions: &'static [&'static str],
    pub outputs: &'static [&'static str],
    pub guardrails: &'static [&'static str],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaybookResponse {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaybookRun {
    pub id: String,
    pub project_id: String,
    pub role: AgentRole,
    pub title: String,
    pub responses: Vec<PlaybookResponse>,
    pub outputs: Vec<String>,
    pub guardrails: Vec<String>,
    pub next_step: String,
    pub provenance: Provenance,
    pub confidence: Confidence,
    pub created_at: i64,
}

impl PlaybookRun {
    pub fn validate(&self) -> crate::error::Result<()> {
        if self.id.trim().is_empty()
            || self.project_id.trim().is_empty()
            || self.title.trim().is_empty()
            || self.responses.is_empty()
            || self.outputs.is_empty()
            || self.guardrails.is_empty()
            || self.next_step.trim().is_empty()
            || self.confidence.rationale.trim().is_empty()
        {
            return Err(crate::error::anyhow!(
                "Playbook run requires context, questions, outputs and guardrails"
            ));
        }
        if self.provenance != Provenance::Untested {
            return Err(crate::error::anyhow!(
                "Playbook templates remain UNTESTED until a user supplies outcome evidence"
            ));
        }
        Ok(())
    }
}

/// Execute one deterministic role contract against a workspace brief. This is
/// an inspectable local artifact; it makes no provider or analytics request.
pub fn execute(
    workspace: &GrowthWorkspace,
    role_id: &str,
    answers: &[String],
) -> crate::error::Result<PlaybookRun> {
    let item = catalog()
        .into_iter()
        .find(|item| item.id == role_id)
        .ok_or_else(|| crate::error::anyhow!("Unknown growth playbook role"))?;
    if workspace.config.permissions.mode == PermissionMode::Implement
        && workspace.config.validation.commands.is_empty()
    {
        return Err(crate::error::anyhow!(
            "Workspace implementation mode has no validation contract"
        ));
    }
    if answers.len() > item.questions.len() {
        return Err(crate::error::anyhow!(
            "A playbook accepts at most one answer per question"
        ));
    }
    let responses = item
        .questions
        .iter()
        .enumerate()
        .map(|(index, question)| {
            let answer = answers
                .get(index)
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .unwrap_or("Unknown — supply product evidence before making a claim.");
            PlaybookResponse {
                question: (*question).into(),
                answer: answer.into(),
            }
        })
        .collect();
    let run = PlaybookRun {
        id: uuid::Uuid::new_v4().to_string(),
        project_id: workspace.project_id.clone(),
        role: item.role,
        title: item.title.into(),
        responses,
        outputs: item.outputs.iter().map(|value| (*value).into()).collect(),
        guardrails: item.guardrails.iter().map(|value| (*value).into()).collect(),
        next_step: format!("Review the {} outputs, add evidence, then decide whether to prepare a bounded experiment.", item.focus.to_ascii_lowercase()),
        provenance: Provenance::Untested,
        confidence: Confidence { label: ConfidenceLabel::Low, rationale: "Deterministic role contract grounded in the configured product brief; no provider, market or outcome evidence was supplied.".into() },
        created_at: crate::store::now_ms(),
    };
    run.validate()?;
    Ok(run)
}

/// The initial role catalog exposed by the local GrowthLab API.
///
/// Keep each entry grounded in inspectable product context. A playbook never
/// implies that an agent ran, that a source was researched, or that an outcome
/// was measured.
pub fn catalog() -> Vec<GrowthPlaybook> {
    vec![
        GrowthPlaybook {
            id: "strategist",
            title: "Goal framing",
            role: AgentRole::Strategist,
            focus: "Choose a tractable growth question",
            summary: "Turn a broad growth goal into a small set of testable levers and a clear decision rule.",
            questions: &[
                "What outcome matters now, and which audience is in scope?",
                "Which product surface can change without widening permissions?",
                "What would make this test useful even if the result is inconclusive?",
            ],
            outputs: &[
                "Prioritized experiment brief",
                "Hypothesis tree with parent and child questions",
                "Primary metric, guardrails and decision rule",
            ],
            guardrails: &[
                "Use only the committed product brief and user-supplied evidence.",
                "Do not present a proxy score as conversion, revenue or ranking evidence.",
            ],
        },
        GrowthPlaybook {
            id: "researcher",
            title: "Evidence map",
            role: AgentRole::Researcher,
            focus: "Collect permitted facts before proposing copy",
            summary: "Separate product facts, primary sources and model inference so a growth claim can be checked.",
            questions: &[
                "Which claim is directly supported by the product or a primary source?",
                "What source date, publisher and limitation should be recorded?",
                "Which missing fact would most change the proposed experiment?",
            ],
            outputs: &[
                "Evidence records with retrieval dates",
                "Claim-to-source map",
                "Open questions and confidence rationale",
            ],
            guardrails: &[
                "Respect robots.txt, terms, rate limits and access boundaries.",
                "Never invent customer quotes, market size or competitor facts.",
            ],
        },
        GrowthPlaybook {
            id: "positioning",
            title: "Positioning clarity",
            role: AgentRole::Positioning,
            focus: "Make the product's value easy to understand",
            summary: "Compare outcome-led, pain-led and audience-specific messages using facts the product can support.",
            questions: &[
                "Who is the intended visitor and what job are they trying to finish?",
                "What concrete outcome can the product demonstrate today?",
                "Which words could overpromise capability or certainty?",
            ],
            outputs: &[
                "Three concise value propositions",
                "Headline, supporting proof and call-to-action variants",
                "Claim checklist tied to product evidence",
            ],
            guardrails: &[
                "Do not add testimonials, logos or results without supplied evidence.",
                "Keep the original product safe until a user selects a change.",
            ],
        },
        GrowthPlaybook {
            id: "conversion",
            title: "Conversion path",
            role: AgentRole::Conversion,
            focus: "Remove uncertainty before the first action",
            summary: "Improve landing-page hierarchy and calls to action while preserving a transparent, reversible review path.",
            questions: &[
                "What is the first useful action for a qualified visitor?",
                "What proof can be inspected without a hidden or external dependency?",
                "Which friction is observable in the current page or flow?",
            ],
            outputs: &[
                "Page hierarchy and CTA alternatives",
                "Inspectable proof placement plan",
                "Validation and guardrail checklist",
            ],
            guardrails: &[
                "No dark patterns, forced consent or deceptive urgency.",
                "Do not claim a conversion lift before real telemetry is connected.",
            ],
        },
        GrowthPlaybook {
            id: "seo",
            title: "Ethical SEO discovery",
            role: AgentRole::Seo,
            focus: "Match useful pages to real search intent",
            summary: "Find evidence-backed page opportunities and improve on-page clarity without spam or ranking promises.",
            questions: &[
                "What question can the product answer better than a generic page?",
                "Which supplied facts support the title, headings and description?",
                "What would make the page useful after the visitor arrives from search?",
            ],
            outputs: &[
                "Search-intent and page brief",
                "Title, description, heading and internal-link suggestions",
                "Local structural audit with next steps",
            ],
            guardrails: &[
                "No keyword stuffing, doorway pages, scraped copy or guaranteed rankings.",
                "Label rubric output ESTIMATED and keep real traffic MEASURED only from user data.",
            ],
        },
        GrowthPlaybook {
            id: "onboarding",
            title: "Activation path",
            role: AgentRole::Onboarding,
            focus: "Shorten the path to first value",
            summary: "Make the first successful product action clear and executable for the intended audience.",
            questions: &[
                "What does first value look like for this audience?",
                "Which setup step can be removed, reordered or explained with real product context?",
                "Which guardrail catches a faster path that attracts the wrong users?",
            ],
            outputs: &[
                "First-value journey map",
                "Quick-start and empty-state variants",
                "Activation event and guardrail definitions",
            ],
            guardrails: &[
                "Do not hide important limits, costs or permissions.",
                "A shorter flow is a hypothesis, not proof of activation lift.",
            ],
        },
        GrowthPlaybook {
            id: "pricing",
            title: "Pricing questions",
            role: AgentRole::Pricing,
            focus: "Explore packaging without inventing market facts",
            summary: "Frame reversible pricing and packaging hypotheses from product value and explicit user constraints.",
            questions: &[
                "Which capability or outcome is being packaged, and for whom?",
                "What willingness-to-pay evidence is actually available?",
                "What experiment can run without mutating live billing?",
            ],
            outputs: &[
                "Packaging and value-metric hypotheses",
                "Non-production pricing-page variants",
                "Billing, trust and support guardrails",
            ],
            guardrails: &[
                "Never change live prices, billing or entitlements automatically.",
                "Mark market assumptions as untested and request user evidence.",
            ],
        },
        GrowthPlaybook {
            id: "launch",
            title: "Launch narrative",
            role: AgentRole::Launch,
            focus: "Prepare a truthful channel-specific launch",
            summary: "Turn a real product change into launch assets that fit a channel and make the next action clear.",
            questions: &[
                "What changed, who benefits and where can it be verified?",
                "Which channel's norms and audience are in scope?",
                "What should a reader do next without being pressured?",
            ],
            outputs: &[
                "Channel-specific announcement drafts",
                "Proof links and launch checklist",
                "Reply and support questions to monitor",
            ],
            guardrails: &[
                "No mass unsolicited outreach, fake scarcity or fabricated traction.",
                "Drafts never post, email or publish without explicit user action.",
            ],
        },
        GrowthPlaybook {
            id: "evaluator",
            title: "Inspectable evaluation",
            role: AgentRole::Evaluator,
            focus: "Apply a stable rubric to every candidate",
            summary: "Check implementation quality and evidence with separate, visible inputs instead of a magic score.",
            questions: &[
                "Which deterministic checks and rubric dimensions apply?",
                "Can another reviewer reproduce every score from recorded inputs?",
                "Which limitation prevents this from being called a measured outcome?",
            ],
            outputs: &[
                "Decomposable evaluation rows",
                "Observed checks and estimated rubric notes",
                "Confidence label with rationale",
            ],
            guardrails: &[
                "The evaluator must not silently reward its own proposal.",
                "Never label a candidate Winner without a defined outcome evaluation.",
            ],
        },
        GrowthPlaybook {
            id: "skeptic",
            title: "Skeptic review",
            role: AgentRole::Skeptic,
            focus: "Try to falsify the growth claim",
            summary: "Look for missing evidence, alternative explanations and unsafe implementation assumptions before a decision.",
            questions: &[
                "What observation would disconfirm the proposed mechanism?",
                "Which segment, guardrail or time window is missing?",
                "Could the change improve a proxy while harming the intended outcome?",
            ],
            outputs: &[
                "Counter-hypotheses and failure cases",
                "Evidence gaps and follow-up measurements",
                "Reject, revise or inconclusive recommendation",
            ],
            guardrails: &[
                "Keep uncertainty visible instead of converting it into a score.",
                "Do not access private customer data or bypass product permissions.",
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn catalog_covers_each_initial_role_with_explicit_boundaries() {
        let catalog = catalog();
        assert_eq!(catalog.len(), 10);
        let ids: HashSet<_> = catalog.iter().map(|item| item.id).collect();
        assert_eq!(ids.len(), catalog.len());
        let roles: HashSet<_> = catalog.iter().map(|item| item.role).collect();
        assert_eq!(roles.len(), 10);
        assert!(catalog.iter().all(|item| {
            !item.summary.is_empty()
                && !item.questions.is_empty()
                && !item.outputs.is_empty()
                && !item.guardrails.is_empty()
        }));
        let seo = catalog.iter().find(|item| item.id == "seo").unwrap();
        assert!(seo
            .guardrails
            .iter()
            .any(|guardrail| guardrail.contains("keyword")));
    }

    #[test]
    fn execution_fills_missing_answers_and_keeps_outcomes_untested() {
        let workspace = GrowthWorkspace {
            project_id: "p".into(),
            config: super::super::config::fixture(),
            source_snapshot_commit: "a".repeat(40),
            created_at: 1,
        };
        let run = execute(&workspace, "seo", &["Specific developer question".into()]).unwrap();
        assert_eq!(run.role, AgentRole::Seo);
        assert_eq!(run.responses.len(), 3);
        assert_eq!(run.responses[0].answer, "Specific developer question");
        assert!(run.responses[1].answer.starts_with("Unknown"));
        assert_eq!(run.provenance, Provenance::Untested);
        assert_eq!(run.confidence.label, ConfidenceLabel::Low);
        assert!(run.validate().is_ok());
    }
}
