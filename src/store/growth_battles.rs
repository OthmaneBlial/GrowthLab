use rusqlite::{params, OptionalExtension};

use crate::error::{anyhow, Result};
use crate::growth::archive::digest;
use crate::growth::battle_model::{
    BattleRun, BattleStatus, GrowthBattle, GrowthVariant, SealedBattleRun,
};
use crate::local::model::LocalExperiment;

use super::{Store, EXPERIMENT_COLS};

fn decode<T: serde::de::DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value).map_err(|_| anyhow!("Invalid stored Growth Battle schema"))
}

impl Store {
    pub fn register_growth_battle(
        &self,
        battle: &GrowthBattle,
        variants: &[(GrowthVariant, LocalExperiment)],
    ) -> Result<()> {
        let workspace = self
            .get_growth_workspace(&battle.project_id)?
            .ok_or_else(|| anyhow!("Growth workspace not found"))?;
        let contract = serde_json::to_string(&battle.contract)?;
        if digest(contract.as_bytes()) != battle.contract_digest
            || battle.contract.source_snapshot_commit != workspace.source_snapshot_commit
            || battle.contract.config != workspace.config
            || battle.status != BattleStatus::Ready
            || variants.len() != 3
            || battle.contract.hypotheses.len() != 3
        {
            return Err(anyhow!(
                "Battle must use a valid frozen workspace contract and three competitors"
            ));
        }
        let tx = self.begin()?;
        tx.execute("INSERT INTO growth_battles (id,project_id,contract_json,contract_digest,status,created_at) VALUES (?1,?2,?3,?4,'ready',?5)", params![battle.id,battle.project_id,contract,battle.contract_digest,battle.created_at])?;
        for ((variant, experiment), hypothesis) in variants.iter().zip(&battle.contract.hypotheses)
        {
            hypothesis.validate()?;
            if variant.id != experiment.id
                || variant.battle_id != battle.id
                || variant.hypothesis_id != hypothesis.id
                || experiment.project_id != battle.project_id
                || variant.branch_name != experiment.branch_name
                || hypothesis.source_snapshot_commit != battle.contract.source_snapshot_commit
                || hypothesis.project_id != battle.project_id
            {
                return Err(anyhow!(
                    "Battle variant must match its hypothesis and inherited experiment"
                ));
            }
            tx.execute(&format!("INSERT INTO local_experiments ({EXPERIMENT_COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)"), params![experiment.id,experiment.project_id,experiment.parent_experiment_id,experiment.slug,experiment.branch_name,experiment.title,experiment.description,experiment.run_command,experiment.agent_status,experiment.created_at,experiment.updated_at,experiment.chat_session_id])?;
            tx.execute(
                "INSERT INTO growth_variants (id,battle_id,payload_json) VALUES (?1,?2,?3)",
                params![variant.id, battle.id, serde_json::to_string(variant)?],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_growth_battle(&self, id: &str) -> Result<Option<GrowthBattle>> {
        self.conn.query_row("SELECT project_id,contract_json,contract_digest,status,created_at,ended_at,cancel_requested FROM growth_battles WHERE id=?1", [id], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,i64>(4)?,row.get::<_,Option<i64>>(5)?,row.get::<_,bool>(6)?))).optional()?.map(|(project,json,hash,status,created,ended,cancel)| {
            if digest(json.as_bytes()) != hash { return Err(anyhow!("Frozen battle contract failed its digest check")); }
            Ok(GrowthBattle { id:id.into(), project_id:project, contract:decode(&json)?, contract_digest:hash, status:decode(&serde_json::to_string(&status)?)?, created_at:created, ended_at:ended, cancel_requested:cancel })
        }).transpose()
    }

    pub fn list_growth_battles(&self, project_id: Option<&str>) -> Result<Vec<GrowthBattle>> {
        let mut query = self.conn.prepare("SELECT id FROM growth_battles WHERE (?1 IS NULL OR project_id=?1) ORDER BY created_at,id")?;
        let ids = query
            .query_map([project_id], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ids.into_iter()
            .map(|id| {
                self.get_growth_battle(&id)?
                    .ok_or_else(|| anyhow!("Battle disappeared"))
            })
            .collect()
    }

    pub fn growth_variants(&self, battle_id: &str) -> Result<Vec<GrowthVariant>> {
        let battle = self
            .get_growth_battle(battle_id)?
            .ok_or_else(|| anyhow!("Battle not found"))?;
        let mut query = self
            .conn
            .prepare("SELECT payload_json FROM growth_variants WHERE battle_id=?1")?;
        let variants = query
            .query_map([battle_id], |row| row.get::<_, String>(0))?
            .map(|row| decode::<GrowthVariant>(&row?))
            .collect::<Result<Vec<_>>>()?;
        battle
            .contract
            .hypotheses
            .iter()
            .map(|hypothesis| {
                variants
                    .iter()
                    .find(|variant| variant.hypothesis_id == hypothesis.id)
                    .cloned()
                    .ok_or_else(|| anyhow!("Battle has a missing variant"))
            })
            .collect()
    }

    pub fn claim_growth_battle(&self, id: &str) -> Result<bool> {
        Ok(self.conn.execute("UPDATE growth_battles SET status='running' WHERE id=?1 AND status='ready' AND cancel_requested=0", [id])? == 1)
    }

    pub fn request_growth_battle_cancel(&self, id: &str) -> Result<bool> {
        Ok(self.conn.execute("UPDATE growth_battles SET cancel_requested=1,ended_at=CASE WHEN status='ready' THEN ?2 ELSE ended_at END,status=CASE WHEN status='ready' THEN 'cancelled' ELSE status END WHERE id=?1 AND status IN ('ready','running')", params![id,super::now_ms()])? == 1)
    }

    pub fn finish_growth_battle(&self, id: &str, status: BattleStatus) -> Result<()> {
        if !matches!(
            status,
            BattleStatus::Completed | BattleStatus::Failed | BattleStatus::Cancelled
        ) {
            return Err(anyhow!("A battle can only finish with a terminal status"));
        }
        self.conn.execute("UPDATE growth_battles SET status=?2,ended_at=?3 WHERE id=?1 AND status IN ('ready','running')", params![id,serde_json::to_value(status)?.as_str(),super::now_ms()])?;
        Ok(())
    }

    pub fn save_growth_attempt(&self, run: &BattleRun) -> Result<()> {
        let variant: String = self.conn.query_row(
            "SELECT battle_id FROM growth_variants WHERE id=?1",
            [&run.variant_id],
            |row| row.get(0),
        )?;
        let battle = self
            .get_growth_battle(&variant)?
            .ok_or_else(|| anyhow!("Battle not found"))?;
        if run.battle_id != variant
            || run.contract_digest != battle.contract_digest
            || run.source_snapshot_commit != battle.contract.source_snapshot_commit
        {
            return Err(anyhow!("Run must match the frozen battle contract"));
        }
        self.conn.execute("INSERT INTO growth_battle_runs (id,variant_id,battle_id,payload_json) VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET payload_json=excluded.payload_json", params![run.id,run.variant_id,run.battle_id,serde_json::to_string(run)?])?;
        Ok(())
    }

    pub fn seal_growth_attempt(&self, run: &BattleRun, hash: &str) -> Result<()> {
        let files = crate::growth::archive::verify(self.data_root(), hash)?;
        if files.get("run.json") != Some(&serde_json::to_vec(run)?) {
            return Err(anyhow!(
                "Sealed run bytes do not match the persisted outcome"
            ));
        }
        let changed = self.conn.execute("UPDATE growth_battle_runs SET payload_json=?2,archive_digest=?3 WHERE id=?1 AND archive_digest IS NULL", params![run.id,serde_json::to_string(run)?,hash])?;
        if changed != 1 {
            return Err(anyhow!("Run was already sealed or was not registered"));
        }
        Ok(())
    }

    pub fn sealed_growth_runs(&self, battle_id: &str) -> Result<Vec<SealedBattleRun>> {
        let mut query = self.conn.prepare("SELECT payload_json,archive_digest FROM growth_battle_runs WHERE battle_id=?1 AND archive_digest IS NOT NULL ORDER BY rowid")?;
        let results = query
            .query_map([battle_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .map(|row| {
                let (json, hash) = row?;
                Ok(SealedBattleRun {
                    run: decode(&json)?,
                    archive_digest: hash,
                })
            })
            .collect();
        results
    }
}
