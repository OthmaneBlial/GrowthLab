use std::path::{Path, PathBuf};

use super::archive;
use super::battle::{self, AgentInput, BattleAgent, HarnessAgent, ReplayAgent};
use super::battle_model::*;
use super::model::Provenance;
use crate::store::Store;

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
        let root =
            std::env::temp_dir().join(format!("growth-battle-fixture-{}", uuid::Uuid::new_v4()));
        let product = root.join("product");
        std::fs::create_dir_all(product.join("website")).unwrap();
        let mut config = super::config::fixture();
        if let Some(command) = command {
            config.validation.commands = vec![command.into()];
            config.validation.timeout_seconds = 1;
        }
        config.write_new(&product).unwrap();
        std::fs::write(
            product.join("website/index.html"),
            "<!doctype html><h1>Baseline</h1>",
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
