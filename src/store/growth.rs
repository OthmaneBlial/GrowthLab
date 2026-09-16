//! Transactional growth extensions. Their migration ledger is independent of
//! the inherited transcript user_version and never swallows SQL failures.
use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{anyhow, Result};
use crate::growth::{
    model::{GrowthHypothesis, GrowthWorkspace},
    playbooks::PlaybookRun,
};
use crate::local::model::LocalProject;

use super::{Store, PROJECT_COLS};

pub(super) fn migrate(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS growth_schema_migrations (
        version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL
    );",
    )?;
    let version: i64 = tx.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM growth_schema_migrations",
        [],
        |row| row.get(0),
    )?;
    if version > 5 {
        return Err(anyhow!(
            "GrowthLab database was created by a newer version; no growth migration applied"
        ));
    }
    if version < 1 {
        tx.execute_batch(
            "CREATE TABLE growth_workspaces (
            project_id TEXT PRIMARY KEY REFERENCES local_projects(id),
            config_json TEXT NOT NULL,
            source_snapshot_commit TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE growth_hypotheses (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES growth_workspaces(project_id),
            payload_json TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE INDEX idx_growth_hypotheses_project ON growth_hypotheses(project_id, created_at);
        CREATE TRIGGER cleanup_growth_project AFTER DELETE ON local_projects BEGIN
            DELETE FROM growth_hypotheses WHERE project_id = OLD.id;
            DELETE FROM growth_workspaces WHERE project_id = OLD.id;
        END;",
        )?;
        tx.execute(
            "INSERT INTO growth_schema_migrations (version, applied_at) VALUES (1, ?1)",
            [super::now_ms()],
        )?;
    }
    if version < 2 {
        tx.execute_batch(
            "CREATE TABLE growth_battles (
            id TEXT PRIMARY KEY, project_id TEXT NOT NULL,
            contract_json TEXT NOT NULL, contract_digest TEXT NOT NULL,
            status TEXT NOT NULL, created_at INTEGER NOT NULL,
            ended_at INTEGER, cancel_requested INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX idx_growth_battles_project ON growth_battles(project_id, created_at);
        CREATE TABLE growth_variants (
            id TEXT PRIMARY KEY, battle_id TEXT NOT NULL, payload_json TEXT NOT NULL
        );
        CREATE INDEX idx_growth_variants_battle ON growth_variants(battle_id);
        CREATE TABLE growth_battle_runs (
            id TEXT PRIMARY KEY, variant_id TEXT NOT NULL UNIQUE,
            battle_id TEXT NOT NULL, payload_json TEXT NOT NULL, archive_digest TEXT
        );
        CREATE TRIGGER freeze_sealed_growth_run BEFORE UPDATE ON growth_battle_runs
        WHEN OLD.archive_digest IS NOT NULL BEGIN
            SELECT RAISE(ABORT, 'sealed growth run is immutable');
        END;",
        )?;
        tx.execute(
            "INSERT INTO growth_schema_migrations (version, applied_at) VALUES (2, ?1)",
            [super::now_ms()],
        )?;
    }
    if version < 3 {
        tx.execute_batch(
            "CREATE TABLE growth_selections (
                id TEXT PRIMARY KEY, battle_id TEXT NOT NULL,
                variant_id TEXT NOT NULL, payload_json TEXT NOT NULL,
                status TEXT NOT NULL, created_at INTEGER NOT NULL
            );
            CREATE INDEX idx_growth_selections_battle ON growth_selections(battle_id,created_at);
            CREATE TRIGGER freeze_finished_growth_selection BEFORE UPDATE ON growth_selections
            WHEN OLD.status != 'pending' BEGIN
                SELECT RAISE(ABORT, 'finished growth selection is immutable');
            END;
            CREATE TRIGGER cleanup_growth_battle_project AFTER DELETE ON local_projects BEGIN
                DELETE FROM growth_selections WHERE battle_id IN (SELECT id FROM growth_battles WHERE project_id=OLD.id);
                DELETE FROM growth_battle_runs WHERE battle_id IN (SELECT id FROM growth_battles WHERE project_id=OLD.id);
                DELETE FROM growth_variants WHERE battle_id IN (SELECT id FROM growth_battles WHERE project_id=OLD.id);
                DELETE FROM growth_battles WHERE project_id=OLD.id;
            END;",
        )?;
        tx.execute(
            "INSERT INTO growth_schema_migrations (version, applied_at) VALUES (3, ?1)",
            [super::now_ms()],
        )?;
    }
    if version < 4 {
        tx.execute_batch("ALTER TABLE growth_battle_runs ADD COLUMN checkpoint_digest TEXT;")?;
        tx.execute(
            "INSERT INTO growth_schema_migrations (version, applied_at) VALUES (4, ?1)",
            [super::now_ms()],
        )?;
    }
    if version < 5 {
        tx.execute_batch(
            "CREATE TABLE growth_playbook_runs (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL REFERENCES growth_workspaces(project_id),
                role TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX idx_growth_playbook_runs_project ON growth_playbook_runs(project_id, created_at);",
        )?;
        let has_local_projects: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='local_projects')",
            [],
            |row| row.get(0),
        )?;
        // A migration unit test may intentionally provide only the growth
        // tables. Real stores always have local_projects, where the cleanup
        // trigger prevents orphaned local role-contract artifacts.
        if has_local_projects {
            tx.execute_batch(
                "CREATE TRIGGER cleanup_growth_playbook_project AFTER DELETE ON local_projects BEGIN
                    DELETE FROM growth_playbook_runs WHERE project_id = OLD.id;
                END;",
            )?;
        }
        tx.execute(
            "INSERT INTO growth_schema_migrations (version, applied_at) VALUES (5, ?1)",
            [super::now_ms()],
        )?;
    }
    tx.commit()?;
    Ok(())
}

impl Store {
    pub fn register_growth_workspace(
        &self,
        project: &LocalProject,
        workspace: &GrowthWorkspace,
    ) -> Result<()> {
        workspace.validate()?;
        if project.id != workspace.project_id || project.github_sync_enabled {
            return Err(anyhow!(
                "Growth workspace must reference its local project with publication disabled"
            ));
        }
        let config_json = serde_json::to_string(&workspace.config)?;
        let tx = self.begin()?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM local_projects WHERE repo_path = ?1)",
            [&project.repo_path],
            |row| row.get(0),
        )?;
        if exists {
            return Err(anyhow!("Product repository is already registered"));
        }
        tx.execute(&format!("INSERT INTO local_projects ({PROJECT_COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)"), params![
            project.id, project.name, project.slug, project.github_owner, project.github_repo,
            project.github_sync_enabled, project.baseline_branch, project.repo_path, project.run_command,
            project.paper_id, project.created_at, project.updated_at,
        ])?;
        tx.execute("INSERT INTO growth_workspaces (project_id, config_json, source_snapshot_commit, created_at) VALUES (?1,?2,?3,?4)", params![workspace.project_id, config_json, workspace.source_snapshot_commit, workspace.created_at])?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_growth_workspace(&self, project_id: &str) -> Result<Option<GrowthWorkspace>> {
        let row: Option<(String,String,i64)> = self.conn.query_row(
            "SELECT config_json, source_snapshot_commit, created_at FROM growth_workspaces WHERE project_id = ?1",
            [project_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).optional()?;
        row.map(|(json, commit, created)| {
            let config: crate::growth::config::GrowthConfig = serde_json::from_str(&json)
                .map_err(|_| anyhow!("Stored growth workspace has invalid configuration"))?;
            let workspace = GrowthWorkspace {
                project_id: project_id.into(),
                config,
                source_snapshot_commit: commit,
                created_at: created,
            };
            workspace.validate()?;
            Ok(workspace)
        })
        .transpose()
    }

    pub fn list_growth_workspaces(&self) -> Result<Vec<GrowthWorkspace>> {
        let mut query = self
            .conn
            .prepare("SELECT project_id FROM growth_workspaces ORDER BY created_at, project_id")?;
        let ids = query
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| {
                self.get_growth_workspace(&id)?
                    .ok_or_else(|| anyhow!("Growth workspace disappeared"))
            })
            .collect()
    }

    /// Insert an entire portfolio or none. These proposal records are not run
    /// results; all outcomes remain untested until execution provides evidence.
    pub fn insert_growth_hypotheses(
        &self,
        project_id: &str,
        hypotheses: &[GrowthHypothesis],
    ) -> Result<()> {
        let workspace = self
            .get_growth_workspace(project_id)?
            .ok_or_else(|| anyhow!("Growth workspace not found"))?;
        for hypothesis in hypotheses {
            hypothesis.validate()?;
            if hypothesis.project_id != project_id
                || hypothesis.source_snapshot_commit != workspace.source_snapshot_commit
            {
                return Err(anyhow!(
                    "Hypothesis must use the workspace and its recorded source snapshot"
                ));
            }
        }
        let tx = self.begin()?;
        for hypothesis in hypotheses {
            tx.execute("INSERT INTO growth_hypotheses (id, project_id, payload_json, created_at) VALUES (?1,?2,?3,?4)", params![hypothesis.id, project_id, serde_json::to_string(hypothesis)?, hypothesis.created_at])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_growth_hypotheses(&self, project_id: &str) -> Result<Vec<GrowthHypothesis>> {
        if self.get_growth_workspace(project_id)?.is_none() {
            return Err(anyhow!("Growth workspace not found"));
        }
        let mut query = self.conn.prepare("SELECT payload_json FROM growth_hypotheses WHERE project_id = ?1 ORDER BY created_at, rowid")?;
        let values = query
            .query_map([project_id], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        values
            .into_iter()
            .map(|value| {
                let hypothesis: GrowthHypothesis = serde_json::from_str(&value)
                    .map_err(|_| anyhow!("Stored growth hypothesis has invalid schema"))?;
                hypothesis.validate()?;
                Ok(hypothesis)
            })
            .collect()
    }

    /// Replace the user-editable fields of one hypothesis while keeping its
    /// identity, source snapshot and recorded evidence stable. A prepared or
    /// running battle owns a frozen copy, so the workspace map cannot drift
    /// underneath it.
    pub fn update_growth_hypothesis(
        &self,
        project_id: &str,
        hypothesis: &GrowthHypothesis,
    ) -> Result<()> {
        let workspace = self
            .get_growth_workspace(project_id)?
            .ok_or_else(|| anyhow!("Growth workspace not found"))?;
        hypothesis.validate()?;
        if hypothesis.project_id != project_id
            || hypothesis.source_snapshot_commit != workspace.source_snapshot_commit
        {
            return Err(anyhow!(
                "Hypothesis must use the workspace and its recorded source snapshot"
            ));
        }
        let existing_json: Option<String> = self
            .conn
            .query_row(
                "SELECT payload_json FROM growth_hypotheses WHERE id=?1 AND project_id=?2",
                params![hypothesis.id, project_id],
                |row| row.get(0),
            )
            .optional()?;
        let existing: GrowthHypothesis = existing_json
            .ok_or_else(|| anyhow!("Growth hypothesis not found"))
            .and_then(|value| {
                serde_json::from_str(&value)
                    .map_err(|_| anyhow!("Stored growth hypothesis has invalid schema"))
            })?;
        if existing.id != hypothesis.id
            || existing.project_id != hypothesis.project_id
            || existing.goal != hypothesis.goal
            || existing.audience != hypothesis.audience
            || existing.channel != hypothesis.channel
            || existing.role != hypothesis.role
            || existing.source_snapshot_commit != hypothesis.source_snapshot_commit
            || existing.evidence != hypothesis.evidence
            || existing.provenance != hypothesis.provenance
            || existing.confidence != hypothesis.confidence
            || existing.decision != hypothesis.decision
            || existing.created_at != hypothesis.created_at
        {
            return Err(anyhow!(
                "Only hypothesis wording, metric, thresholds and risks can be edited"
            ));
        }
        let tx = self.begin()?;
        let active_battle: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM growth_battles WHERE project_id=?1 AND status IN ('ready','running'))",
            [project_id],
            |row| row.get(0),
        )?;
        if active_battle {
            return Err(anyhow!(
                "Hypotheses cannot be edited while a Growth Battle is ready or running"
            ));
        }
        tx.execute(
            "UPDATE growth_hypotheses SET payload_json=?1 WHERE id=?2 AND project_id=?3",
            params![
                serde_json::to_string(hypothesis)?,
                hypothesis.id,
                project_id
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn insert_growth_playbook_run(&self, run: &PlaybookRun) -> Result<()> {
        run.validate()?;
        if self.get_growth_workspace(&run.project_id)?.is_none() {
            return Err(anyhow!("Growth workspace not found"));
        }
        let payload = serde_json::to_string(run)?;
        self.conn.execute(
            "INSERT INTO growth_playbook_runs (id, project_id, role, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![run.id, run.project_id, serde_json::to_string(&run.role)?, payload, run.created_at],
        )?;
        Ok(())
    }

    pub fn list_growth_playbook_runs(&self, project_id: &str) -> Result<Vec<PlaybookRun>> {
        if self.get_growth_workspace(project_id)?.is_none() {
            return Err(anyhow!("Growth workspace not found"));
        }
        let mut query = self.conn.prepare(
            "SELECT payload_json FROM growth_playbook_runs WHERE project_id = ?1 ORDER BY created_at, rowid",
        )?;
        let values = query
            .query_map([project_id], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        values
            .into_iter()
            .map(|value| {
                let run: PlaybookRun = serde_json::from_str(&value)
                    .map_err(|_| anyhow!("Stored playbook run has invalid schema"))?;
                run.validate()?;
                if run.project_id != project_id {
                    return Err(anyhow!("Stored playbook run references another workspace"));
                }
                Ok(run)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_migration_preserves_version_three_records() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE growth_schema_migrations (version INTEGER PRIMARY KEY,applied_at INTEGER); INSERT INTO growth_schema_migrations VALUES(3,1); CREATE TABLE growth_battle_runs(id TEXT PRIMARY KEY,variant_id TEXT,battle_id TEXT,payload_json TEXT,archive_digest TEXT); INSERT INTO growth_battle_runs VALUES('existing','variant','battle','original payload','original seal');").unwrap();
        migrate(&conn).unwrap();
        let (payload,seal,checkpoint):(String,String,Option<String>)=conn.query_row("SELECT payload_json,archive_digest,checkpoint_digest FROM growth_battle_runs WHERE id='existing'",[],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
        assert_eq!(payload, "original payload");
        assert_eq!(seal, "original seal");
        assert_eq!(checkpoint, None);
        assert_eq!(
            conn.query_row(
                "SELECT MAX(version) FROM growth_schema_migrations",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            5
        );
    }

    #[test]
    fn failed_checkpoint_migration_rolls_back_without_advancing_version_or_changing_records() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE growth_schema_migrations (version INTEGER PRIMARY KEY,applied_at INTEGER); INSERT INTO growth_schema_migrations VALUES(3,1); CREATE TABLE growth_battle_runs(id TEXT,payload_json TEXT,checkpoint_digest TEXT); INSERT INTO growth_battle_runs VALUES('existing','original payload','existing checkpoint');").unwrap();
        assert!(migrate(&conn).is_err());
        assert_eq!(
            conn.query_row(
                "SELECT MAX(version) FROM growth_schema_migrations",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            3
        );
        assert_eq!(
            conn.query_row(
                "SELECT payload_json FROM growth_battle_runs WHERE id='existing'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "original payload"
        );
        assert_eq!(
            conn.query_row(
                "SELECT checkpoint_digest FROM growth_battle_runs WHERE id='existing'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "existing checkpoint"
        );
    }

    #[test]
    fn failed_selection_migration_preserves_version_two_and_rolls_back_ddl() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE growth_schema_migrations (version INTEGER PRIMARY KEY,applied_at INTEGER); INSERT INTO growth_schema_migrations VALUES (2,1); CREATE TABLE idx_growth_selections_battle (collision TEXT);").unwrap();
        assert!(migrate(&conn).is_err());
        let version: i64 = conn
            .query_row(
                "SELECT MAX(version) FROM growth_schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 2);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name='growth_selections'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "failed selection migration must not leave a partial table"
        );
    }

    #[test]
    fn selection_migration_preserves_a_version_two_product_and_hypotheses() {
        let root = std::env::temp_dir().join(format!(
            "growth-selection-migration-{}",
            uuid::Uuid::new_v4()
        ));
        let store = Store::open_at(root.clone()).unwrap();
        let (project, workspace) = workspace();
        store
            .register_growth_workspace(&project, &workspace)
            .unwrap();
        let hypotheses = crate::growth::model::starter_hypotheses(&workspace);
        store
            .insert_growth_hypotheses(&project.id, &hypotheses)
            .unwrap();
        let hypotheses = store.list_growth_hypotheses(&project.id).unwrap();
        store.conn.execute_batch("DROP TRIGGER cleanup_growth_battle_project; DROP TRIGGER cleanup_growth_playbook_project; DROP TRIGGER freeze_finished_growth_selection; DROP TABLE growth_selections; DROP TABLE growth_playbook_runs; ALTER TABLE growth_battle_runs DROP COLUMN checkpoint_digest; DELETE FROM growth_schema_migrations WHERE version>=3;").unwrap();
        drop(store);
        let upgraded = Store::open_at(root.clone()).unwrap();
        assert_eq!(
            upgraded.get_growth_workspace(&project.id).unwrap(),
            Some(workspace)
        );
        assert_eq!(
            upgraded.list_growth_hypotheses(&project.id).unwrap(),
            hypotheses
        );
        assert!(upgraded.growth_selections("no-battle").unwrap().is_empty());
        drop(upgraded);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_battle_migration_preserves_version_one_and_rolls_back_tables() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE growth_schema_migrations (version INTEGER PRIMARY KEY,applied_at INTEGER); INSERT INTO growth_schema_migrations VALUES (1,1); CREATE TABLE growth_variants (bad TEXT);").unwrap();
        assert!(migrate(&conn).is_err());
        let version: i64 = conn
            .query_row(
                "SELECT MAX(version) FROM growth_schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 1);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name='growth_battles'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "failed version two must not leave a partial battle schema"
        );
    }

    #[test]
    fn upgrading_a_legacy_store_preserves_its_projects_and_transcript_version() {
        let dir = std::env::temp_dir().join(format!("growthlab-legacy-{}", uuid::Uuid::new_v4()));
        let store = Store::open_at(dir.clone()).unwrap();
        let (project, _) = workspace();
        store.create_local_project(&project).unwrap();
        // Remove only the new extension from this isolated fixture to recreate
        // a pre-GrowthLab store with actual inherited schema and product data.
        store.conn.execute_batch("DROP TRIGGER cleanup_growth_battle_project; DROP TRIGGER cleanup_growth_playbook_project; DROP TRIGGER freeze_finished_growth_selection; DROP TABLE growth_selections; DROP TRIGGER freeze_sealed_growth_run; DROP TABLE growth_battle_runs; DROP TABLE growth_variants; DROP TABLE growth_battles; DROP TRIGGER cleanup_growth_project; DROP TABLE growth_playbook_runs; DROP TABLE growth_hypotheses; DROP TABLE growth_workspaces; DROP TABLE growth_schema_migrations;").unwrap();
        let transcript_version: i64 = store
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        drop(store);
        let store = Store::open_at(dir.clone()).unwrap();
        assert_eq!(
            store
                .get_local_project(&project.id)
                .unwrap()
                .unwrap()
                .repo_path,
            project.repo_path
        );
        let upgraded: i64 = store
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(upgraded, transcript_version);
        assert!(store.list_growth_workspaces().unwrap().is_empty());
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }
    fn workspace() -> (LocalProject, GrowthWorkspace) {
        let workspace = GrowthWorkspace {
            project_id: "p".into(),
            config: crate::growth::config::fixture(),
            source_snapshot_commit: "a".repeat(40),
            created_at: 1,
        };
        let project = LocalProject {
            id: "p".into(),
            name: "Fixture".into(),
            slug: "fixture".into(),
            github_owner: String::new(),
            github_repo: String::new(),
            github_sync_enabled: false,
            baseline_branch: "main".into(),
            repo_path: "/synthetic/fixture".into(),
            run_command: None,
            paper_id: None,
            created_at: 1,
            updated_at: 1,
        };
        (project, workspace)
    }

    #[test]
    fn migrations_roundtrip_idempotency_and_atomic_portfolio_failure() {
        let dir = std::env::temp_dir().join(format!("growthlab-store-{}", uuid::Uuid::new_v4()));
        let store = Store::open_at(dir.clone()).unwrap();
        let (project, workspace) = workspace();
        store
            .register_growth_workspace(&project, &workspace)
            .unwrap();
        assert!(store
            .register_growth_workspace(&project, &workspace)
            .is_err());
        assert_eq!(
            store.get_growth_workspace("p").unwrap(),
            Some(workspace.clone())
        );
        let proposals = crate::growth::model::starter_hypotheses(&workspace);
        let mut duplicate = proposals.clone();
        duplicate[2].id = duplicate[0].id.clone();
        assert!(store.insert_growth_hypotheses("p", &duplicate).is_err());
        assert!(store.list_growth_hypotheses("p").unwrap().is_empty());
        store.insert_growth_hypotheses("p", &proposals).unwrap();
        assert_eq!(store.list_growth_hypotheses("p").unwrap(), proposals);
        let mut edited = proposals[0].clone();
        edited.title = "Sharper outcome-first positioning".into();
        edited.hypothesis = "Lead with the first qualified outcome and one next action.".into();
        edited.primary_metric = "qualified_signup_rate".into();
        store.update_growth_hypothesis("p", &edited).unwrap();
        let stored = store.list_growth_hypotheses("p").unwrap();
        assert_eq!(stored[0].title, edited.title);
        assert_eq!(stored[0].primary_metric, edited.primary_metric);
        assert_eq!(stored[0].evidence, proposals[0].evidence);
        let mut tampered = edited.clone();
        tampered.role = crate::growth::model::AgentRole::Skeptic;
        assert!(store.update_growth_hypothesis("p", &tampered).is_err());
        let mut foreign = proposals.clone();
        foreign[0].source_snapshot_commit = "b".repeat(40);
        assert!(store.insert_growth_hypotheses("p", &foreign).is_err());
        drop(store);
        let store = Store::open_at(dir.clone()).unwrap();
        assert_eq!(store.list_growth_hypotheses("p").unwrap().len(), 3);
        assert_eq!(store.list_growth_workspaces().unwrap(), vec![workspace]);
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn malformed_migration_rolls_back_and_newer_schema_is_rejected() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE local_projects (id TEXT PRIMARY KEY); CREATE TABLE growth_hypotheses (bad TEXT);").unwrap();
        assert!(migrate(&conn).is_err());
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name='growth_workspaces'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "failed migration must not leave a half-created workspace table"
        );
        conn.execute_batch("DROP TABLE growth_hypotheses; CREATE TABLE growth_schema_migrations (version INTEGER PRIMARY KEY, applied_at INTEGER); INSERT INTO growth_schema_migrations VALUES (6,1);").unwrap();
        assert!(migrate(&conn).is_err());
    }
}
