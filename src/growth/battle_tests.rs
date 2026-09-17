use std::path::{Path, PathBuf};

use super::archive;
use super::battle::{self, AgentInput, BattleAgent, HarnessAgent, ReplayAgent};
use super::battle_model::*;
use super::model::Provenance;
use crate::store::Store;

const GATED_CHECK: &str = "node -e \"const fs=require('node:fs');fs.writeFileSync('ready','started');const timer=setInterval(()=>{if(fs.existsSync('release')){clearInterval(timer);process.exit(fs.readFileSync('website/index.html','utf8').includes('<h1>')?0:2)}},20)\"";

async fn abandon_controller(
    fixture: &Fixture,
    battle_id: &str,
    agent: &dyn BattleAgent,
    active: bool,
) -> Vec<GrowthAttempt> {
    let execution = battle::execute(&fixture.store, battle_id, agent);
    tokio::pin!(execution);
    let started = async {
        for _ in 0..1500 {
            let attempts = fixture.store.growth_attempts(battle_id).unwrap();
            if attempts.len() == 3
                && attempts.iter().all(|attempt| {
                    attempt.checkpoint_digest.is_some()
                        && (!active
                            || attempt.run.active_validation.as_ref().is_some_and(|job| {
                                fixture
                                    .store
                                    .data_root()
                                    .join("growth-jobs")
                                    .join(&job.run_id)
                                    .join("repo/ready")
                                    .exists()
                            }))
                })
            {
                let error = super::recovery::recover(&fixture.store, battle_id).unwrap_err();
                assert!(error.to_string().contains("live controller"));
                return attempts;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        panic!("fixture controller never reached the requested checkpoint");
    };
    tokio::select! {
        result=&mut execution=>panic!("fixture controller finished before interruption: {result:?}"),
        attempts=started=>attempts,
    }
    // Dropping this actual execution future releases its OS lease. Registered
    // Bash controllers and their payloads remain independently alive.
}

async fn release_registered_jobs(fixture: &Fixture, attempts: &[GrowthAttempt]) {
    let jobs: Vec<_> = attempts
        .iter()
        .map(|attempt| {
            let id = &attempt.run.active_validation.as_ref().unwrap().run_id;
            let job = fixture.store.get_run(id).unwrap().unwrap();
            let descriptor = crate::jobs::BackendDescriptor::parse(&job.backend_json).unwrap();
            let directory = PathBuf::from(descriptor.job_id.unwrap());
            std::fs::write(directory.join("repo/release"), "fixture release").unwrap();
            directory
        })
        .collect();
    for _ in 0..300 {
        if jobs.iter().all(|directory| {
            matches!(
                crate::jobs::localbox::inspect_job(directory).stage.as_str(),
                "COMPLETED" | "ERROR"
            )
        }) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("released fixture jobs did not become terminal");
}

#[tokio::test]
async fn interrupted_jobs_wait_then_recover_sealed_context_without_rerunning_or_reading_worktrees()
{
    let fixture = Fixture::with_timeout(Some(GATED_CHECK), 60);
    let battle =
        battle::prepare(&fixture.store, &fixture.project_id, "Interrupted fixture").unwrap();
    let attempts = abandon_controller(&fixture, &battle.id, &Fixture::plan(), true).await;
    let waiting = super::recovery::recover(&fixture.store, &battle.id).unwrap();
    assert_eq!(waiting.status, "waiting");
    assert_eq!(waiting.waiting_jobs.len(), 3);
    assert!(waiting.recovered_attempts.is_empty());
    assert_eq!(fixture.store.growth_attempts(&battle.id).unwrap(), attempts);
    let job_id = &attempts[0].run.active_validation.as_ref().unwrap().run_id;
    let registered_job = fixture.store.get_run(job_id).unwrap().unwrap();
    let descriptor = crate::jobs::BackendDescriptor::parse(&registered_job.backend_json).unwrap();
    let mut unrelated = descriptor.clone();
    unrelated.job_id = Some(
        fixture
            .root
            .join("unrelated-controller")
            .to_string_lossy()
            .into_owned(),
    );
    fixture
        .store
        .set_backend_json(job_id, &unrelated.to_json())
        .unwrap();
    assert!(super::recovery::recover(&fixture.store, &battle.id).is_err());
    assert_eq!(fixture.store.growth_attempts(&battle.id).unwrap(), attempts);
    let mut missing_source = descriptor.clone();
    missing_source.source_digest = None;
    fixture
        .store
        .set_backend_json(job_id, &missing_source.to_json())
        .unwrap();
    assert!(super::recovery::recover(&fixture.store, &battle.id).is_err());
    assert_eq!(fixture.store.growth_attempts(&battle.id).unwrap(), attempts);
    fixture
        .store
        .set_backend_json(job_id, &registered_job.backend_json)
        .unwrap();
    let directory = PathBuf::from(descriptor.job_id.unwrap());
    std::fs::rename(
        directory.join("pid"),
        directory.join("fixture-retained-pid"),
    )
    .unwrap();
    let unregistered = super::recovery::recover(&fixture.store, &battle.id).unwrap();
    assert_eq!(unregistered.status, "waiting");
    assert!(unregistered
        .warnings
        .iter()
        .any(|warning| warning.contains("termination is unverified")));
    assert_eq!(fixture.store.growth_attempts(&battle.id).unwrap(), attempts);
    std::fs::rename(
        directory.join("fixture-retained-pid"),
        directory.join("pid"),
    )
    .unwrap();
    let variants = fixture.store.growth_variants(&battle.id).unwrap();
    std::fs::write(
        Path::new(&variants[0].worktree).join("website/index.html"),
        "Unsealed later work",
    )
    .unwrap();
    release_registered_jobs(&fixture, &attempts).await;
    let moved = fixture.root.join("temporarily-unavailable-product");
    std::fs::rename(&fixture.product, &moved).unwrap();
    let recovered = super::recovery::recover(&fixture.store, &battle.id).unwrap();
    assert_eq!(recovered.status, "recovered");
    assert_eq!(recovered.recovered_attempts.len(), 3);
    assert_eq!(recovered.battle.status, BattleStatus::Failed);
    let sealed = fixture.store.sealed_growth_runs(&battle.id).unwrap();
    assert_eq!(sealed.len(), 3);
    for seal in &sealed {
        assert_eq!(seal.run.status, "failed");
        assert!(seal.run.active_validation.is_none());
        assert_eq!(seal.run.provenance, Provenance::Simulated);
        assert_eq!(seal.run.outcome_provenance, Provenance::Untested);
        assert_eq!(seal.run.validations.len(), 1);
        assert_eq!(seal.run.validations[0].provenance, Provenance::Observed);
        let files = archive::verify(fixture.store.data_root(), &seal.archive_digest).unwrap();
        assert!(
            files.contains_key("proposal-input.json")
                && files.contains_key("implementation.diff")
                && files.contains_key("validation-0.log")
                && files.contains_key("recovery.json")
        );
        if seal.run.variant_id == variants[0].id {
            assert_eq!(files["files/website/index.html"], b"<h1>Outcome-first</h1>");
        }
    }
    let comparison = super::evaluation::compare(&fixture.store, &battle.id).unwrap();
    assert!(
        comparison.recommended_candidates.is_empty(),
        "interrupted attempts must never be promoted to success"
    );
    assert_eq!(comparison.rows[1].checks[0].exit_code, Some(2));
    assert_eq!(
        comparison
            .rows
            .iter()
            .map(|row| row.hypothesis_id.as_deref())
            .collect::<Vec<_>>(),
        battle
            .contract
            .hypotheses
            .iter()
            .map(|hypothesis| Some(hypothesis.id.as_str()))
            .collect::<Vec<_>>()
    );
    super::report::export(
        &fixture.store,
        &battle.id,
        &super::report::ReportOptions::default(),
        &fixture.root.join("recovered.html"),
        false,
    )
    .unwrap();
    assert_eq!(
        super::recovery::recover(&fixture.store, &battle.id)
            .unwrap()
            .status,
        "unchanged"
    );
    assert_eq!(
        fixture.store.sealed_growth_runs(&battle.id).unwrap(),
        sealed
    );
    std::fs::rename(moved, &fixture.product).unwrap();
    fixture.unchanged();
    assert_eq!(
        std::fs::read_to_string(Path::new(&variants[0].worktree).join("website/index.html"))
            .unwrap(),
        "Unsealed later work"
    );
    assert_eq!(
        fixture
            .store
            .list_runs_by_project(&fixture.project_id)
            .unwrap()
            .len(),
        3,
        "recovery cannot launch new jobs"
    );
}

#[test]
fn prepared_battle_preserves_the_workspace_experiment_map_lineage() {
    let fixture = Fixture::new(None);
    let workspace = fixture
        .store
        .get_growth_workspace(&fixture.project_id)
        .unwrap()
        .unwrap();
    let hypotheses = super::model::starter_hypotheses(&workspace);
    fixture
        .store
        .insert_growth_hypotheses(&fixture.project_id, &hypotheses)
        .unwrap();

    let battle = battle::prepare(&fixture.store, &fixture.project_id, "Map lineage goal").unwrap();
    assert_eq!(battle.contract.goal, "Map lineage goal");
    assert_eq!(battle.contract.hypotheses.len(), 3);
    for (persisted, contract) in hypotheses.iter().zip(&battle.contract.hypotheses) {
        assert_eq!(persisted.id, contract.id);
        assert_eq!(contract.goal, "Map lineage goal");
    }
    let variants = fixture.store.growth_variants(&battle.id).unwrap();
    assert_eq!(
        variants
            .iter()
            .map(|variant| variant.hypothesis_id.as_str())
            .collect::<Vec<_>>(),
        hypotheses
            .iter()
            .map(|hypothesis| hypothesis.id.as_str())
            .collect::<Vec<_>>()
    );
    fixture.unchanged();
}

#[tokio::test]
async fn terminal_checkpoint_finalization_failure_recovers_one_seal_and_preserves_siblings() {
    let fixture = Fixture::new(None);
    let battle = battle::prepare(&fixture.store, &fixture.project_id, "Finalize fixture").unwrap();
    let variant = fixture.store.growth_variants(&battle.id).unwrap()[0]
        .id
        .clone();
    let transaction = fixture.store.begin().unwrap();
    transaction.execute_batch(&format!("CREATE TRIGGER fixture_seal_failure BEFORE UPDATE ON growth_battle_runs WHEN NEW.archive_digest IS NOT NULL AND NEW.variant_id='{variant}' BEGIN SELECT RAISE(ABORT,'synthetic finalization failure'); END;")).unwrap();
    transaction.commit().unwrap();
    assert!(
        battle::execute(&fixture.store, &battle.id, &Fixture::plan())
            .await
            .is_err()
    );
    let attempts = fixture.store.growth_attempts(&battle.id).unwrap();
    let interrupted = attempts
        .iter()
        .find(|attempt| attempt.run.variant_id == variant)
        .unwrap();
    assert!(interrupted.archive_digest.is_none());
    assert_eq!(interrupted.run.status, "done");
    let original_hash = interrupted.checkpoint_digest.clone().unwrap();
    let siblings = fixture.store.sealed_growth_runs(&battle.id).unwrap();
    assert_eq!(siblings.len(), 2);
    let transaction = fixture.store.begin().unwrap();
    transaction
        .execute_batch("DROP TRIGGER fixture_seal_failure;")
        .unwrap();
    transaction.commit().unwrap();
    let result = super::recovery::recover(&fixture.store, &battle.id).unwrap();
    assert_eq!(result.recovered_attempts, vec![interrupted.run.id.clone()]);
    let sealed = fixture.store.sealed_growth_runs(&battle.id).unwrap();
    assert!(siblings.iter().all(|sibling| sealed.contains(sibling)));
    assert!(sealed
        .iter()
        .any(|seal| seal.archive_digest == original_hash && seal.run == interrupted.run));
    assert_eq!(
        super::evaluation::compare(&fixture.store, &battle.id)
            .unwrap()
            .recommended_candidates
            .len(),
        2
    );
    assert_eq!(
        super::recovery::recover(&fixture.store, &battle.id)
            .unwrap()
            .status,
        "unchanged"
    );
    fixture.unchanged();
}

#[tokio::test]
async fn tampered_checkpoint_refuses_recovery_before_any_outcome_changes() {
    let fixture = Fixture::new(None);
    let battle = battle::prepare(
        &fixture.store,
        &fixture.project_id,
        "Checkpoint tamper fixture",
    )
    .unwrap();
    let attempts = abandon_controller(&fixture, &battle.id, &WaitingAgent, false).await;
    let target = fixture
        .store
        .data_root()
        .join("growth-archives")
        .join(attempts[0].checkpoint_digest.as_ref().unwrap())
        .join("proposal-input.json");
    let original = std::fs::read(&target).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    std::fs::write(&target, "tampered").unwrap();
    assert!(super::recovery::recover(&fixture.store, &battle.id).is_err());
    assert_eq!(fixture.store.growth_attempts(&battle.id).unwrap(), attempts);
    assert!(fixture
        .store
        .sealed_growth_runs(&battle.id)
        .unwrap()
        .is_empty());
    assert_eq!(
        fixture
            .store
            .get_growth_battle(&battle.id)
            .unwrap()
            .unwrap()
            .status,
        BattleStatus::Running
    );
    std::fs::write(target, original).unwrap();
    fixture
        .store
        .request_growth_battle_cancel(&battle.id)
        .unwrap();
    let result = super::recovery::recover(&fixture.store, &battle.id).unwrap();
    assert_eq!(result.battle.status, BattleStatus::Cancelled);
    assert_eq!(result.recovered_attempts.len(), 3);
    assert!(fixture
        .store
        .sealed_growth_runs(&battle.id)
        .unwrap()
        .iter()
        .all(|seal| seal.run.status == "cancelled"));
    fixture.unchanged();
}

#[tokio::test]
async fn failed_active_job_checkpoint_prevents_command_launch_and_preserves_terminal_failures() {
    let fixture = Fixture::new(None);
    let battle = battle::prepare(
        &fixture.store,
        &fixture.project_id,
        "Checkpoint persistence fixture",
    )
    .unwrap();
    let transaction = fixture.store.begin().unwrap();
    transaction.execute_batch("CREATE TRIGGER fixture_checkpoint_failure BEFORE UPDATE ON growth_battle_runs WHEN json_type(NEW.payload_json,'$.activeValidation') IS NOT NULL BEGIN SELECT RAISE(ABORT,'synthetic checkpoint failure'); END;").unwrap();
    transaction.commit().unwrap();
    let result = battle::execute(&fixture.store, &battle.id, &Fixture::plan())
        .await
        .unwrap();
    assert_eq!(result.status, BattleStatus::Failed);
    assert!(
        !fixture.store.data_root().join("growth-jobs").exists(),
        "job cannot launch before its durable link checkpoint"
    );
    let jobs = fixture
        .store
        .list_runs_by_project(&fixture.project_id)
        .unwrap();
    assert_eq!(jobs.len(), 3);
    assert!(jobs.iter().all(|job| job.status == "failed"));
    let seals = fixture.store.sealed_growth_runs(&battle.id).unwrap();
    assert_eq!(seals.len(), 3);
    assert!(seals.iter().all(|seal| seal.run.status == "failed"
        && seal.run.validations.is_empty()
        && seal.run.active_validation.is_none()));
    assert_eq!(
        super::recovery::recover(&fixture.store, &battle.id)
            .unwrap()
            .status,
        "unchanged"
    );
    assert_eq!(
        serde_json::to_value(
            fixture
                .store
                .list_runs_by_project(&fixture.project_id)
                .unwrap()
        )
        .unwrap(),
        serde_json::to_value(jobs).unwrap()
    );
    fixture.unchanged();
}

#[tokio::test]
async fn selected_delivery_uses_sealed_objects_and_preserves_head_index_and_remotes() {
    let fixture = Fixture::new(None);
    let battle = battle::prepare(&fixture.store, &fixture.project_id, "Delivery fixture").unwrap();
    battle::execute(&fixture.store, &battle.id, &Fixture::plan())
        .await
        .unwrap();
    let variants = fixture.store.growth_variants(&battle.id).unwrap();
    assert!(super::selection::select(&fixture.store, &variants[1].id).is_err());
    assert!(super::selection::apply(&fixture.store, &variants[1].id).is_err());
    let selection = super::selection::select(&fixture.store, &variants[0].id).unwrap();
    assert_eq!(selection.status, super::selection::SelectionStatus::Done);
    fixture.unchanged();
    // Stale mutable agent content must never enter an export or apply.
    std::fs::write(
        Path::new(&variants[0].worktree).join("website/index.html"),
        "<h1>Unsealed stale worktree text</h1>",
    )
    .unwrap();
    let output = fixture.root.join("selected.patch");
    let export = super::selection::export(&fixture.store, &variants[0].id, &output).unwrap();
    let patch = std::fs::read_to_string(&output).unwrap();
    assert!(patch.contains("+<h1>Outcome-first</h1>"));
    assert!(!patch.contains("Unsealed stale"));
    assert_eq!(
        export.artifact_digest,
        Some(archive::digest(patch.as_bytes()))
    );
    assert!(super::selection::export(&fixture.store, &variants[0].id, &output).is_err());
    assert_eq!(std::fs::read_to_string(&output).unwrap(), patch);
    fixture.unchanged();
    let preview = super::selection::preview_apply(&fixture.store, &variants[0].id).unwrap();
    assert_eq!(preview.changed_files, vec!["website/index.html"]);
    fixture.unchanged();
    let applied = super::selection::apply(&fixture.store, &variants[0].id).unwrap();
    assert_eq!(applied.decision, super::model::Decision::Ship);
    assert_eq!(
        std::fs::read_to_string(fixture.product.join("website/index.html")).unwrap(),
        "<h1>Outcome-first</h1>"
    );
    assert_eq!(
        git(&fixture.product, &["rev-parse", "HEAD"]),
        fixture.commit
    );
    assert_eq!(
        git(&fixture.product, &["diff", "--cached", "--name-only"]),
        ""
    );
    assert_eq!(git(&fixture.product, &["remote"]), "");
    assert!(super::selection::apply(&fixture.store, &variants[0].id).is_err());
    assert!(
        fixture.store.finish_growth_selection(&applied).is_err(),
        "a terminal receipt cannot be overwritten"
    );
    let transaction = fixture.store.begin().unwrap();
    assert!(transaction
        .execute(
            "UPDATE growth_selections SET payload_json='{}' WHERE id=?1",
            [applied.id]
        )
        .is_err());
    transaction.rollback().unwrap();
}

#[tokio::test]
async fn selected_apply_refuses_dirty_staged_untracked_changed_policy_and_moving_head() {
    let fixture = Fixture::new(None);
    let battle = battle::prepare(
        &fixture.store,
        &fixture.project_id,
        "Clean baseline fixture",
    )
    .unwrap();
    battle::execute(&fixture.store, &battle.id, &Fixture::plan())
        .await
        .unwrap();
    let id = fixture.store.growth_variants(&battle.id).unwrap()[0]
        .id
        .clone();
    let path = fixture.product.join("website/index.html");
    git(
        &fixture.product,
        &["update-index", "--assume-unchanged", "website/index.html"],
    );
    assert!(super::selection::apply(&fixture.store, &id).is_err());
    git(
        &fixture.product,
        &[
            "update-index",
            "--no-assume-unchanged",
            "website/index.html",
        ],
    );
    std::fs::write(&path, "<h1>Builder's local work</h1>").unwrap();
    assert!(super::selection::apply(&fixture.store, &id).is_err());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "<h1>Builder's local work</h1>"
    );
    git(&fixture.product, &["add", "website/index.html"]);
    assert!(super::selection::apply(&fixture.store, &id).is_err());
    assert_eq!(
        git(&fixture.product, &["diff", "--cached", "--name-only"]),
        "website/index.html"
    );
    git(&fixture.product, &["reset", "--hard", &fixture.commit]);
    std::fs::write(fixture.product.join("builder-notes.txt"), "Local draft").unwrap();
    assert!(super::selection::apply(&fixture.store, &id).is_err());
    std::fs::remove_file(fixture.product.join("builder-notes.txt")).unwrap();
    let original_config = std::fs::read(fixture.product.join("growthlab.yaml")).unwrap();
    let mut config = super::config::GrowthConfig::load(&fixture.product).unwrap();
    config.permissions.mode = super::config::PermissionMode::AnalyzeOnly;
    std::fs::write(
        fixture.product.join("growthlab.yaml"),
        serde_yaml_ng::to_string(&config).unwrap(),
    )
    .unwrap();
    assert!(super::selection::apply(&fixture.store, &id).is_err());
    std::fs::write(fixture.product.join("growthlab.yaml"), original_config).unwrap();
    std::fs::write(&path, "<h1>Builder's newer commit</h1>").unwrap();
    git(&fixture.product, &["add", "website/index.html"]);
    git(
        &fixture.product,
        &["commit", "-m", "Builder newer baseline"],
    );
    let newer = git(&fixture.product, &["rev-parse", "HEAD"]);
    assert!(super::selection::apply(&fixture.store, &id).is_err());
    // A portable export is still useful when the product has advanced.
    super::selection::export(&fixture.store, &id, &fixture.root.join("advanced.patch")).unwrap();
    assert_eq!(git(&fixture.product, &["rev-parse", "HEAD"]), newer);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "<h1>Builder's newer commit</h1>"
    );
}

#[tokio::test]
async fn selected_apply_delivers_additions_empty_text_and_deletions() {
    let fixture = Fixture::new(None);
    let battle =
        battle::prepare(&fixture.store, &fixture.project_id, "Add delete fixture").unwrap();
    let mut agent = Fixture::plan();
    for implementation in &mut agent.0.implementations {
        implementation.files.extend([
            FileEdit {
                path: "website/empty file.txt".into(),
                contents: Some(String::new()),
            },
            FileEdit {
                path: "website/retired.txt".into(),
                contents: None,
            },
            FileEdit {
                path: "website/lines.txt".into(),
                contents: Some("++Added prefix\n--Also added\n".into()),
            },
        ]);
    }
    battle::execute(&fixture.store, &battle.id, &agent)
        .await
        .unwrap();
    let id = fixture.store.growth_variants(&battle.id).unwrap()[0]
        .id
        .clone();
    let preview = super::selection::preview_apply(&fixture.store, &id).unwrap();
    assert_eq!(preview.changed_files.len(), 4);
    let report = super::report::build(
        &fixture.store,
        &battle.id,
        &super::report::ReportOptions::default(),
    )
    .unwrap();
    assert_eq!(report.variants[0].files_changed, 4);
    assert_eq!(
        report.variants[0].hypothesis_id.as_deref(),
        Some(battle.contract.hypotheses[0].id.as_str())
    );
    assert_eq!(
        report.variants[0].lines_added, 3,
        "diff body lines starting with ++ must count as additions"
    );
    assert_eq!(report.variants[0].lines_removed, 2);
    super::selection::apply(&fixture.store, &id).unwrap();
    assert_eq!(
        std::fs::read(fixture.product.join("website/empty file.txt")).unwrap(),
        Vec::<u8>::new()
    );
    assert!(!fixture.product.join("website/retired.txt").exists());
    assert_eq!(
        git(&fixture.product, &["diff", "--cached", "--name-only"]),
        ""
    );
    assert_eq!(
        git(&fixture.product, &["rev-parse", "HEAD"]),
        fixture.commit
    );
}

#[tokio::test]
async fn failed_delivery_intent_refuses_product_and_export_writes() {
    let fixture = Fixture::new(None);
    let battle = battle::prepare(
        &fixture.store,
        &fixture.project_id,
        "Intent failure fixture",
    )
    .unwrap();
    battle::execute(&fixture.store, &battle.id, &Fixture::plan())
        .await
        .unwrap();
    let id = fixture.store.growth_variants(&battle.id).unwrap()[0]
        .id
        .clone();
    let transaction = fixture.store.begin().unwrap();
    transaction.execute_batch("CREATE TRIGGER fixture_selection_failure BEFORE INSERT ON growth_selections BEGIN SELECT RAISE(ABORT,'synthetic intent failure'); END;").unwrap();
    transaction.commit().unwrap();
    assert!(super::selection::apply(&fixture.store, &id).is_err());
    let output = fixture.root.join("not-written.patch");
    assert!(super::selection::export(&fixture.store, &id, &output).is_err());
    assert!(!output.exists());
    fixture.unchanged();
}

#[tokio::test]
async fn pending_apply_recovery_finalizes_exact_candidate_without_rollback() {
    let fixture = Fixture::new(None);
    let battle =
        battle::prepare(&fixture.store, &fixture.project_id, "Pending apply fixture").unwrap();
    battle::execute(&fixture.store, &battle.id, &Fixture::plan())
        .await
        .unwrap();
    let id = fixture.store.growth_variants(&battle.id).unwrap()[0]
        .id
        .clone();
    let transaction = fixture.store.begin().unwrap();
    transaction
        .execute_batch("CREATE TRIGGER fixture_pending_apply BEFORE UPDATE ON growth_selections WHEN NEW.status='done' BEGIN SELECT RAISE(ABORT,'synthetic receipt finalization failure'); END;")
        .unwrap();
    transaction.commit().unwrap();
    assert!(super::selection::apply(&fixture.store, &id).is_err());
    let pending = fixture
        .store
        .growth_selections(&battle.id)
        .unwrap()
        .into_iter()
        .find(|receipt| receipt.action == super::selection::SelectionAction::Apply)
        .expect("apply intent is durable before the working-tree write");
    assert_eq!(pending.status, super::selection::SelectionStatus::Pending);
    assert_eq!(
        std::fs::read_to_string(fixture.product.join("website/index.html")).unwrap(),
        "<h1>Outcome-first</h1>"
    );
    std::fs::write(
        fixture.product.join("website/index.html"),
        "<h1>Builder's local recovery conflict</h1>",
    )
    .unwrap();
    let conflict = super::selection::recover_delivery(&fixture.store, &pending.id, true).unwrap();
    assert_eq!(conflict.state, "conflict");
    assert_eq!(conflict.status, super::selection::SelectionStatus::Pending);
    assert_eq!(
        std::fs::read_to_string(fixture.product.join("website/index.html")).unwrap(),
        "<h1>Builder's local recovery conflict</h1>"
    );
    std::fs::write(
        fixture.product.join("website/index.html"),
        "<h1>Outcome-first</h1>",
    )
    .unwrap();
    let transaction = fixture.store.begin().unwrap();
    transaction
        .execute_batch("DROP TRIGGER fixture_pending_apply;")
        .unwrap();
    transaction.commit().unwrap();
    let recovered = super::selection::recover_delivery(&fixture.store, &pending.id, false).unwrap();
    assert_eq!(recovered.state, "candidate");
    assert_eq!(recovered.status, super::selection::SelectionStatus::Done);
    assert!(!recovered.resumed);
    assert_eq!(
        super::selection::recover_delivery(&fixture.store, &pending.id, true)
            .unwrap()
            .state,
        "terminal"
    );
    assert_eq!(
        git(&fixture.product, &["rev-parse", "HEAD"]),
        fixture.commit
    );
    assert_eq!(
        git(&fixture.product, &["diff", "--cached", "--name-only"]),
        ""
    );
    assert_eq!(git(&fixture.product, &["remote"]), "");
}

#[tokio::test]
async fn selected_preflight_never_executes_repository_content_filters() {
    let fixture = Fixture::new(None);
    let battle = battle::prepare(
        &fixture.store,
        &fixture.project_id,
        "Filter boundary fixture",
    )
    .unwrap();
    battle::execute(&fixture.store, &battle.id, &Fixture::plan())
        .await
        .unwrap();
    let id = fixture.store.growth_variants(&battle.id).unwrap()[0]
        .id
        .clone();
    let probe = "node -e \"require('node:fs').writeFileSync('filter-executed.txt','unexpected')\"";
    git(&fixture.product, &["config", "diff.external", probe]);
    git(&fixture.product, &["config", "core.fsmonitor", probe]);
    super::selection::preview_apply(&fixture.store, &id).unwrap();
    assert!(
        !fixture.product.join("filter-executed.txt").exists(),
        "diff/fsmonitor hooks must not execute during safe inspection"
    );
    git(
        &fixture.product,
        &[
            "config",
            "filter.fixture.clean",
            "node -e \"require('node:fs').writeFileSync('filter-executed.txt','unexpected')\"",
        ],
    );
    std::fs::write(
        fixture.product.join(".gitattributes"),
        "*.html filter=fixture\n",
    )
    .unwrap();
    std::fs::write(
        fixture.product.join("website/index.html"),
        "<h1>Local work would trigger a clean filter during status</h1>",
    )
    .unwrap();
    let error = super::selection::preview_apply(&fixture.store, &id).unwrap_err();
    assert!(
        error.to_string().contains("content filters"),
        "the capability boundary must fail before ordinary dirty-state inspection"
    );
    assert!(!fixture.product.join("filter-executed.txt").exists());
    assert!(
        std::fs::read_to_string(fixture.product.join("website/index.html"))
            .unwrap()
            .contains("Local work")
    );
}

#[tokio::test]
async fn reports_withhold_private_context_escape_explicit_context_and_verify_seals() {
    let fixture = Fixture::new(None);
    let private_goal =
        "PrivateProjectUnique goal <script>bad()</script> ![x](https://example.invalid/pixel)";
    let battle = battle::prepare(&fixture.store, &fixture.project_id, private_goal).unwrap();
    let mut agent = Fixture::plan();
    for implementation in &mut agent.0.implementations {
        implementation.summary =
            "PrivateSummaryUnique <img src='https://example.invalid/pixel'>".into();
    }
    battle::execute(&fixture.store, &battle.id, &agent)
        .await
        .unwrap();
    let variants = fixture.store.growth_variants(&battle.id).unwrap();
    super::selection::select(&fixture.store, &variants[0].id).unwrap();
    let options = super::report::ReportOptions::default();
    let report = super::report::build(&fixture.store, &battle.id, &options).unwrap();
    assert_eq!(report.selected_candidate, Some(1));
    assert!(!report.visuals_disclosed);
    assert!(report
        .variants
        .iter()
        .all(|variant| variant.screenshots.is_empty()));
    let output = super::report::document(&report);
    let markdown = super::report::markdown(&report);
    let json = serde_json::to_string(&report).unwrap();
    for text in [&output, &markdown, &json] {
        for private in [
            "PrivateProjectUnique",
            "PrivateSummaryUnique",
            "website/index.html",
            "website/check.mjs",
            "Observed heading check passed",
            fixture.product.to_str().unwrap(),
        ] {
            assert!(
                !text.contains(private),
                "private input leaked into default report"
            );
        }
        assert!(
            text.contains("SIMULATED") && text.contains("OBSERVED") && text.contains("UNTESTED")
        );
        assert!(text.contains(&battle.contract_digest));
    }
    assert!(output.contains("Suggested next steps"));
    assert!(output.contains("Hypothesis ID:"));
    assert!(markdown.contains("Suggested next steps"));
    assert!(markdown.contains("Hypothesis ID"));
    assert_eq!(report.variants[1].checks[0].exit_code, Some(2));
    let disclosed = super::report::build(
        &fixture.store,
        &battle.id,
        &super::report::ReportOptions {
            include_context: true,
            without_attribution: true,
            public_goal: None,
            include_visuals: false,
        },
    )
    .unwrap();
    let html = super::report::document(&disclosed);
    assert!(html.contains("PrivateProjectUnique") && html.contains("&lt;script&gt;"));
    assert!(!html.contains("<script>") && !html.contains("<img ") && !html.contains("<footer>"));
    assert!(!super::report::markdown(&disclosed).contains("![x]"));
    let output_path = fixture.root.join("report.html");
    super::report::export(&fixture.store, &battle.id, &options, &output_path, false).unwrap();
    assert!(
        super::report::export(&fixture.store, &battle.id, &options, &output_path, false).is_err()
    );
    assert!(super::report::export(
        &fixture.store,
        &battle.id,
        &options,
        &fixture.product.join("report.html"),
        false
    )
    .is_err());
    let seal = fixture.store.sealed_growth_runs(&battle.id).unwrap()[0].clone();
    let path = fixture
        .root
        .join("lab/growth-archives")
        .join(seal.archive_digest)
        .join("implementation.diff");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    std::fs::write(path, "tampered").unwrap();
    assert!(super::report::build(&fixture.store, &battle.id, &options).is_err());
    assert!(super::selection::export(
        &fixture.store,
        &variants[0].id,
        &fixture.root.join("tampered.patch")
    )
    .is_err());
}

#[tokio::test]
async fn sealed_report_exports_when_original_checkout_is_unavailable() {
    let fixture = Fixture::new(None);
    let battle = battle::prepare(
        &fixture.store,
        &fixture.project_id,
        "Unavailable checkout fixture",
    )
    .unwrap();
    battle::execute(&fixture.store, &battle.id, &Fixture::plan())
        .await
        .unwrap();
    let moved = fixture.root.join("moved-product");
    std::fs::rename(&fixture.product, &moved).unwrap();
    let output = fixture.root.join("recovered-report.html");
    super::report::export(
        &fixture.store,
        &battle.id,
        &super::report::ReportOptions::default(),
        &output,
        false,
    )
    .unwrap();
    assert!(std::fs::read_to_string(output)
        .unwrap()
        .contains(&battle.contract_digest));
    std::fs::rename(moved, &fixture.product).unwrap();
    fixture.unchanged();
}

#[test]
fn moving_product_head_does_not_move_the_battle_baseline() {
    let fixture = Fixture::new(None);
    std::fs::write(
        fixture.product.join("website/index.html"),
        "<h1>Builder's newer work</h1>",
    )
    .unwrap();
    git(&fixture.product, &["add", "website/index.html"]);
    git(
        &fixture.product,
        &["commit", "-m", "Builder continues independently"],
    );
    let newer = git(&fixture.product, &["rev-parse", "HEAD"]);
    let battle =
        battle::prepare(&fixture.store, &fixture.project_id, "Pinned source fixture").unwrap();
    assert_eq!(battle.contract.source_snapshot_commit, fixture.commit);
    for variant in fixture.store.growth_variants(&battle.id).unwrap() {
        assert_eq!(
            git(Path::new(&variant.worktree), &["rev-parse", "HEAD"]),
            fixture.commit
        );
    }
    assert_eq!(git(&fixture.product, &["rev-parse", "HEAD"]), newer);
    assert!(
        std::fs::read_to_string(fixture.product.join("website/index.html"))
            .unwrap()
            .contains("Builder's newer work")
    );
}

#[test]
fn failed_registration_compensates_only_new_lab_branches_and_worktrees() {
    let fixture = Fixture::new(None);
    let refs = git(&fixture.product, &["for-each-ref", "--format=%(refname)"]);
    let transaction = fixture.store.begin().unwrap();
    transaction.execute_batch("CREATE TRIGGER fixture_registration_failure BEFORE INSERT ON local_experiments BEGIN SELECT RAISE(ABORT, 'synthetic fixture failure'); END;").unwrap();
    transaction.commit().unwrap();
    assert!(battle::prepare(
        &fixture.store,
        &fixture.project_id,
        "Registration failure fixture"
    )
    .is_err());
    assert_eq!(
        git(&fixture.product, &["for-each-ref", "--format=%(refname)"]),
        refs
    );
    assert_eq!(
        git(&fixture.product, &["worktree", "list", "--porcelain"])
            .lines()
            .filter(|line| line.starts_with("worktree "))
            .count(),
        1
    );
    assert!(fixture
        .store
        .list_experiments_by_project(&fixture.project_id)
        .unwrap()
        .is_empty());
    assert!(fixture
        .store
        .list_growth_battles(Some(&fixture.project_id))
        .unwrap()
        .is_empty());
    fixture.unchanged();
}

struct Fixture {
    root: PathBuf,
    product: PathBuf,
    store: Store,
    project_id: String,
    commit: String,
    _cleanup: FixtureCleanup,
}

fn git(root: &Path, args: &[&str]) -> String {
    let mut options = vec![
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.hooksPath=.disabled-fixture-hooks",
    ];
    options.extend_from_slice(args);
    crate::local::git::git(Some(root), &options).unwrap()
}

impl Fixture {
    fn new(command: Option<&str>) -> Self {
        Self::with_timeout(command, 1)
    }
    fn with_timeout(command: Option<&str>, timeout: u64) -> Self {
        let root =
            std::env::temp_dir().join(format!("growth-battle-fixture-{}", uuid::Uuid::new_v4()));
        let product = root.join("product");
        std::fs::create_dir_all(product.join("website")).unwrap();
        let mut config = super::config::fixture();
        if let Some(command) = command {
            config.validation.commands = vec![command.into()];
            config.validation.timeout_seconds = timeout;
        }
        config.write_new(&product).unwrap();
        std::fs::write(
            product.join("website/index.html"),
            "<!doctype html><h1>Baseline</h1>",
        )
        .unwrap();
        std::fs::write(
            product.join("website/retired.txt"),
            "Synthetic obsolete copy",
        )
        .unwrap();
        std::fs::write(product.join("website/check.mjs"),"import {readFileSync} from 'node:fs'; if (!readFileSync('website/index.html','utf8').includes('<h1>')) process.exit(2); console.log('Observed heading check passed');").unwrap();
        git(&product, &["init", "-b", "main"]);
        git(&product, &["add", "growthlab.yaml", "website"]);
        git(&product, &["commit", "-m", "Synthetic public product"]);
        let store = Store::open_at(root.join("lab")).unwrap();
        let workspace = super::cli::import(&store, &product).unwrap();
        Self {
            _cleanup: FixtureCleanup(root.clone()),
            root,
            product,
            store,
            project_id: workspace.project_id,
            commit: workspace.source_snapshot_commit,
        }
    }
    fn plan() -> ReplayAgent {
        ReplayAgent(ReplayPlan {
            version: 1,
            implementations: [
                "<h1>Outcome-first</h1>",
                "No heading: deliberate failing fixture",
                "<h1>First success</h1>",
            ]
            .into_iter()
            .map(|html| Implementation {
                summary: "Declared synthetic strategy".into(),
                files: vec![FileEdit {
                    path: "website/index.html".into(),
                    contents: Some(html.into()),
                }],
                risks: vec!["No real growth outcome".into()],
            })
            .collect(),
        })
    }
    fn unchanged(&self) {
        assert_eq!(git(&self.product, &["rev-parse", "HEAD"]), self.commit);
        assert_eq!(git(&self.product, &["status", "--porcelain"]), "");
        assert_eq!(git(&self.product, &["remote"]), "");
        assert_eq!(
            std::fs::read_to_string(self.product.join("website/index.html")).unwrap(),
            "<!doctype html><h1>Baseline</h1>"
        );
    }
}

struct FixtureCleanup(PathBuf);
impl Drop for FixtureCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn real_worktrees_snapshots_failure_evidence_and_tamper_refusal() {
    let fixture = Fixture::new(None);
    let battle = battle::prepare(
        &fixture.store,
        &fixture.project_id,
        "Improve qualified activation",
    )
    .unwrap();
    let variants = fixture.store.growth_variants(&battle.id).unwrap();
    assert_eq!(variants.len(), 3);
    for variant in &variants {
        assert_eq!(
            git(Path::new(&variant.worktree), &["rev-parse", "HEAD"]),
            fixture.commit
        );
    }
    assert_ne!(variants[0].worktree, variants[1].worktree);
    fixture.unchanged();
    let result = battle::execute(&fixture.store, &battle.id, &Fixture::plan())
        .await
        .unwrap();
    assert_eq!(
        result.status,
        BattleStatus::Failed,
        "one actual command fails"
    );
    let comparison = super::evaluation::compare(&fixture.store, &battle.id).unwrap();
    assert_eq!(comparison.recommended_candidates.len(), 2);
    assert!(comparison
        .rows
        .iter()
        .all(|row| row.outcome_provenance == Provenance::Untested));
    assert!(comparison
        .rows
        .iter()
        .all(|row| row.implementation_provenance == Provenance::Simulated
            && row.check_provenance == Provenance::Observed));
    assert_eq!(comparison.rows[1].checks[0].exit_code, Some(2));
    let runs = fixture.store.sealed_growth_runs(&battle.id).unwrap();
    assert_eq!(runs.len(), 3);
    assert_eq!(
        fixture
            .store
            .list_runs_by_project(&fixture.project_id)
            .unwrap()
            .len(),
        3,
        "validations reuse generic run persistence"
    );
    for sealed in &runs {
        let archive = archive::verify(fixture.store.data_root(), &sealed.archive_digest).unwrap();
        assert!(archive.contains_key("implementation.diff"));
        assert!(archive.contains_key("validation-0.log"));
        assert_eq!(
            sealed.run.validations[0].source_commit,
            sealed.run.candidate_commit.clone().unwrap()
        );
        assert!(
            fixture.store.save_growth_attempt(&sealed.run).is_err(),
            "a sealed outcome cannot be overwritten"
        );
    }
    assert!(
        battle::execute(&fixture.store, &battle.id, &Fixture::plan())
            .await
            .is_err(),
        "an immutable run is never silently rerun"
    );
    fixture.unchanged();
    let path = fixture
        .store
        .data_root()
        .join("growth-archives")
        .join(&runs[0].archive_digest)
        .join("implementation.diff");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    std::fs::write(path, "tampered").unwrap();
    assert!(super::evaluation::compare(&fixture.store, &battle.id).is_err());
}

#[tokio::test]
async fn invalid_file_proposals_are_sealed_without_outside_writes() {
    let fixture = Fixture::new(None);
    let battle = battle::prepare(
        &fixture.store,
        &fixture.project_id,
        "Invalid proposal fixture",
    )
    .unwrap();
    let mut agent = Fixture::plan();
    agent.0.implementations[0].files[0].path = "../outside".into();
    agent.0.implementations[1].files[0].path = "website/.env".into();
    agent.0.implementations[2].files[0].contents = Some("API_KEY=fixture-private-value".into());
    let result = battle::execute(&fixture.store, &battle.id, &agent)
        .await
        .unwrap();
    assert_eq!(result.status, BattleStatus::Failed);
    assert!(super::evaluation::compare(&fixture.store, &battle.id)
        .unwrap()
        .recommended_candidates
        .is_empty());
    for sealed in fixture.store.sealed_growth_runs(&battle.id).unwrap() {
        assert!(sealed.run.candidate_commit.is_none());
        for bytes in archive::verify(fixture.store.data_root(), &sealed.archive_digest)
            .unwrap()
            .values()
        {
            assert!(!String::from_utf8_lossy(bytes).contains("fixture-private-value"));
        }
        assert!(
            archive::verify(fixture.store.data_root(), &sealed.archive_digest)
                .unwrap()
                .contains_key("agent.log"),
            "the invalid proposal was actually received before rejection"
        );
    }
    assert!(!fixture.root.join("outside").exists());
    fixture.unchanged();
}

#[test]
fn tracked_credentials_are_refused_before_checkout_or_archival() {
    let fixture = Fixture::new(None);
    std::fs::write(fixture.product.join(".env"), "Synthetic private fixture").unwrap();
    git(&fixture.product, &["add", ".env"]);
    git(
        &fixture.product,
        &["commit", "-m", "Credential filename fixture"],
    );
    let commit = git(&fixture.product, &["rev-parse", "HEAD"]);
    assert!(battle::safe_source(&fixture.product, &commit, &super::config::fixture()).is_err());
    assert!(!fixture.store.data_root().join("source-snapshots").exists());
    assert!(!fixture.store.data_root().join("growth-worktrees").exists());
}

struct WaitingAgent;
#[async_trait::async_trait(?Send)]
impl BattleAgent for WaitingAgent {
    fn metadata(&self) -> AgentMetadata {
        Fixture::plan().metadata()
    }
    fn provenance(&self) -> Provenance {
        Provenance::Simulated
    }
    async fn implement(&self, _input: AgentInput<'_>) -> crate::error::Result<Implementation> {
        std::future::pending().await
    }
}

#[tokio::test]
async fn cancellation_during_proposal_seals_every_started_attempt() {
    let fixture = Fixture::new(None);
    let battle =
        battle::prepare(&fixture.store, &fixture.project_id, "Cancellation fixture").unwrap();
    let other = Store::open_at(fixture.root.join("lab")).unwrap();
    let execution = battle::execute(&fixture.store, &battle.id, &WaitingAgent);
    let cancel = async {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(other.request_growth_battle_cancel(&battle.id).unwrap());
    };
    let (result, ()) = tokio::join!(execution, cancel);
    assert_eq!(result.unwrap().status, BattleStatus::Cancelled);
    let runs = fixture.store.sealed_growth_runs(&battle.id).unwrap();
    assert_eq!(runs.len(), 3);
    assert!(runs.iter().all(|run| run.run.status == "cancelled"));
    fixture.unchanged();
}

#[tokio::test]
async fn timeout_kills_a_child_that_ignores_term() {
    let fixture=Fixture::new(Some("node -e \"process.on('SIGTERM',()=>{}); setTimeout(()=>require('fs').writeFileSync('late-marker','bad'),2500)\""));
    let battle = battle::prepare(&fixture.store, &fixture.project_id, "Timeout fixture").unwrap();
    let result = battle::execute(&fixture.store, &battle.id, &Fixture::plan())
        .await
        .unwrap();
    assert_eq!(result.status, BattleStatus::Failed);
    let runs = fixture.store.sealed_growth_runs(&battle.id).unwrap();
    assert!(runs
        .iter()
        .all(|run| run.run.validations[0].termination_reason.as_deref() == Some("timeout")));
    tokio::time::sleep(std::time::Duration::from_millis(1600)).await;
    for run in fixture
        .store
        .list_runs_by_project(&fixture.project_id)
        .unwrap()
    {
        let descriptor = crate::jobs::BackendDescriptor::parse(&run.backend_json).unwrap();
        assert!(
            !PathBuf::from(descriptor.job_id.unwrap())
                .join("repo/late-marker")
                .exists(),
            "a timed-out descendant must not execute later"
        );
    }
    fixture.unchanged();
}

#[tokio::test]
async fn unknown_tool_isolation_fails_without_provider_execution() {
    let fixture = Fixture::new(None);
    let battle = battle::prepare(
        &fixture.store,
        &fixture.project_id,
        "Native capability fixture",
    )
    .unwrap();
    let agent = HarnessAgent {
        harness_id: "codex".into(),
        model: None,
        timeout_seconds: 1,
    };
    let result = battle::execute(&fixture.store, &battle.id, &agent)
        .await
        .unwrap();
    assert_eq!(result.status, BattleStatus::Failed);
    assert!(fixture
        .store
        .sealed_growth_runs(&battle.id)
        .unwrap()
        .iter()
        .all(|run| run
            .run
            .error
            .as_ref()
            .unwrap()
            .contains("no provider request was made")));
    fixture.unchanged();
}
