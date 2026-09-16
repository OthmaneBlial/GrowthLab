use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{Args, Subcommand, ValueEnum};

use crate::error::{anyhow, Result};
use crate::local::model::LocalProject;
use crate::store::{now_ms, Store};

use super::config::{
    Agents, Goal, GrowthConfig, Metrics, PermissionMode, Permissions, Product, Validation,
    CONFIG_FILE,
};
use super::model::{starter_hypotheses, GrowthWorkspace, Provenance};

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
    /// Archive a restricted static page from this allowed, committed directory.
    #[arg(long)]
    pub preview_root: Option<String>,
    #[arg(long, requires = "preview_root")]
    pub preview_entry: Option<String>,
}

#[derive(Debug, Args)]
pub struct DemoArgs {
    /// Bind a fresh loopback port by default; existing dashboards stay available.
    #[arg(long, default_value_t = 0)]
    pub port: u16,
    #[arg(long)]
    pub no_browser: bool,
}

pub async fn demo(args: DemoArgs) -> Result<()> {
    crate::commands::up::demo(crate::UpArgs {
        port: args.port,
        no_browser: args.no_browser,
        no_agent: true,
        remote: None,
        model: None,
        remote_host: false,
    })
    .await
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
    /// Register a local product and its committed growthlab.yaml.
    Import {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Initialize a new local Git repository and commit the current folder
        /// when the path is not already a repository. No remote is configured.
        #[arg(long)]
        init_git: bool,
    },
    /// Create a local, analysis-only workspace from a manually entered brief.
    /// The generated repository stays inside GrowthLab's data directory and
    /// has no remote; it is a structured starting point, not a product clone.
    Brief {
        #[arg(long)]
        name: String,
        #[arg(long)]
        audience: String,
        #[arg(long)]
        goal: String,
        #[arg(long, default_value = "")]
        description: String,
        #[arg(long, default_value = "qualified_signup")]
        metric: String,
        #[arg(long, value_enum, default_value = "analyze-only")]
        mode: PermissionMode,
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

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SeoAuditFormat {
    Json,
    Markdown,
}

#[derive(Debug, Args)]
pub struct SeoAuditArgs {
    /// Read a regular local HTML file; no network request is made.
    #[arg(long, conflicts_with = "url", required_unless_present = "url")]
    pub html: Option<PathBuf>,
    /// Fetch one public HTTPS page after checking its same-origin robots.txt.
    #[arg(long, conflicts_with = "html", required_unless_present = "html")]
    pub url: Option<String>,
    /// Choose machine-readable JSON or a compact Markdown review.
    #[arg(long, value_enum, default_value = "json")]
    pub format: SeoAuditFormat,
}

#[derive(Debug, Args)]
pub struct RepoAuditArgs {
    /// Inspect one public GitHub repository metadata page without cloning it.
    #[arg(long)]
    pub url: String,
    /// Choose machine-readable JSON or a compact Markdown review.
    #[arg(long, value_enum, default_value = "json")]
    pub format: SeoAuditFormat,
}

#[derive(Debug, Args)]
pub struct MeasureArgs {
    /// Read a regular local CSV; no analytics or network request is made.
    #[arg(long)]
    pub csv: PathBuf,
    /// Variant value used as the arithmetic comparison baseline.
    #[arg(long, default_value = "baseline")]
    pub baseline: String,
    /// Include only rows whose metric column matches this value.
    #[arg(long)]
    pub metric: Option<String>,
    /// CSV column containing the variant name.
    #[arg(long, default_value = "variant")]
    pub variant_column: String,
    /// CSV column containing one numeric observation per row.
    #[arg(long, default_value = "value")]
    pub value_column: String,
    /// Optional CSV column grouping observations into metrics.
    #[arg(long, default_value = "metric")]
    pub metric_column: String,
    /// Optional CSV column used to report the observed date range.
    #[arg(long, default_value = "timestamp")]
    pub timestamp_column: String,
    /// Choose machine-readable JSON or a compact Markdown review.
    #[arg(long, value_enum, default_value = "json")]
    pub format: super::measurement::MeasurementFormat,
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
pub struct RecoverArgs {
    pub battle_id: String,
}

pub fn recover(args: RecoverArgs) -> Result<()> {
    print_json(&super::recovery::recover(&Store::open()?, &args.battle_id)?)
}

#[derive(Debug, Args)]
pub struct RecoverDeliveryArgs {
    pub receipt_id: String,
    /// Apply only exact baseline files that remain; never roll back conflicts.
    #[arg(long)]
    pub resume: bool,
}

pub fn recover_delivery(args: RecoverDeliveryArgs) -> Result<()> {
    print_json(&super::selection::recover_delivery(
        &Store::open()?,
        &args.receipt_id,
        args.resume,
    )?)
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

fn read_audit_html(path: &Path) -> Result<String> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| anyhow!("SEO audit input was not found"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(anyhow!(
            "SEO audit input must be a regular local HTML file, not a directory or symlink"
        ));
    }
    const MAX_HTML_BYTES: u64 = 4 * 1024 * 1024;
    if metadata.len() > MAX_HTML_BYTES {
        return Err(anyhow!("SEO audit input must be at most 4 MiB"));
    }
    let bytes = std::fs::read(path).map_err(|_| anyhow!("SEO audit input could not be read"))?;
    String::from_utf8(bytes).map_err(|_| anyhow!("SEO audit input must be UTF-8 HTML"))
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn provenance_label(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::Measured => "MEASURED",
        Provenance::Observed => "OBSERVED",
        Provenance::Estimated => "ESTIMATED",
        Provenance::Simulated => "SIMULATED",
        Provenance::Untested => "UNTESTED",
    }
}

fn audit_markdown(path: &Path, rubric: &super::evaluation::SeoRubric) -> String {
    let mut output = format!(
        "# Local SEO page audit\n\n- File: `{}`\n- Score: **{} / {}**\n- Provenance: **{}**\n- Rubric: **{}** (`{}`)\n\n",
        path.display(),
        rubric.score,
        rubric.max_score,
        provenance_label(rubric.provenance),
        rubric.label,
        rubric.id
    );
    output.push_str(
        "## Dimensions\n\n| Dimension | Score | Status | Evidence |\n| --- | ---: | --- | --- |\n",
    );
    for dimension in &rubric.dimensions {
        output.push_str(&format!(
            "| {} | {}/{} | {} | {} |\n",
            markdown_cell(&dimension.label),
            dimension.score,
            dimension.max_score,
            markdown_cell(&dimension.status),
            markdown_cell(&dimension.evidence.join(" "))
        ));
    }
    output.push_str(&format!("\n**Calculation:** {}\n\n", rubric.calculation));
    output.push_str("## Suggested next steps\n\n");
    if rubric.recommendations.is_empty() {
        output.push_str("- No structural gaps were found by this local rubric.\n\n");
    } else {
        for recommendation in &rubric.recommendations {
            output.push_str(&format!("- {recommendation}\n"));
        }
        output.push('\n');
    }
    output.push_str("## Limits\n\n");
    for limitation in &rubric.limitations {
        output.push_str(&format!("- {limitation}\n"));
    }
    output.push_str("\nThis is a local structural review of the supplied file. It does not fetch the web and does not claim rankings, traffic, or conversions.\n");
    output
}

pub async fn seo_audit(args: SeoAuditArgs) -> Result<()> {
    if let Some(url) = args.url {
        let audit = super::web_audit::fetch(&url).await?;
        return match args.format {
            SeoAuditFormat::Json => print_json(&audit),
            SeoAuditFormat::Markdown => {
                print!("{}", super::web_audit::markdown(&audit));
                Ok(())
            }
        };
    }
    let html = read_audit_html(args.html.as_deref().expect("clap requires --html or --url"))?;
    let rubric = super::evaluation::seo_rubric(&html);
    match args.format {
        SeoAuditFormat::Json => print_json(&serde_json::json!({
            "path": args.html,
            "scope": "Local UTF-8 HTML file; no network request",
            "rubric": rubric,
        })),
        SeoAuditFormat::Markdown => {
            print!(
                "{}",
                audit_markdown(args.html.as_deref().expect("html was validated"), &rubric)
            );
            Ok(())
        }
    }
}

pub async fn repo_audit(args: RepoAuditArgs) -> Result<()> {
    let audit = super::github_audit::fetch(&args.url).await?;
    match args.format {
        SeoAuditFormat::Json => print_json(&audit),
        SeoAuditFormat::Markdown => {
            print!("{}", super::github_audit::markdown(&audit));
            Ok(())
        }
    }
}

pub fn measure(args: MeasureArgs) -> Result<()> {
    let report = super::measurement::import(
        &args.csv,
        &args.baseline,
        args.metric.as_deref(),
        &args.variant_column,
        &args.value_column,
        &args.metric_column,
        &args.timestamp_column,
    )?;
    match args.format {
        super::measurement::MeasurementFormat::Json => print_json(&report),
        super::measurement::MeasurementFormat::Markdown => {
            print!("{}", super::measurement::markdown(&report));
            Ok(())
        }
    }
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
        &serde_json::json!({"battle":store.get_growth_battle(&args.battle_id)?.ok_or_else(||anyhow!("Battle not found"))?,"runs":store.sealed_growth_runs(&args.battle_id)?,"attempts":store.growth_attempts(&args.battle_id)?,"selections":store.growth_selections(&args.battle_id)?}),
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
        static_preview: args.preview_root.map(|root| super::config::StaticPreview {
            root,
            entry: args.preview_entry.unwrap_or_else(|| "index.html".into()),
        }),
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
    import_with_options(store, path, false)
}

/// Import a committed local product. With `initialize_git`, an otherwise
/// unversioned folder receives a local-only initial snapshot after protected
/// paths are checked. Existing repositories are never auto-committed.
pub fn import_with_options(
    store: &Store,
    path: &Path,
    initialize_git: bool,
) -> Result<GrowthWorkspace> {
    let path = crate::paths::canonicalize(path)?;
    if !path.is_dir() {
        return Err(anyhow!("Product path must be a directory"));
    }
    // Validate the reviewed context before any optional Git initialization.
    // This avoids turning an invalid or secret-bearing config into a commit.
    let config = GrowthConfig::load(&path).map_err(|error| {
        if initialize_git && !crate::local::git::own_repository_state(&path).is_initialized() {
            anyhow!("A valid growthlab.yaml is required before --init-git: {error}")
        } else {
            error
        }
    })?;
    let state = crate::local::git::own_repository_state(&path);
    if matches!(state, crate::local::git::RepositoryState::NotRepository) && initialize_git {
        initialize_local_repository(&path)?;
    } else if matches!(state, crate::local::git::RepositoryState::Unborn) && initialize_git {
        commit_initial_snapshot(&path)?;
    }
    let root = crate::local::git::repository_root(&path).map_err(|_| {
        anyhow!(
            "Product path is not a Git repository. Review growthlab.yaml, then rerun with --init-git to create a local snapshot."
        )
    })?;
    if root != path {
        return Err(anyhow!(
            "Import the product repository root, not a nested folder"
        ));
    }
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

fn initialize_local_repository(root: &Path) -> Result<()> {
    match crate::local::git::own_repository_state(root) {
        crate::local::git::RepositoryState::Invalid => {
            return Err(anyhow!(
                "The product has an invalid .git entry; repair it manually before --init-git"
            ));
        }
        crate::local::git::RepositoryState::NotRepository => {}
        crate::local::git::RepositoryState::Unborn => return commit_initial_snapshot(root),
        crate::local::git::RepositoryState::Ready
        | crate::local::git::RepositoryState::Detached => return Ok(()),
    }
    crate::local::git::git(Some(root), &["init", "-b", "main"])
        .or_else(|_| crate::local::git::git(Some(root), &["init"]))?;
    commit_initial_snapshot(root)
}

fn commit_initial_snapshot(root: &Path) -> Result<()> {
    crate::local::git::git(Some(root), &["add", "--all"])?;
    let staged = crate::local::git::git(Some(root), &["diff", "--cached", "--name-only", "-z"])?;
    let files: Vec<_> = staged.split('\0').filter(|path| !path.is_empty()).collect();
    if !files.contains(&CONFIG_FILE) {
        return Err(anyhow!(
            "The initial snapshot must include a regular growthlab.yaml"
        ));
    }
    if let Some(path) = files
        .iter()
        .find(|path| **path != CONFIG_FILE && super::config::protected_path(path))
    {
        return Err(anyhow!(
            "The initial snapshot includes a protected path ({path}); remove it or add it to .gitignore before --init-git"
        ));
    }
    let has_identity = crate::local::git::git(Some(root), &["config", "user.name"]).is_ok()
        && crate::local::git::git(Some(root), &["config", "user.email"]).is_ok();
    if has_identity {
        crate::local::git::git(
            Some(root),
            &[
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=.growthlab-no-hooks",
                "commit",
                "-m",
                "Initialize GrowthLab product snapshot",
            ],
        )?;
    } else {
        crate::local::git::git(
            Some(root),
            &[
                "-c",
                "user.name=GrowthLab local initializer",
                "-c",
                "user.email=growthlab@localhost",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=.growthlab-no-hooks",
                "commit",
                "-m",
                "Initialize GrowthLab product snapshot",
            ],
        )?;
    }
    Ok(())
}

pub fn workspace(args: WorkspaceArgs) -> Result<()> {
    let store = Store::open()?;
    match args.command {
        WorkspaceCommand::Import { path, init_git } => {
            print_json(&import_with_options(&store, &path, init_git)?)
        }
        WorkspaceCommand::Brief {
            name,
            audience,
            goal,
            description,
            metric,
            mode,
        } => print_json(&create_brief(
            &store,
            &name,
            &audience,
            &goal,
            &description,
            &metric,
            mode,
        )?),
        WorkspaceCommand::List => print_json(&store.list_growth_workspaces()?),
        WorkspaceCommand::View { project_id } => print_json(
            &store
                .get_growth_workspace(&project_id)?
                .ok_or_else(|| anyhow!("Growth workspace not found"))?,
        ),
    }
}

/// Create a self-contained workspace from user-entered product context. The
/// brief is deliberately local and analysis-only by default: no provider,
/// network request, remote, deployment or product checkout is touched.
pub fn create_brief(
    store: &Store,
    name: &str,
    audience: &str,
    goal: &str,
    description: &str,
    metric: &str,
    mode: PermissionMode,
) -> Result<GrowthWorkspace> {
    for (label, value, limit) in [
        ("name", name, 512usize),
        ("audience", audience, 2048usize),
        ("goal", goal, 4096usize),
        ("description", description, 8192usize),
        ("metric", metric, 256usize),
    ] {
        if value.trim().is_empty() && label != "description" {
            return Err(anyhow!("Manual brief {label} must not be empty"));
        }
        if value.len() > limit {
            return Err(anyhow!(
                "Manual brief {label} exceeds the {limit}-byte limit"
            ));
        }
        if super::redaction::contains_secret(value) {
            return Err(anyhow!(
                "Manual brief {label} contains a possible credential"
            ));
        }
    }
    // Implementation mode is allowed only when a concrete path and command
    // contract exist. A manual brief has neither, so downgrade that request
    // with an explicit error instead of creating an unusable battle workspace.
    if mode == PermissionMode::Implement {
        return Err(anyhow!(
            "Manual briefs start in analyze-only or draft mode; import a configured repository for implementation battles"
        ));
    }
    let config = GrowthConfig {
        version: 1,
        product: Product {
            name: name.trim().into(),
            audience: audience.trim().into(),
            description: description.trim().into(),
        },
        goal: Goal {
            primary: goal.trim().into(),
        },
        permissions: Permissions {
            mode,
            allowed_paths: Vec::new(),
            denied_paths: Vec::new(),
        },
        validation: Validation::default(),
        metrics: Metrics {
            primary: metric.trim().into(),
            guardrails: vec!["page_load_time".into()],
        },
        agents: Agents { parallelism: 3 },
        static_preview: None,
    };
    config.validate()?;

    let id = uuid::Uuid::new_v4().to_string();
    let root = store.data_root().join("growth-briefs").join(&id);
    std::fs::create_dir_all(&root)?;
    let result = (|| -> Result<GrowthWorkspace> {
        config.write_new(&root)?;
        let brief = format!(
            "# {}\n\nAudience: {}\n\nGoal: {}\n\n{}\n",
            config.product.name,
            config.product.audience,
            config.goal.primary,
            config.product.description
        );
        std::fs::write(root.join("BRIEF.md"), brief)?;
        crate::local::git::git(Some(&root), &["init", "-b", "main"])
            .or_else(|_| crate::local::git::git(Some(&root), &["init"]))?;
        crate::local::git::git(Some(&root), &["add", "growthlab.yaml", "BRIEF.md"])?;
        crate::local::git::git(
            Some(&root),
            &[
                "-c",
                "user.name=GrowthLab brief",
                "-c",
                "user.email=brief@growthlab.local",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=.growthlab-no-hooks",
                "commit",
                "-m",
                "Create local product brief",
            ],
        )?;
        let commit = git(&root, &["rev-parse", "HEAD"])?.trim().to_string();
        let now = now_ms();
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
            baseline_branch: git(&root, &["symbolic-ref", "--short", "HEAD"])?
                .trim()
                .to_string(),
            repo_path: root.to_string_lossy().to_string(),
            run_command: None,
            paper_id: None,
            created_at: now,
            updated_at: now,
        };
        store.register_growth_workspace(&project, &workspace)?;
        Ok(workspace)
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&root);
    }
    result
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
    fn seo_audit_reads_local_html_and_formats_markdown() {
        let dir =
            std::env::temp_dir().join(format!("growthlab-seo-audit-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("index.html");
        std::fs::write(&path, "<html lang=\"en\"><head><title>Local SEO page</title></head><body><h1>Find the right page</h1><p>Useful product copy for a local audit.</p></body></html>").unwrap();
        let html = read_audit_html(&path).unwrap();
        let rubric = super::super::evaluation::seo_rubric(&html);
        let markdown = audit_markdown(&path, &rubric);
        assert!(markdown.contains("# Local SEO page audit"));
        assert!(markdown.contains("Provenance: **ESTIMATED**"));
        assert!(markdown.contains("SEO page hygiene"));
        assert!(markdown.contains("does not claim rankings"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn seo_audit_rejects_symlink_inputs() {
        let dir = std::env::temp_dir().join(format!("growthlab-seo-link-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("index.html");
        let link = dir.join("link.html");
        std::fs::write(&target, "<html></html>").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(read_audit_html(&link).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn measure_reads_local_csv_and_reports_descriptive_comparison() {
        let dir =
            std::env::temp_dir().join(format!("growthlab-measure-cli-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("metrics.csv");
        std::fs::write(
            &path,
            "timestamp,variant,metric,value\n2026-09-01,baseline,signup,10\n2026-09-02,hero,signup,15\n",
        )
        .unwrap();
        let report = super::super::measurement::import(
            &path,
            "baseline",
            None,
            "variant",
            "value",
            "metric",
            "timestamp",
        )
        .unwrap();
        let markdown = super::super::measurement::markdown(&report);
        assert!(markdown.contains("# Local growth measurement"));
        assert!(markdown.contains("Provenance: **MEASURED**"));
        assert!(markdown.contains("Relative change"));
        assert!(markdown.contains("does not establish causality"));
        std::fs::remove_dir_all(dir).unwrap();
    }
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

    #[test]
    fn initializes_a_non_git_folder_only_with_explicit_option() {
        let dir =
            std::env::temp_dir().join(format!("growthlab-init-import-{}", uuid::Uuid::new_v4()));
        let product = dir.join("product");
        std::fs::create_dir_all(product.join("website")).unwrap();
        super::super::config::fixture().write_new(&product).unwrap();
        std::fs::write(product.join("website/index.html"), "<h1>Local product</h1>").unwrap();
        let store = Store::open_at(dir.join("store")).unwrap();

        assert!(import_with_options(&store, &product, false).is_err());
        let workspace = import_with_options(&store, &product, true).unwrap();
        assert_eq!(workspace.source_snapshot_commit.len(), 40);
        assert_eq!(git(&product, &["status", "--porcelain"]).unwrap(), "");
        assert_eq!(
            git(&product, &["show", "--format=%s", "--no-patch"])
                .unwrap()
                .trim(),
            "Initialize GrowthLab product snapshot"
        );
        assert_eq!(
            git(
                &product,
                &["ls-files", "--error-unmatch", "website/index.html"]
            )
            .unwrap()
            .trim(),
            "website/index.html"
        );
        assert!(git(&product, &["remote"]).unwrap().is_empty());
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refuses_protected_paths_before_the_non_git_initial_commit() {
        let dir =
            std::env::temp_dir().join(format!("growthlab-init-secret-{}", uuid::Uuid::new_v4()));
        let product = dir.join("product");
        std::fs::create_dir_all(product.join("website")).unwrap();
        super::super::config::fixture().write_new(&product).unwrap();
        std::fs::write(product.join("website/index.html"), "<h1>Local product</h1>").unwrap();
        std::fs::write(product.join(".env"), "DEMO=value").unwrap();
        let store = Store::open_at(dir.join("store")).unwrap();
        let error = import_with_options(&store, &product, true)
            .unwrap_err()
            .to_string();
        assert!(error.contains("protected path"));
        assert!(git(&product, &["rev-parse", "--verify", "HEAD"]).is_err());
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn creates_a_local_manual_brief_without_remote_or_implementation_permission() {
        let dir = std::env::temp_dir().join(format!("growthlab-brief-{}", uuid::Uuid::new_v4()));
        let store = Store::open_at(dir.join("store")).unwrap();
        let workspace = create_brief(
            &store,
            "Brief product",
            "Indie makers",
            "Increase qualified signups",
            "A local-first product brief.",
            "qualified_signup",
            PermissionMode::AnalyzeOnly,
        )
        .unwrap();
        assert_eq!(
            workspace.config.permissions.mode,
            PermissionMode::AnalyzeOnly
        );
        assert_eq!(workspace.source_snapshot_commit.len(), 40);
        let project = store
            .get_local_project(&workspace.project_id)
            .unwrap()
            .unwrap();
        let root = PathBuf::from(project.repo_path);
        assert!(root.join("BRIEF.md").is_file());
        assert_eq!(git(&root, &["remote"]).unwrap(), "");
        assert_eq!(git(&root, &["status", "--porcelain"]).unwrap(), "");
        assert!(create_brief(
            &store,
            "Product",
            "Audience",
            "Goal",
            "Description",
            "metric",
            PermissionMode::Implement,
        )
        .is_err());
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn manual_brief_rejects_credential_like_text() {
        let dir =
            std::env::temp_dir().join(format!("growthlab-brief-secret-{}", uuid::Uuid::new_v4()));
        let store = Store::open_at(dir.join("store")).unwrap();
        let result = create_brief(
            &store,
            "Product",
            "Audience",
            "Goal",
            "API_KEY=should-not-be-stored",
            "metric",
            PermissionMode::AnalyzeOnly,
        );
        assert!(result.is_err());
        drop(store);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
