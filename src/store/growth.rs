//! Transactional growth extensions. Their migration ledger is independent of
//! the inherited transcript user_version and never swallows SQL failures.
use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{anyhow, Result};
use crate::growth::model::{GrowthHypothesis, GrowthWorkspace};
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
    if version > 2 {
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
        store.conn.execute_batch("DROP TRIGGER freeze_sealed_growth_run; DROP TABLE growth_battle_runs; DROP TABLE growth_variants; DROP TABLE growth_battles; DROP TRIGGER cleanup_growth_project; DROP TABLE growth_hypotheses; DROP TABLE growth_workspaces; DROP TABLE growth_schema_migrations;").unwrap();
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
        conn.execute_batch("DROP TABLE growth_hypotheses; CREATE TABLE growth_schema_migrations (version INTEGER PRIMARY KEY, applied_at INTEGER); INSERT INTO growth_schema_migrations VALUES (3,1);").unwrap();
        assert!(migrate(&conn).is_err());
    }
}
