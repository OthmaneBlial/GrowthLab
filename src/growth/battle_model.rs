//! Frozen battle contracts and append-only, provenance-bearing run outcomes.
use serde::{Deserialize, Serialize};

use super::config::GrowthConfig;
use super::model::{Confidence, GrowthHypothesis, Provenance};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BattleContract {
    pub version: u32,
    pub goal: String,
    pub config: GrowthConfig,
    pub source_snapshot_commit: String,
    pub source_snapshot_digest: String,
    pub hypotheses: Vec<GrowthHypothesis>,
    pub evaluation: String,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BattleStatus {
    Ready,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrowthBattle {
    pub id: String,
    pub project_id: String,
    pub contract: BattleContract,
    pub contract_digest: String,
    pub status: BattleStatus,
    pub created_at: i64,
    pub ended_at: Option<i64>,
    pub cancel_requested: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrowthVariant {
    /// Same ID as the inherited LocalExperiment node.
    pub id: String,
    pub battle_id: String,
    pub hypothesis_id: String,
    pub branch_name: String,
    pub worktree: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileEdit {
    pub path: String,
    /// None means delete this allowed file. Directories/symlinks are forbidden.
    pub contents: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Implementation {
    pub summary: String,
    pub files: Vec<FileEdit>,
    pub risks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplayPlan {
    pub version: u32,
    /// Ordered to match the frozen hypothesis portfolio. This is simulation
    /// input, never evidence of a native agent or a measured growth outcome.
    pub implementations: Vec<Implementation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentMetadata {
    pub harness: String,
    pub requested_model: Option<String>,
    pub model_selection_honoured: bool,
    pub mode: String,
    pub cost: Option<f64>,
    pub tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationRecord {
    /// Inherited StoredRun backed by the localbox controller.
    pub run_id: String,
    pub command: String,
    pub source_commit: String,
    pub source_digest: String,
    pub status: String,
    pub exit_code: Option<i64>,
    pub termination_reason: Option<String>,
    pub log_truncated: bool,
    pub started_at: i64,
    pub ended_at: i64,
    pub provenance: Provenance,
    pub limitation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confinement: Option<super::confinement::ConfinementRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActiveValidation {
    pub run_id: String,
    pub command_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confinement: Option<super::confinement::ConfinementRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BattleRun {
    pub id: String,
    pub variant_id: String,
    pub battle_id: String,
    pub contract_digest: String,
    pub source_snapshot_commit: String,
    pub candidate_commit: Option<String>,
    pub agent: AgentMetadata,
    pub implementation: Option<Implementation>,
    pub validations: Vec<ValidationRecord>,
    pub status: String,
    pub error: Option<String>,
    pub started_at: i64,
    pub ended_at: i64,
    /// Implementation proposal provenance, separate from observed validations.
    pub provenance: Provenance,
    pub confidence: Confidence,
    pub outcome_provenance: Provenance,
    // Keep old sealed run serialization byte-for-byte compatible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_validation: Option<ActiveValidation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SealedBattleRun {
    pub run: BattleRun,
    pub archive_digest: String,
}

/// A live attempt/checkpoint is not a terminal seal or evaluation result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrowthAttempt {
    pub run: BattleRun,
    pub checkpoint_digest: Option<String>,
    pub archive_digest: Option<String>,
}
