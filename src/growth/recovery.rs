//! Recover abandoned attempts from verified checkpoints and registered jobs.
//! Recovery never reruns a provider/command, applies a patch, or resets a worktree.
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{anyhow, Result};
use crate::jobs::{localbox, BackendDescriptor};
use crate::store::{now_ms, RunStatus, Store, StoredRun};

use super::archive;
use super::battle;
use super::battle_model::{
    BattleRun, BattleStatus, GrowthBattle, SealedBattleRun, ValidationRecord,
};
use super::evaluation::{BattleEvaluator, ConfiguredCommandEvaluator};
use super::model::Provenance;
use super::redaction::redact;

const LOG_CAP: u64 = 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryOutcome {
    pub battle: GrowthBattle,
    pub status: String,
    pub recovered_attempts: Vec<String>,
    pub waiting_jobs: Vec<String>,
    pub warnings: Vec<String>,
}

fn read_optional(directory: &Path, name: &str, cap: u64) -> Result<Option<Vec<u8>>> {
    let path = directory.join(name);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || (name != "log" && metadata.len() > cap)
    {
        return Err(anyhow!("Recovery refuses non-regular job metadata/logs"));
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut bytes = Vec::new();
    options.open(path)?.take(cap).read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

fn job_directory(store: &Store, job: &StoredRun) -> Result<PathBuf> {
    uuid::Uuid::parse_str(&job.id).map_err(|_| anyhow!("Invalid registered validation ID"))?;
    let expected = store.data_root().join("growth-jobs").join(&job.id);
    let descriptor = BackendDescriptor::parse(&job.backend_json)?;
    if descriptor.kind != "local_job"
        || descriptor.job_id.as_deref().map(Path::new) != Some(expected.as_path())
    {
        return Err(anyhow!(
            "Recovery refuses an unrelated validation controller"
        ));
    }
    if expected.exists() {
        let canonical = crate::paths::canonicalize(&expected)?;
        if canonical
            != crate::paths::canonicalize(store.data_root())?
                .join("growth-jobs")
                .join(&job.id)
        {
            return Err(anyhow!("Recovery controller path cannot traverse symlinks"));
        }
    } else if std::fs::symlink_metadata(&expected).is_ok()
        || expected.parent().is_some_and(|parent| {
            std::fs::symlink_metadata(parent)
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
        })
    {
        return Err(anyhow!("Recovery controller path cannot be a symlink"));
    }
    Ok(expected)
}

struct InspectedJob {
    job: StoredRun,
    state: Option<RunStatus>,
    exit: Option<i64>,
    log: String,
    truncated: bool,
    registered: bool,
    reason: String,
}

fn inspect_job(store: &Store, job: StoredRun) -> Result<InspectedJob> {
    let directory = job_directory(store, &job)?;
    let descriptor = BackendDescriptor::parse(&job.backend_json)?;
    if !descriptor
        .source_digest
        .as_ref()
        .is_some_and(|hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(anyhow!(
            "Registered validation source digest is missing/malformed"
        ));
    }
    let exit_bytes = read_optional(&directory, "exit_code", 64)?;
    let exit = exit_bytes
        .as_ref()
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .and_then(|text| text.trim().parse::<i64>().ok());
    if exit_bytes.is_some() && exit.is_none() {
        return Err(anyhow!("Registered validation exit metadata is malformed"));
    }
    let pid = read_optional(&directory, "pid", 64)?;
    let registered = pid.as_ref().is_some_and(|bytes| {
        std::str::from_utf8(bytes)
            .ok()
            .and_then(|text| text.trim().parse::<u32>().ok())
            .is_some_and(|id| id > 0)
    });
    if pid.is_some() && !registered {
        return Err(anyhow!(
            "Registered validation process metadata is malformed"
        ));
    }
    let (state, reason) = if let Some(exit) = exit {
        (
            Some(if exit == 0 {
                RunStatus::Done
            } else {
                RunStatus::Failed
            }),
            if read_optional(&directory, "timed_out", 64)?.is_some() {
                "timeout"
            } else if exit == 0 {
                "interrupted_controller"
            } else {
                "command_failed_or_process_crashed"
            },
        )
    } else if registered {
        match localbox::inspect_job(&directory).stage.as_str() {
            "RUNNING" => (
                None,
                "Live registered job; its existing watchdog remains responsible for the deadline",
            ),
            // Re-read exit metadata on the next recovery call if completion raced inspection.
            "COMPLETED" => (
                None,
                "Job completed during inspection; inspect the same handle again",
            ),
            "ERROR" => (Some(RunStatus::Failed), "interrupted_controller"),
            _ => (
                None,
                "Job state is not established; inspect the same handle again",
            ),
        }
    } else if !directory.exists() {
        (Some(RunStatus::Failed), "interrupted_before_launch")
    } else {
        // Directory/run.sh alone cannot prove whether a launcher survives.
        // Never signal guessed PIDs or turn an observation timeout into death.
        (
            None,
            "Launcher registration is incomplete; process termination is unverified",
        )
    };
    let raw = read_optional(&directory, "log", LOG_CAP)?.unwrap_or_default();
    let truncated =
        std::fs::metadata(directory.join("log")).is_ok_and(|metadata| metadata.len() > LOG_CAP);
    Ok(InspectedJob {
        job,
        state,
        exit,
        log: redact(&String::from_utf8_lossy(&raw)),
        truncated,
        registered,
        reason: reason.into(),
    })
}

struct PreparedAttempt {
    run: BattleRun,
    files: BTreeMap<String, Vec<u8>>,
    terminal_checkpoint: Option<String>,
}

pub fn recover(store: &Store, id: &str) -> Result<RecoveryOutcome> {
    // Acquisition proves no compliant live Growth controller owns this battle.
    let _lease = battle::lease(store, id)?;
    let original = store
        .get_growth_battle(id)?
        .ok_or_else(|| anyhow!("Battle not found"))?;
    let attempts = store.growth_attempts(id)?;
    let _ = super::evaluation::compare(store, id)?;
    let mut prepared = Vec::new();
    // Verify all checkpoints before changing any run or finalizing any seal.
    for attempt in &attempts {
        if attempt.archive_digest.is_some() {
            continue;
        }
        let files = if let Some(hash) = &attempt.checkpoint_digest {
            let files = archive::verify(store.data_root(), hash)?;
            if files.get("run.json") != Some(&serde_json::to_vec(&attempt.run)?)
                || files.get("contract.json") != Some(&serde_json::to_vec(&original.contract)?)
            {
                return Err(anyhow!(
                    "Recovery checkpoint does not match the attempt/frozen contract"
                ));
            }
            super::preview::verify(&original, &attempt.run, &files)?;
            files
        } else {
            BTreeMap::from([(
                "contract.json".into(),
                serde_json::to_vec(&original.contract)?,
            )])
        };
        let terminal_checkpoint = attempt
            .checkpoint_digest
            .as_ref()
            .filter(|_| matches!(attempt.run.status.as_str(), "done" | "failed" | "cancelled"))
            .cloned();
        if terminal_checkpoint.is_some() {
            if attempt.run.active_validation.is_some() || attempt.run.ended_at <= 0 {
                return Err(anyhow!(
                    "Terminal checkpoint still has unfinished execution"
                ));
            }
            if attempt.run.status == "done"
                && !ConfiguredCommandEvaluator
                    .evaluate(
                        &original,
                        Some(&SealedBattleRun {
                            run: attempt.run.clone(),
                            archive_digest: terminal_checkpoint.clone().unwrap(),
                        }),
                        "Recovered candidate",
                        &attempt.run.variant_id,
                    )
                    .eligible
            {
                return Err(anyhow!(
                    "Successful checkpoint does not satisfy the frozen evaluation contract"
                ));
            }
        }
        prepared.push(PreparedAttempt {
            run: attempt.run.clone(),
            files,
            terminal_checkpoint,
        });
    }
    if original.status == BattleStatus::Ready {
        if !attempts.is_empty() {
            return Err(anyhow!("An unstarted battle cannot have attempts"));
        }
        return Ok(RecoveryOutcome {
            battle: original,
            status: "not_started".into(),
            recovered_attempts: vec![],
            waiting_jobs: vec![],
            warnings: vec![],
        });
    }
    let mut inspected = BTreeMap::new();
    let mut waiting = Vec::new();
    let mut warnings = Vec::new();
    let mut orphans = Vec::new();
    for attempt in &prepared {
        if let Some(active) = &attempt.run.active_validation {
            if let Some(record) = &active.confinement {
                let policy = attempt
                    .files
                    .get(&format!("validation-{}.policy.json", active.command_index))
                    .ok_or_else(|| anyhow!("Captured validation confinement policy is missing"))?;
                super::confinement::verify_record(record, policy)?;
            }
            let job = store
                .get_run(&active.run_id)?
                .ok_or_else(|| anyhow!("Active validation registration is missing"))?;
            if attempt.run.candidate_commit.is_none()
                || job.experiment_id != attempt.run.variant_id
                || job.project_id != original.project_id
                || job.commit_sha != attempt.run.candidate_commit
                || active.command_index != attempt.run.validations.len()
                || original
                    .contract
                    .config
                    .validation
                    .commands
                    .get(active.command_index)
                    != Some(&job.command)
            {
                return Err(anyhow!(
                    "Active validation does not match the checkpoint contract"
                ));
            }
            let job = inspect_job(store, job)?;
            if job.state.is_none() {
                waiting.push(job.job.id.clone());
                warnings.push(job.reason.clone());
            }
            inspected.insert(job.job.id.clone(), job);
        }
    }
    // Older attempts had no active-job link. Also cover a crash after writing
    // the generic run but before checkpointing that link, without guessing a PID.
    for job in store.list_runs_by_project(&original.project_id)? {
        let Some(attempt) = prepared.iter().find(|attempt| {
            attempt.run.variant_id == job.experiment_id && job.created_at >= attempt.run.started_at
        }) else {
            continue;
        };
        if inspected.contains_key(&job.id)
            || attempt
                .run
                .validations
                .iter()
                .any(|check| check.run_id == job.id)
        {
            continue;
        }
        let descriptor = BackendDescriptor::parse(&job.backend_json)?;
        if descriptor.kind != "local_job"
            || descriptor.job_id.as_deref().map(Path::new)
                != Some(
                    store
                        .data_root()
                        .join("growth-jobs")
                        .join(&job.id)
                        .as_path(),
                )
        {
            continue;
        }
        if attempt.terminal_checkpoint.is_some() && attempt.run.status == "done" {
            return Err(anyhow!("Successful terminal checkpoint has an unrecorded Growth validation; inspect its registration"));
        }
        let job = inspect_job(store, job)?;
        if job.state.is_none() {
            waiting.push(job.job.id.clone());
            warnings.push(job.reason.clone());
        }
        orphans.push(job);
    }
    if !waiting.is_empty() {
        return Ok(RecoveryOutcome {
            battle: original,
            status: "waiting".into(),
            recovered_attempts: vec![],
            waiting_jobs: waiting,
            warnings,
        });
    }
    let inspected_at = now_ms();
    // All handles are now terminal or proved not submitted (no job directory).
    for orphan in &orphans {
        store.update_status(
            &orphan.job.id,
            orphan
                .state
                .ok_or_else(|| anyhow!("Validation is still active"))?,
            Some(inspected_at),
            orphan.exit,
        )?;
    }
    let mut recovered = Vec::new();
    for mut attempt in prepared {
        if let Some(hash) = attempt.terminal_checkpoint {
            store.seal_growth_attempt(&attempt.run, &hash)?;
        } else {
            for orphan in orphans
                .iter()
                .filter(|orphan| orphan.job.experiment_id == attempt.run.variant_id)
            {
                attempt.files.insert(format!("orphan-validation-{}.json",orphan.job.id),serde_json::to_vec(&serde_json::json!({"runId":orphan.job.id,"status":orphan.state.map(|state|state.as_str()),"exitCode":orphan.exit,"reason":orphan.reason,"sourceCommit":orphan.job.commit_sha,"observedAt":inspected_at,"includedInRubric":false}))?);
                if !orphan.log.is_empty() {
                    attempt.files.insert(
                        format!("orphan-validation-{}.log", orphan.job.id),
                        orphan.log.as_bytes().to_vec(),
                    );
                }
            }
            if let Some(active) = attempt.run.active_validation.take() {
                let job = inspected
                    .remove(&active.run_id)
                    .ok_or_else(|| anyhow!("Inspected validation disappeared"))?;
                let status = job
                    .state
                    .ok_or_else(|| anyhow!("Validation is still active"))?;
                store.update_status(&job.job.id, status, Some(inspected_at), job.exit)?;
                attempt.files.insert(
                    format!("validation-{}.log", active.command_index),
                    job.log.into_bytes(),
                );
                let descriptor = BackendDescriptor::parse(&job.job.backend_json)?;
                attempt.run.validations.push(ValidationRecord {
                    run_id:job.job.id,command:job.job.command,source_commit:job.job.commit_sha.unwrap(),source_digest:descriptor.source_digest.ok_or_else(||anyhow!("Validation source digest missing"))?,status:status.as_str().into(),exit_code:job.exit,termination_reason:Some(job.reason),log_truncated:job.truncated,started_at:job.job.created_at,ended_at:inspected_at,
                    provenance:if job.registered || job.exit.is_some() {Provenance::Observed} else {Provenance::Untested},
                    limitation:"Recovered job observation on its recorded source. Completion time is recovery inspection; the command may have ended earlier. An interrupted attempt is never promoted to a successful candidate.".into(),
                    confinement:active.confinement,
                });
            }
            attempt.run.status = if original.cancel_requested {
                "cancelled"
            } else {
                "failed"
            }
            .into();
            attempt.run.ended_at = inspected_at;
            attempt.run.error=Some("Growth controller was interrupted before a terminal checkpoint. Retained evidence was recovered; no provider or validation command was rerun. Uncaptured evidence remains unavailable.".into());
            attempt.run.outcome_provenance = Provenance::Untested;
            attempt.files.insert("recovery.json".into(),serde_json::to_vec(&serde_json::json!({"version":1,"inspectedAt":inspected_at,"reason":"interrupted_controller","reranCommands":false,"reranProvider":false,"restoredWorktree":false}))?);
            let hash = battle::checkpoint(store, &attempt.run, &mut attempt.files)?;
            store.seal_growth_attempt(&attempt.run, &hash)?;
        }
        if let Some(mut experiment) = store.get_local_experiment(&attempt.run.variant_id)? {
            experiment.agent_status = attempt.run.status.clone();
            store.update_local_experiment(&experiment)?;
        }
        recovered.push(attempt.run.id);
    }
    let sealed = store.sealed_growth_runs(id)?;
    let all_done = sealed.len() == original.contract.hypotheses.len()
        && sealed.iter().all(|seal| seal.run.status == "done");
    let status = if original.cancel_requested {
        BattleStatus::Cancelled
    } else if all_done {
        BattleStatus::Completed
    } else {
        BattleStatus::Failed
    };
    store.finish_growth_battle(id, status)?;
    Ok(RecoveryOutcome {
        battle: store
            .get_growth_battle(id)?
            .ok_or_else(|| anyhow!("Battle disappeared"))?,
        status: if !recovered.is_empty() || original.status == BattleStatus::Running {
            "recovered"
        } else {
            "unchanged"
        }
        .into(),
        recovered_attempts: recovered,
        waiting_jobs: vec![],
        warnings,
    })
}
