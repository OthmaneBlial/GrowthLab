use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{Args, Subcommand};

use crate::error::{anyhow, Result};
use crate::local::model::LocalProject;
use crate::store::{now_ms, Store};

use super::config::{
    Agents, Goal, GrowthConfig, Metrics, PermissionMode, Permissions, Product, Validation,
};
use super::model::{starter_hypotheses, GrowthWorkspace};

#[derive(Debug, Args)]
pub struct InitArgs {
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub audience: String,
    #[arg(long)]
    pub goal: String,
    #[arg(long, value_enum, default_value = "analyze-only")]
    pub mode: PermissionMode,
    /// Portable file/directory prefix; repeat to permit multiple paths.
    #[arg(long = "allow")]
    pub allowed_paths: Vec<String>,
    #[arg(long = "deny")]
    pub denied_paths: Vec<String>,
    /// Explicitly authorized project command. Repeated in every competitor.
    #[arg(long = "validate")]
    pub commands: Vec<String>,
    #[arg(long, default_value = "qualified_signup")]
    pub metric: String,
}

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}
#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Validate the schema without changing product files.
    Check {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Inspect whether a file could be changed under the declared policy.
    CheckPath {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        relative: String,
    },
}

#[derive(Debug, Args)]
pub struct WorkspaceArgs {
    #[command(subcommand)]
    pub command: WorkspaceCommand,
}
#[derive(Debug, Subcommand)]
pub enum WorkspaceCommand {
    /// Register a local Git product and its committed growthlab.yaml.
    Import {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    List,
    View {
        project_id: String,
    },
}

#[derive(Debug, Args)]
pub struct HypothesesArgs {
    pub project_id: String,
    /// Read existing proposals; otherwise create three untested templates.
    #[arg(long)]
    pub list: bool,
}

#[derive(Debug, Args)]
pub struct ExecutionArgs {
    /// Declared simulation input. All file changes and checks still execute.
    #[arg(long, conflicts_with = "harness")]
    pub replay: Option<PathBuf>,
    /// Authorize this native CLI to send the allowed product context for proposals.
    #[arg(long, conflicts_with = "replay")]
    pub harness: Option<String>,
    #[arg(long, requires = "harness")]
    pub model: Option<String>,
    #[arg(long, default_value_t = 120)]
    pub agent_timeout_seconds: u64,
}

#[derive(Debug, Args)]
pub struct BattleArgs {
    pub goal: String,
    #[arg(long)]
    pub project: String,
    /// Prepare the frozen contract/worktrees without calling an agent.
    #[arg(long, conflicts_with_all=["replay","harness"])]
    pub prepare_only: bool,
    #[command(flatten)]
    pub execution: ExecutionArgs,
}

#[derive(Debug, Args)]
pub struct BattleRunArgs {
    pub battle_id: String,
    #[command(flatten)]
    pub execution: ExecutionArgs,
}

#[derive(Debug, Args)]
pub struct CompareArgs {
    pub battle_id: String,
}

#[derive(Debug, Args)]
pub struct ExperimentsArgs {
    #[arg(long)]
    pub project: Option<String>,
}

#[derive(Debug, Args)]
pub struct BattleStatusArgs {
    pub battle_id: String,
    /// Persist cancellation intent for the live controller.
    #[arg(long)]
    pub cancel: bool,
}

#[derive(Debug, Args)]
pub struct SelectArgs {
    pub variant_id: String,
}

#[derive(Debug, Args)]
pub struct ApplyArgs {
    pub variant_id: String,
    /// Verify eligibility, permissions and clean matching baseline without writes.
    #[arg(long)]
    pub check: bool,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    pub variant_id: String,
    /// Create-only local patch file outside product/variant checkouts.
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ReportFormat {
    Html,
    Markdown,
}

#[derive(Debug, Args)]
pub struct ReportArgs {
    pub battle_id: String,
    #[arg(long)]
    pub output: PathBuf,
    #[arg(long, value_enum, default_value = "html")]
    pub format: ReportFormat,
    /// Explicitly disclose goal, variant titles, summaries and validation commands.
    /// That text may contain identifying names/paths. Prompts/raw logs stay private.
    #[arg(long)]
    pub include_context: bool,
    /// Public goal text to show instead of the private configured goal.
    #[arg(long)]
    pub public_goal: Option<String>,
    #[arg(long)]
    pub without_attribution: bool,
}

pub fn select(args: SelectArgs) -> Result<()> {
    print_json(&super::selection::select(
        &Store::open()?,
        &args.variant_id,
    )?)
}
pub fn apply(args: ApplyArgs) -> Result<()> {
    let store = Store::open()?;
    if args.check {
        print_json(&super::selection::preview_apply(&store, &args.variant_id)?)
    } else {
        print_json(&super::selection::apply(&store, &args.variant_id)?)
    }
}
pub fn export(args: ExportArgs) -> Result<()> {
    print_json(&super::selection::export(
        &Store::open()?,
        &args.variant_id,
        &args.output,
    )?)
}
pub fn report(args: ReportArgs) -> Result<()> {
    let store = Store::open()?;
    let options = super::report::ReportOptions {
        include_context: args.include_context,
        public_goal: args.public_goal,
        without_attribution: args.without_attribution,
    };
    let path = super::report::export(
        &store,
        &args.battle_id,
        &options,
        &args.output,
        matches!(args.format, ReportFormat::Markdown),
    )?;
    print_json(
        &serde_json::json!({"output":path,"contextDisclosed":options.include_context,
        "outcomeProvenance":"UNTESTED","privacy":"Private metadata, prompts and raw logs are omitted. Explicit context export may contain identifying names/paths."}),
    )
}

fn battle_agent(args: ExecutionArgs) -> Result<Box<dyn super::battle::BattleAgent>> {
    if let Some(path) = args.replay {
        let metadata = std::fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 1024 * 1024
        {
            return Err(anyhow!(
                "Replay input must be a regular JSON file of at most 1 MiB"
            ));
        }
        let plan: super::battle_model::ReplayPlan =
            serde_json::from_slice(&std::fs::read(path)?)
                .map_err(|_| anyhow!("Invalid replay JSON schema"))?;
        if plan.version != 1 || plan.implementations.len() != 3 {
            return Err(anyhow!("Replay schema v1 requires three implementations"));
        }
        return Ok(Box::new(super::battle::ReplayAgent(plan)));
    }
    if let Some(harness_id) = args.harness {
        if args.model.as_ref().is_some_and(|model| {
            model.len() > 256 || model.trim().is_empty() || super::redaction::contains_secret(model)
        }) {
            return Err(anyhow!(
                "Model identifier is invalid or contains a possible credential"
            ));
        }
        if !crate::local::harness::registry()
            .iter()
            .any(|harness| harness.id() == harness_id)
        {
            return Err(anyhow!("Unknown native harness"));
        }
        return Ok(Box::new(super::battle::HarnessAgent {
            harness_id,
            model: args.model,
            timeout_seconds: args.agent_timeout_seconds,
        }));
    }
    Err(anyhow!(
        "Choose --replay <plan.json> or --harness <id>, or use --prepare-only"
    ))
}

pub async fn battle(args: BattleArgs) -> Result<()> {
    let agent = if args.prepare_only {
        None
    } else {
        Some(battle_agent(args.execution)?)
    };
    let store = Store::open()?;
    let battle = super::battle::prepare(&store, &args.project, &args.goal)?;
    let battle = if let Some(agent) = agent {
        super::battle::execute(&store, &battle.id, &*agent).await?
    } else {
        battle
    };
    print_json(&serde_json::json!({"battle":battle,"variants":store.growth_variants(&battle.id)?}))
}

pub async fn battle_run(args: BattleRunArgs) -> Result<()> {
    let agent = battle_agent(args.execution)?;
    let store = Store::open()?;
    print_json(&super::battle::execute(&store, &args.battle_id, &*agent).await?)
}

pub fn compare(args: CompareArgs) -> Result<()> {
    print_json(&super::evaluation::compare(
        &Store::open()?,
        &args.battle_id,
    )?)
}

pub fn experiments(args: ExperimentsArgs) -> Result<()> {
    let store = Store::open()?;
    let battles = store.list_growth_battles(args.project.as_deref())?;
    let mut records = Vec::new();
    for battle in battles {
        records.push(
            serde_json::json!({"variants":store.growth_variants(&battle.id)?,"battle":battle}),
        );
    }
    print_json(&records)
}

pub fn battle_status(args: BattleStatusArgs) -> Result<()> {
    let store = Store::open()?;
    if args.cancel {
        store.request_growth_battle_cancel(&args.battle_id)?;
    }
    print_json(
        &serde_json::json!({"battle":store.get_growth_battle(&args.battle_id)?.ok_or_else(||anyhow!("Battle not found"))?,"runs":store.sealed_growth_runs(&args.battle_id)?,"selections":store.growth_selections(&args.battle_id)?}),
    )
}

pub fn init(args: InitArgs) -> Result<()> {
    let config = GrowthConfig {
        version: 1,
        product: Product {
            name: args.name,
            audience: args.audience,
            description: String::new(),
        },
        goal: Goal { primary: args.goal },
        permissions: Permissions {
            mode: args.mode,
            allowed_paths: args.allowed_paths,
            denied_paths: args.denied_paths,
        },
        validation: Validation {
            commands: args.commands,
            ..Validation::default()
        },
        metrics: Metrics {
            primary: args.metric,
            guardrails: vec!["page_load_time".into()],
        },
        agents: Agents { parallelism: 3 },
    };
    config.write_new(&args.path)?;
    println!(
        "Created growthlab.yaml. Review and commit this configuration before workspace import."
    );
    Ok(())
}

pub fn config(args: ConfigArgs) -> Result<()> {
    match args.command {
        ConfigCommand::Check { path } => {
            GrowthConfig::load(&path)?;
            println!("growthlab.yaml schema v1: valid");
        }
        ConfigCommand::CheckPath { path, relative } => {
            GrowthConfig::load(&path)?.check_write(&path, &relative)?;
            println!("Path is allowed for isolated implementation under this configuration");
        }
    }
    Ok(())
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|_| anyhow!("Git is unavailable; install Git before importing a product"))?;
    if !output.status.success() {
        // stderr can include private URLs/credentials. Give actionable context
        // without copying arbitrary subprocess content into persistence/logs.
        return Err(anyhow!("Git product inspection failed; use a repository with a committed growthlab.yaml and a checked-out branch"));
    }
    String::from_utf8(output.stdout).map_err(|_| anyhow!("Git returned invalid text"))
}

pub fn import(store: &Store, path: &Path) -> Result<GrowthWorkspace> {
    let path = crate::paths::canonicalize(path)?;
    let root = crate::paths::canonicalize(Path::new(
        git(&path, &["rev-parse", "--show-toplevel"])?.trim(),
    ))?;
    if root != path {
        return Err(anyhow!(
            "Import the product repository root, not a nested folder"
        ));
    }
    let config = GrowthConfig::load(&root)?;
    let committed = GrowthConfig::parse(&git(&root, &["show", "HEAD:growthlab.yaml"])?)?;
    if committed != config {
        return Err(anyhow!(
            "Commit the reviewed growthlab.yaml changes before importing"
        ));
    }
    let branch = git(&root, &["symbolic-ref", "--short", "HEAD"])?
        .trim()
        .to_string();
    let commit = git(&root, &["rev-parse", "HEAD"])?.trim().to_string();
    let now = now_ms();
    let id = uuid::Uuid::new_v4().to_string();
    let workspace = GrowthWorkspace {
        project_id: id.clone(),
        config,
        source_snapshot_commit: commit,
        created_at: now,
    };
    let project = LocalProject {
        id,
        name: workspace.config.product.name.clone(),
        slug: format!(
            "{}-{}",
            crate::local::slugify(&workspace.config.product.name),
            &workspace.project_id[..8]
        ),
        github_owner: String::new(),
        github_repo: String::new(),
        github_sync_enabled: false,
        baseline_branch: branch,
        repo_path: root.to_string_lossy().to_string(),
        run_command: None,
        paper_id: None,
        created_at: now,
        updated_at: now,
    };
    store.register_growth_workspace(&project, &workspace)?;
    Ok(workspace)
}

pub fn workspace(args: WorkspaceArgs) -> Result<()> {
    let store = Store::open()?;
    match args.command {
        WorkspaceCommand::Import { path } => print_json(&import(&store, &path)?),
        WorkspaceCommand::List => print_json(&store.list_growth_workspaces()?),
        WorkspaceCommand::View { project_id } => print_json(
            &store
                .get_growth_workspace(&project_id)?
                .ok_or_else(|| anyhow!("Growth workspace not found"))?,
        ),
    }
}

pub fn hypotheses(args: HypothesesArgs) -> Result<()> {
    let store = Store::open()?;
    if !args.list {
        let workspace = store
            .get_growth_workspace(&args.project_id)?
            .ok_or_else(|| anyhow!("Growth workspace not found"))?;
        let hypotheses = starter_hypotheses(&workspace);
        store.insert_growth_hypotheses(&args.project_id, &hypotheses)?;
        print_json(&hypotheses)
    } else {
        print_json(&store.list_growth_hypotheses(&args.project_id)?)
    }
}

fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imports_committed_context_without_touching_product_or_publication() {
        let dir = std::env::temp_dir().join(format!("growthlab-import-{}", uuid::Uuid::new_v4()));
        let product = dir.join("product");
        std::fs::create_dir_all(&product).unwrap();
        let store = Store::open_at(dir.join("store")).unwrap();
        assert!(import(&store, &product).is_err());
        super::super::config::fixture().write_new(&product).unwrap();
        git(&product, &["init", "-b", "main"]).unwrap();
        assert!(
            import(&store, &product).is_err(),
            "uncommitted context is rejected"
        );
        git(&product, &["add", "growthlab.yaml"]).unwrap();
        git(
            &product,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=.disabled-fixture-hooks",
                "commit",
                "-m",
                "fixture context",
            ],
        )
        .unwrap();
        let original_head = git(&product, &["rev-parse", "HEAD"]).unwrap();
        let workspace = import(&store, &product).unwrap();
        assert_eq!(workspace.source_snapshot_commit, original_head.trim());
        assert_eq!(git(&product, &["status", "--porcelain"]).unwrap(), "");
        assert_eq!(
            git(&product, &["rev-parse", "HEAD"]).unwrap(),
            original_head
        );
        assert!(!store
            .get_local_project(&workspace.project_id)
            .unwrap()
            .unwrap()
            .github_enabled());
        assert!(
            import(&store, &product).is_err(),
            "duplicate import is rejected"
        );
        let mut config = super::super::config::fixture();
        config.goal.primary = "Changed context".into();
        std::fs::write(
            product.join("growthlab.yaml"),
            serde_yaml_ng::to_string(&config).unwrap(),
        )
        .unwrap();
        assert!(
            import(&store, &product).is_err(),
            "dirty configuration cannot silently replace snapshot context"
        );
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
