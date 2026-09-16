//! Explainable deterministic comparison. Passing a command is an observation,
//! not proof of conversion lift or an accessibility/performance score.
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
        "configured-command-pass-v1"
    }
    fn calculation(&self) -> &str {
        "Passed configured commands / required configured commands. Eligibility additionally requires a successful sealed run, the exact frozen command list and one immutable candidate commit. This ratio normalizes only command checks; it is not a combined growth score."
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
        EvaluationRow { variant_id:variant_id.into(),title:title.into(),status:run.map(|run|run.status.clone()).unwrap_or("untested".into()),passed_commands:passed,required_commands:required,pass_fraction:(!checks.is_empty() && required>0).then_some(passed as f64 / required.max(1) as f64),eligible:required>0 && matches && passed==required && run.is_some_and(|run|run.status=="done"),checks,implementation_provenance:run.map(|run|run.provenance).unwrap_or(Provenance::Untested),check_provenance:if run.is_some_and(|run|!run.validations.is_empty()) {Provenance::Observed} else {Provenance::Untested},outcome_provenance:Provenance::Untested,confidence:Confidence { label:ConfidenceLabel::Low,rationale:"Command checks cannot establish outcome lift; candidate choice needs user review and real product evidence.".into() },archive_digest:sealed.map(|sealed|sealed.archive_digest.clone()) }
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
