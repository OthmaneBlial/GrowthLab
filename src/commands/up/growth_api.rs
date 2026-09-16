//! GrowthLab domain API. All execution uses the CLI's battle engine and archives.
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::FromRef;
use axum::http::HeaderValue;

use super::*;
use crate::growth::{
    archive, battle, battle_model::*, cli, evaluation, model, recovery, report, selection,
};

#[derive(Clone)]
struct GrowthState {
    host: Arc<GrowthHost>,
    lifecycle: Arc<ProjectLifecycle>,
    gate: Arc<tokio::sync::Mutex<()>>,
    moving: Arc<AtomicBool>,
    stopping: Arc<AtomicBool>,
    /// Tests use explicit roots, never a user's database or process-global env.
    root: Option<PathBuf>,
}

impl FromRef<AppState> for GrowthState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            host: state.growth.clone(),
            lifecycle: state.project_lifecycle.clone(),
            gate: state.data_dir_gate.clone(),
            moving: state.data_dir_move_in_progress.clone(),
            stopping: state.stopping.clone(),
            root: None,
        }
    }
}

impl GrowthState {
    fn ready(&self) -> std::result::Result<(), ApiError> {
        if self.stopping.load(Ordering::SeqCst) {
            return Err(ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "GrowthLab is stopping.".into(),
            ));
        }
        if self.moving.load(Ordering::SeqCst) {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "GrowthLab storage is moving. Retry after it finishes.".into(),
            ));
        }
        Ok(())
    }

    #[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
    fn data_root(&self) -> PathBuf {
        self.root.clone().unwrap_or_else(crate::store::data_dir)
    }
}

#[derive(Default)]
pub(super) struct GrowthHost {
    workers: Mutex<BTreeMap<String, Worker>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Worker {
    running: bool,
    error: Option<String>,
    #[serde(skip)]
    root: PathBuf,
}

impl GrowthHost {
    fn status(&self, id: &str) -> Option<Worker> {
        self.workers.lock().unwrap().get(id).cloned()
    }

    fn launch(
        self: &Arc<Self>,
        store: Store,
        storage: crate::store::DataDirUseLock,
        battle_id: String,
        request: ExecutionRequest,
        admission: ProjectAdmissionLease,
    ) -> std::result::Result<(), ApiError> {
        let root = store.data_root().to_path_buf();
        {
            let mut workers = self.workers.lock().unwrap();
            if workers.get(&battle_id).is_some_and(|worker| worker.running) {
                return Err(ApiError(
                    StatusCode::CONFLICT,
                    "A controller already owns this battle.".into(),
                ));
            }
            if workers.values().filter(|worker| worker.running).count() >= 2 {
                return Err(ApiError(
                    StatusCode::CONFLICT,
                    "Two battles are already running. Finish or cancel one first.".into(),
                ));
            }
            if workers.len() >= 128 {
                workers.retain(|_, worker| worker.running);
            }
            workers.insert(
                battle_id.clone(),
                Worker {
                    running: true,
                    error: None,
                    root: root.clone(),
                },
            );
        }
        let host = self.clone();
        let id = battle_id.clone();
        // Native harness futures are intentionally !Send. Move the pinned Store
        // into an owned thread and construct the agent on its local runtime.
        // The project admission stays alive through execution and blocks moves
        // and deletion without holding the gate needed by live readers.
        let launched = std::thread::Builder::new().name("growth-battle".into()).spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
                let _storage = storage;
                let _admission = admission;
                let store = store;
                let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
                let agent = request.agent();
                runtime.block_on(battle::execute(&store, &id, &*agent))?;
                Ok(())
            }));
            let error = match result {
                Ok(Ok(())) => None,
                Ok(Err(error)) => Some(crate::growth::redaction::redact(&error.to_string())),
                Err(_) => Some("Battle controller stopped unexpectedly. Inspect and recover its captured attempts.".into()),
            };
            if let Some(worker) = host.workers.lock().unwrap().get_mut(&id) {
                worker.running = false;
                worker.error = error;
            }
        });
        if launched.is_err() {
            self.workers.lock().unwrap().remove(&battle_id);
            return Err(ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "Could not start the battle controller; no execution was submitted.".into(),
            ));
        }
        Ok(())
    }

    pub(super) async fn shutdown(&self) {
        let active: Vec<_> = self
            .workers
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, worker)| worker.running)
            .map(|(id, worker)| (id.clone(), worker.root.clone()))
            .collect();
        let _ = tokio::task::spawn_blocking(move || {
            for (id, root) in active {
                if let Ok(store) = Store::open_at(root) {
                    let _ = store.request_growth_battle_cancel(&id);
                }
            }
        })
        .await;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        while self
            .workers
            .lock()
            .unwrap()
            .values()
            .any(|worker| worker.running)
        {
            if tokio::time::Instant::now() >= deadline {
                eprintln!("GrowthLab: unfinished battle controllers retain checkpoints; inspect explicit recovery after restart.");
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}

pub(super) fn routes() -> Router<AppState> {
    build_routes()
}

fn build_routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    GrowthState: FromRef<S>,
{
    Router::new()
        .route("/api/growth/demo", post(create_demo))
        .route("/api/growth/capabilities", get(capabilities))
        .route("/api/growth/measurement/sources", get(measurement_sources))
        .route("/api/growth/playbooks", get(playbooks))
        .route("/api/growth/url-audit", post(url_audit))
        .route("/api/growth/repository-audit", post(repository_audit))
        .route("/api/growth/repository-import", post(repository_import))
        .route("/api/growth/briefs", post(create_brief_workspace))
        .route(
            "/api/growth/workspaces",
            get(workspaces).post(import_workspace),
        )
        .route("/api/growth/workspaces/{id}", get(workspace))
        .route(
            "/api/growth/workspaces/{id}/hypotheses",
            get(hypotheses).post(create_hypotheses),
        )
        .route(
            "/api/growth/workspaces/{id}/hypotheses/{hypothesis_id}",
            patch(update_hypothesis),
        )
        .route("/api/growth/workspaces/{id}/playbooks", get(playbook_runs))
        .route(
            "/api/growth/workspaces/{id}/playbooks/{role}",
            post(run_playbook),
        )
        .route("/api/growth/battles", get(battles).post(prepare_battle))
        .route("/api/growth/battles/{id}", get(battle_status))
        .route("/api/growth/battles/{id}/run", post(run_battle))
        .route("/api/growth/battles/{id}/cancel", post(cancel_battle))
        .route("/api/growth/battles/{id}/recover", post(recover_battle))
        .route("/api/growth/battles/{id}/compare", get(compare_battle))
        .route("/api/growth/battles/{id}/report", get(battle_report))
        .route("/api/growth/variants/{id}/artifacts", get(artifacts))
        .route("/api/growth/variants/{id}/artifact", get(artifact))
        .route(
            "/api/growth/variants/{id}/static-preview",
            get(static_preview),
        )
        .route(
            "/api/growth/variants/{id}/static-preview/{viewport}",
            get(static_preview_screenshot),
        )
        .route("/api/growth/variants/{id}/select", post(select_variant))
        .route(
            "/api/growth/variants/{id}/apply-preview",
            get(apply_preview),
        )
        .route("/api/growth/variants/{id}/apply", post(apply_variant))
        .route("/api/growth/variants/{id}/export", post(export_variant))
        .route("/api/growth/delivery/{id}/recover", post(recover_delivery))
        .route("/api/growth/events", get(growth_events))
        .layer(DefaultBodyLimit::max(1024 * 1024))
}

async fn with_store<T, F>(
    state: GrowthState,
    project_id: Option<String>,
    action: F,
) -> std::result::Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&Store) -> std::result::Result<T, ApiError> + Send + 'static,
{
    with_owned_store(state, project_id, move |store, storage| {
        let _storage = storage;
        action(&store)
    })
    .await
}

async fn with_owned_store<T, F>(
    state: GrowthState,
    project_id: Option<String>,
    action: F,
) -> std::result::Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(Store, crate::store::DataDirUseLock) -> std::result::Result<T, ApiError>
        + Send
        + 'static,
{
    state.ready()?;
    let gate = state.gate.clone().lock_owned().await;
    state.ready()?;
    let admission = state
        .lifecycle
        .admit(project_id.as_deref().unwrap_or("__project_create__"))
        .ok_or_else(|| {
            ApiError(
                StatusCode::CONFLICT,
                "This workspace is being deleted.".into(),
            )
        })?;
    tokio::task::spawn_blocking(move || {
        // A disconnected HTTP client cannot release these while writes continue.
        let (_gate, _admission) = (gate, admission);
        let store = (if let Some(root) = state.root {
            Store::open_at(root)
        } else {
            Store::open()
        })
        .map_err(|_| {
            ApiError(
                StatusCode::INTERNAL_SERVER_ERROR,
                "GrowthLab storage is unavailable.".into(),
            )
        })?;
        let storage = store.acquire_data_dir_use_lock().map_err(|_| {
            ApiError(
                StatusCode::CONFLICT,
                "Another dashboard is moving GrowthLab storage. Retry after it finishes.".into(),
            )
        })?;
        action(store, storage)
    })
    .await
    .map_err(|_| {
        ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "GrowthLab operation stopped unexpectedly.".into(),
        )
    })?
}

fn value(value: impl Serialize) -> ApiResult {
    Ok(Json(serde_json::to_value(value).map_err(|_| {
        ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "GrowthLab response serialization failed.".into(),
        )
    })?))
}

fn domain_error(error: crate::error::Error) -> ApiError {
    bad_request(crate::growth::redaction::redact(&error.to_string()))
}

fn get_battle(store: &Store, id: &str) -> std::result::Result<GrowthBattle, ApiError> {
    store
        .get_growth_battle(id)?
        .ok_or_else(|| not_found("Growth Battle"))
}

async fn workspaces(State(state): State<GrowthState>) -> ApiResult {
    with_store(state, None, |store| value(store.list_growth_workspaces()?)).await
}

async fn capabilities() -> ApiResult {
    value(
        json!({"isolationAvailable":crate::growth::confinement::available().is_ok(),"platform":std::env::consts::OS,"harnesses":local::harness::registry().iter().map(|harness|json!({"id":harness.id(),"toolsDisabledProposals":harness.one_shot_has_no_tools()})).collect::<Vec<_>>(),"limitation":"Adapter capabilities do not verify installation, authentication or provider execution. Linux requires native namespace verification; Windows validation is unsupported. Resource quotas are not provided."}),
    )
}

/// Describe local and future measurement adapters without contacting them.
async fn measurement_sources() -> ApiResult {
    value(crate::growth::measurement::sources())
}

/// Return reusable role contracts without contacting an agent or analytics provider.
async fn playbooks() -> ApiResult {
    value(crate::growth::playbooks::catalog())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UrlAuditRequest {
    url: String,
}

/// Fetch one public website page through the conservative read-only auditor.
async fn url_audit(Json(request): Json<UrlAuditRequest>) -> ApiResult {
    if request.url.len() > 2048 {
        return Err(bad_request("Website URL is too long"));
    }
    value(
        crate::growth::web_audit::fetch(&request.url)
            .await
            .map_err(domain_error)?,
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RepositoryAuditRequest {
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RepositoryImportRequest {
    url: String,
    path: PathBuf,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    audience: Option<String>,
    #[serde(default)]
    goal: Option<String>,
    #[serde(default)]
    description: String,
    #[serde(default = "default_brief_metric")]
    metric: String,
    #[serde(default = "default_brief_mode")]
    mode: crate::growth::config::PermissionMode,
    #[serde(default)]
    allowed_paths: Vec<String>,
    #[serde(default)]
    denied_paths: Vec<String>,
    #[serde(default)]
    commands: Vec<String>,
    #[serde(default)]
    shallow: bool,
}

/// Inspect public GitHub metadata only. Source files are never cloned or run.
async fn repository_audit(Json(request): Json<RepositoryAuditRequest>) -> ApiResult {
    if request.url.len() > 2048 {
        return Err(bad_request("Public repository URL is too long"));
    }
    value(
        crate::growth::github_audit::fetch(&request.url)
            .await
            .map_err(domain_error)?,
    )
}

/// Clone and register a public repository only when the caller explicitly
/// supplies a destination. Existing local configuration is preserved; a
/// missing one can be created from the supplied brief fields.
async fn repository_import(
    State(state): State<GrowthState>,
    Json(request): Json<RepositoryImportRequest>,
) -> ApiResult {
    if request.url.len() > 2048 || request.path.to_string_lossy().len() > 4096 {
        return Err(bad_request("Public repository import input is too long"));
    }
    with_store(state, None, move |store| {
        value(
            cli::import_public(
                store,
                &request.url,
                &request.path,
                request.name.as_deref(),
                request.audience.as_deref(),
                request.goal.as_deref(),
                &request.description,
                &request.metric,
                request.mode,
                &request.allowed_paths,
                &request.denied_paths,
                &request.commands,
                request.shallow,
            )
            .map_err(domain_error)?,
        )
    })
    .await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DemoRequest {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DemoLaunch {
    project_id: String,
    battle_id: String,
    accepted: bool,
}

pub(super) async fn start_demo(state: &AppState) -> std::result::Result<String, ApiError> {
    Ok(prepare_and_launch_demo(GrowthState::from_ref(state))
        .await?
        .battle_id)
}

async fn create_demo(
    State(state): State<GrowthState>,
    Json(_): Json<DemoRequest>,
) -> std::result::Result<Response, ApiError> {
    let launched = prepare_and_launch_demo(state).await?;
    Ok((StatusCode::ACCEPTED, Json(launched)).into_response())
}

async fn prepare_and_launch_demo(state: GrowthState) -> std::result::Result<DemoLaunch, ApiError> {
    let host = state.host.clone();
    let lifecycle = state.lifecycle.clone();
    with_owned_store(state, None, move |store, storage| {
        if host
            .workers
            .lock()
            .unwrap()
            .values()
            .filter(|worker| worker.running)
            .count()
            >= 2
        {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "Two battles are already running. Finish or cancel one first.".into(),
            ));
        }
        let battle = crate::growth::demo::prepare(&store).map_err(domain_error)?;
        let admission = lifecycle.admit(&battle.project_id).ok_or_else(|| {
            ApiError(
                StatusCode::CONFLICT,
                "Demo workspace is being deleted.".into(),
            )
        })?;
        let response = DemoLaunch {
            project_id: battle.project_id,
            battle_id: battle.id.clone(),
            accepted: true,
        };
        host.launch(
            store,
            storage,
            battle.id,
            ExecutionRequest::Replay {
                plan: crate::growth::demo::replay(),
            },
            admission,
        )?;
        Ok(response)
    })
    .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportRequest {
    path: PathBuf,
    /// Explicitly authorize creating a local repository and initial snapshot.
    #[serde(default)]
    initialize_git: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BriefRequest {
    name: String,
    audience: String,
    goal: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_brief_metric")]
    metric: String,
    #[serde(default = "default_brief_mode")]
    mode: crate::growth::config::PermissionMode,
}

fn default_brief_metric() -> String {
    "qualified_signup".into()
}

fn default_brief_mode() -> crate::growth::config::PermissionMode {
    crate::growth::config::PermissionMode::AnalyzeOnly
}

async fn create_brief_workspace(
    State(state): State<GrowthState>,
    Json(request): Json<BriefRequest>,
) -> ApiResult {
    with_store(state, None, move |store| {
        value(
            cli::create_brief(
                store,
                &request.name,
                &request.audience,
                &request.goal,
                &request.description,
                &request.metric,
                request.mode,
            )
            .map_err(domain_error)?,
        )
    })
    .await
}
async fn import_workspace(
    State(state): State<GrowthState>,
    Json(request): Json<ImportRequest>,
) -> ApiResult {
    with_store(state, None, move |store| {
        value(
            cli::import_with_options(store, &request.path, request.initialize_git)
                .map_err(domain_error)?,
        )
    })
    .await
}

async fn workspace(State(state): State<GrowthState>, Path(id): Path<String>) -> ApiResult {
    with_store(state, Some(id.clone()), move |store| {
        value(
            store
                .get_growth_workspace(&id)?
                .ok_or_else(|| not_found("Growth workspace"))?,
        )
    })
    .await
}

async fn hypotheses(State(state): State<GrowthState>, Path(id): Path<String>) -> ApiResult {
    with_store(state, Some(id.clone()), move |store| {
        store
            .get_growth_workspace(&id)?
            .ok_or_else(|| not_found("Growth workspace"))?;
        value(store.list_growth_hypotheses(&id)?)
    })
    .await
}

async fn create_hypotheses(State(state): State<GrowthState>, Path(id): Path<String>) -> ApiResult {
    with_store(state, Some(id.clone()), move |store| {
        let workspace = store
            .get_growth_workspace(&id)?
            .ok_or_else(|| not_found("Growth workspace"))?;
        let hypotheses = model::starter_hypotheses(&workspace);
        store.insert_growth_hypotheses(&id, &hypotheses)?;
        value(hypotheses)
    })
    .await
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HypothesisUpdateRequest {
    title: String,
    hypothesis: String,
    mechanism: String,
    baseline_definition: String,
    primary_metric: String,
    #[serde(default)]
    guardrail_metrics: Vec<String>,
    #[serde(default)]
    success_threshold: Option<String>,
    #[serde(default)]
    risks: Vec<String>,
}

fn valid_text(label: &str, value: &str, max: usize) -> std::result::Result<(), ApiError> {
    if value.trim().is_empty() || value.len() > max {
        return Err(bad_request(format!("{label} must contain 1–{max} bytes")));
    }
    Ok(())
}

fn validate_hypothesis_update(
    request: &HypothesisUpdateRequest,
) -> std::result::Result<(), ApiError> {
    for (label, value) in [
        ("title", request.title.as_str()),
        ("hypothesis", request.hypothesis.as_str()),
        ("mechanism", request.mechanism.as_str()),
        ("baselineDefinition", request.baseline_definition.as_str()),
        ("primaryMetric", request.primary_metric.as_str()),
    ] {
        valid_text(label, value, 4096)?;
    }
    if request.guardrail_metrics.len() > 16
        || request
            .guardrail_metrics
            .iter()
            .any(|value| value.trim().is_empty() || value.len() > 1024)
        || request.risks.len() > 16
        || request
            .risks
            .iter()
            .any(|value| value.trim().is_empty() || value.len() > 2048)
        || request
            .success_threshold
            .as_ref()
            .is_some_and(|value| value.len() > 4096)
    {
        return Err(bad_request(
            "Guardrails, risks and success threshold exceed the editable field limits",
        ));
    }
    if crate::growth::redaction::contains_secret(
        &serde_json::to_string(request).map_err(bad_request)?,
    ) {
        return Err(bad_request(
            "Hypothesis wording contains a possible credential",
        ));
    }
    Ok(())
}

async fn update_hypothesis(
    State(state): State<GrowthState>,
    Path((id, hypothesis_id)): Path<(String, String)>,
    Json(request): Json<HypothesisUpdateRequest>,
) -> ApiResult {
    validate_hypothesis_update(&request)?;
    with_store(state, Some(id.clone()), move |store| {
        let mut hypothesis = store
            .list_growth_hypotheses(&id)?
            .into_iter()
            .find(|item| item.id == hypothesis_id)
            .ok_or_else(|| not_found("Growth hypothesis"))?;
        hypothesis.title = request.title;
        hypothesis.hypothesis = request.hypothesis;
        hypothesis.mechanism = request.mechanism;
        hypothesis.baseline_definition = request.baseline_definition;
        hypothesis.primary_metric = request.primary_metric;
        hypothesis.guardrail_metrics = request.guardrail_metrics;
        hypothesis.success_threshold = request.success_threshold;
        hypothesis.risks = request.risks;
        store
            .update_growth_hypothesis(&id, &hypothesis)
            .map_err(domain_error)?;
        value(hypothesis)
    })
    .await
}

async fn playbook_runs(State(state): State<GrowthState>, Path(id): Path<String>) -> ApiResult {
    with_store(state, Some(id.clone()), move |store| {
        value(store.list_growth_playbook_runs(&id)?)
    })
    .await
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlaybookRunRequest {
    #[serde(default)]
    answers: Vec<String>,
}

async fn run_playbook(
    State(state): State<GrowthState>,
    Path((id, role)): Path<(String, String)>,
    Json(request): Json<PlaybookRunRequest>,
) -> ApiResult {
    if request.answers.len() > 16 || request.answers.iter().any(|answer| answer.len() > 4096) {
        return Err(bad_request(
            "Playbook answers accept up to 16 values of at most 4096 bytes",
        ));
    }
    with_store(state, Some(id.clone()), move |store| {
        let workspace = store
            .get_growth_workspace(&id)?
            .ok_or_else(|| not_found("Growth workspace"))?;
        let run = crate::growth::playbooks::execute(&workspace, &role, &request.answers)
            .map_err(domain_error)?;
        store
            .insert_growth_playbook_run(&run)
            .map_err(domain_error)?;
        value(run)
    })
    .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PrepareRequest {
    project_id: String,
    goal: String,
}
async fn prepare_battle(
    State(state): State<GrowthState>,
    Json(request): Json<PrepareRequest>,
) -> ApiResult {
    with_store(state, Some(request.project_id.clone()), move |store| {
        let battle =
            battle::prepare(store, &request.project_id, &request.goal).map_err(domain_error)?;
        value(json!({"variants":store.growth_variants(&battle.id)?,"battle":battle}))
    })
    .await
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BattleQuery {
    project_id: Option<String>,
}
async fn battles(State(state): State<GrowthState>, Query(query): Query<BattleQuery>) -> ApiResult {
    with_store(state, query.project_id.clone(), move |store| {
        value(store.list_growth_battles(query.project_id.as_deref())?)
    })
    .await
}

async fn battle_status(State(state): State<GrowthState>, Path(id): Path<String>) -> ApiResult {
    let host = state.host.clone();
    with_store(state, None, move |store| {
        let battle = get_battle(store, &id)?;
        let runs = store.sealed_growth_runs(&id)?;
        let attempts = store.growth_attempts(&id)?;
        for run in &runs {
            verify_run(store, &battle, &run.run, &run.archive_digest)?;
        }
        for attempt in &attempts {
            if attempt.archive_digest.is_none() {
                if let Some(hash) = &attempt.checkpoint_digest {
                    verify_run(store, &battle, &attempt.run, hash)?;
                }
            }
        }
        value(json!({
            "battle":battle,"variants":store.growth_variants(&id)?,
            "runs":runs,"attempts":attempts,
            "selections":store.growth_selections(&id)?,"controller":host.status(&id),
        }))
    })
    .await
}

#[derive(Clone, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
enum ExecutionRequest {
    Replay {
        plan: ReplayPlan,
    },
    Native {
        harness: String,
        model: Option<String>,
        #[serde(default = "agent_timeout", rename = "agentTimeoutSeconds")]
        timeout: u64,
    },
}
fn agent_timeout() -> u64 {
    120
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
#[path = "growth_api_tests.rs"]
mod tests;
impl ExecutionRequest {
    fn validate(&self) -> std::result::Result<(), ApiError> {
        crate::growth::confinement::available().map_err(domain_error)?;
        match self {
            Self::Replay { plan } => {
                if plan.version != 1
                    || plan.implementations.len() != 3
                    || crate::growth::redaction::contains_secret(
                        &serde_json::to_string(plan).map_err(bad_request)?,
                    )
                {
                    return Err(bad_request(
                        "Replay schema v1 requires three implementations without credentials.",
                    ));
                }
            }
            Self::Native {
                harness,
                model,
                timeout,
            } => {
                if !(1..=3600).contains(timeout)
                    || model.as_ref().is_some_and(|id| {
                        id.trim().is_empty()
                            || id.len() > 256
                            || crate::growth::redaction::contains_secret(id)
                    })
                {
                    return Err(bad_request("Invalid model identifier or agent timeout."));
                }
                let agent = local::harness::registry()
                    .into_iter()
                    .find(|agent| agent.id() == harness)
                    .ok_or_else(|| bad_request("Unknown native harness."))?;
                if !agent.one_shot_has_no_tools() {
                    return Err(bad_request("This harness cannot enforce tools-disabled growth proposals. No provider request was made."));
                }
            }
        }
        Ok(())
    }
    fn agent(self) -> Box<dyn battle::BattleAgent> {
        match self {
            Self::Replay { plan } => Box::new(battle::ReplayAgent(plan)),
            Self::Native {
                harness,
                model,
                timeout,
            } => Box::new(battle::HarnessAgent {
                harness_id: harness,
                model,
                timeout_seconds: timeout,
            }),
        }
    }
}

async fn run_battle(
    State(state): State<GrowthState>,
    Path(id): Path<String>,
    Json(request): Json<ExecutionRequest>,
) -> std::result::Result<Response, ApiError> {
    request.validate()?;
    let host = state.host.clone();
    let lifecycle = state.lifecycle.clone();
    with_owned_store(state, None, move |store, storage| {
        let battle = get_battle(&store, &id)?;
        if battle.status != BattleStatus::Ready || battle.cancel_requested {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "Battle already started or cancelled. Create a new battle to retry.".into(),
            ));
        }
        let admission = lifecycle
            .admit(&battle.project_id)
            .ok_or_else(|| ApiError(StatusCode::CONFLICT, "Workspace is being deleted.".into()))?;
        host.launch(store, storage, id.clone(), request, admission)?;
        Ok((
            StatusCode::ACCEPTED,
            Json(json!({"battleId":id,"accepted":true,"outcomeProvenance":"UNTESTED"})),
        )
            .into_response())
    })
    .await
}

async fn cancel_battle(State(state): State<GrowthState>, Path(id): Path<String>) -> ApiResult {
    with_store(state, None, move |store| {
        get_battle(store,&id)?;
        value(json!({"requested":store.request_growth_battle_cancel(&id)?,"battle":get_battle(store,&id)?}))
    }).await
}
async fn recover_battle(State(state): State<GrowthState>, Path(id): Path<String>) -> ApiResult {
    with_store(state, None, move |store| {
        value(recovery::recover(store, &id).map_err(domain_error)?)
    })
    .await
}
async fn compare_battle(State(state): State<GrowthState>, Path(id): Path<String>) -> ApiResult {
    with_store(state, None, move |store| {
        value(evaluation::compare(store, &id).map_err(domain_error)?)
    })
    .await
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReportQuery {
    #[serde(default)]
    include_context: bool,
    public_goal: Option<String>,
    #[serde(default)]
    without_attribution: bool,
    format: Option<String>,
}
async fn battle_report(
    State(state): State<GrowthState>,
    Path(id): Path<String>,
    Query(query): Query<ReportQuery>,
) -> std::result::Result<Response, ApiError> {
    with_store(state, None, move |store| {
        if !matches!(query.format.as_deref(), None | Some("html" | "markdown")) {
            return Err(bad_request("Report format must be html or markdown."));
        }
        let report = report::build(
            store,
            &id,
            &report::ReportOptions {
                include_context: query.include_context,
                public_goal: query.public_goal,
                without_attribution: query.without_attribution,
            },
        )
        .map_err(domain_error)?;
        let markdown = query.format.as_deref() == Some("markdown");
        let content = if markdown {
            report::markdown(&report)
        } else {
            report::document(&report)
        };
        let mut response = content.into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(if markdown {
                "text/markdown; charset=utf-8"
            } else {
                "text/html; charset=utf-8"
            }),
        );
        response.headers_mut().insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static(if markdown {
                "attachment; filename=GrowthLab-battle.md"
            } else {
                "attachment; filename=GrowthLab-battle.html"
            }),
        );
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        Ok(response)
    })
    .await
}

struct CapturedVariant {
    run: BattleRun,
    digest: String,
    files: BTreeMap<String, Vec<u8>>,
    sealed: bool,
}

fn verify_run(
    store: &Store,
    battle: &GrowthBattle,
    run: &BattleRun,
    hash: &str,
) -> std::result::Result<BTreeMap<String, Vec<u8>>, ApiError> {
    let files = archive::verify(store.data_root(), hash).map_err(domain_error)?;
    let contract = serde_json::to_vec(&battle.contract).map_err(bad_request)?;
    if archive::digest(&contract) != battle.contract_digest
        || files.get("contract.json") != Some(&contract)
        || files.get("run.json") != Some(&serde_json::to_vec(run).map_err(bad_request)?)
        || run.contract_digest != battle.contract_digest
    {
        return Err(bad_request(
            "Captured run does not match its frozen contract/archive. Evidence is refused.",
        ));
    }
    for (index, record) in run
        .validations
        .iter()
        .enumerate()
        .filter_map(|(index, check)| check.confinement.as_ref().map(|record| (index, record)))
        .chain(run.active_validation.iter().filter_map(|active| {
            active
                .confinement
                .as_ref()
                .map(|record| (active.command_index, record))
        }))
    {
        let policy = files
            .get(&format!("validation-{index}.policy.json"))
            .ok_or_else(|| bad_request("Captured confinement policy is missing."))?;
        crate::growth::confinement::verify_record(record, policy).map_err(domain_error)?;
    }
    crate::growth::preview::verify(battle, run, &files).map_err(domain_error)?;
    Ok(files)
}

fn variant_files(store: &Store, id: &str) -> std::result::Result<CapturedVariant, ApiError> {
    let variant = store
        .get_growth_variant(id)?
        .ok_or_else(|| not_found("Growth variant"))?;
    let (run, hash, sealed) = if let Some(seal) = store
        .sealed_growth_runs(&variant.battle_id)?
        .into_iter()
        .find(|seal| seal.run.variant_id == id)
    {
        (seal.run, seal.archive_digest, true)
    } else {
        let attempt = store
            .growth_attempts(&variant.battle_id)?
            .into_iter()
            .find(|attempt| attempt.run.variant_id == id)
            .ok_or_else(|| not_found("Captured attempt"))?;
        (
            attempt.run,
            attempt
                .checkpoint_digest
                .ok_or_else(|| not_found("Captured checkpoint"))?,
            false,
        )
    };
    let battle = get_battle(store, &variant.battle_id)?;
    let files = verify_run(store, &battle, &run, &hash)?;
    Ok(CapturedVariant {
        run,
        digest: hash,
        files,
        sealed,
    })
}

fn visible_artifact(name: &str) -> bool {
    name == "implementation.diff"
        || name == "agent.log"
        || name == crate::growth::preview::DOCUMENT
        || name == crate::growth::preview::METADATA
        || name.starts_with("files/")
        || name.strip_prefix("validation-").is_some_and(|suffix| {
            suffix.strip_suffix(".log").is_some_and(|index| {
                !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit())
            })
        })
}
async fn artifacts(State(state): State<GrowthState>, Path(id): Path<String>) -> ApiResult {
    with_store(state,None,move |store| {
        let CapturedVariant {run,digest:hash,files,sealed}=variant_files(store,&id)?;
        value(json!({"runId":run.id,"archiveDigest":hash,"sealed":sealed,"artifacts":files.iter().filter(|(name,_)|visible_artifact(name)).map(|(name,bytes)|json!({"name":name,"size":bytes.len(),"digest":archive::digest(bytes)})).collect::<Vec<_>>() }))
    }).await
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactQuery {
    name: String,
}
async fn artifact(
    State(state): State<GrowthState>,
    Path(id): Path<String>,
    Query(query): Query<ArtifactQuery>,
) -> ApiResult {
    with_store(state,None,move |store| {
        crate::growth::config::validate_relative_path(&query.name).map_err(domain_error)?;
        if !visible_artifact(&query.name) {return Err(bad_request("Artifact is not a public viewer entry. Private prompts and policies are excluded."));}
        let CapturedVariant {digest:hash,files,sealed,..}=variant_files(store,&id)?;
        let bytes=files.get(&query.name).ok_or_else(||not_found("Captured artifact"))?;
        let text=String::from_utf8(bytes.clone()).map_err(|_|bad_request("Artifact is binary; text viewing is unavailable."))?;
        value(json!({"name":query.name,"text":text,"digest":archive::digest(bytes),"archiveDigest":hash,"sealed":sealed}))
    }).await
}

/// Return data, never an executable HTML response. The UI displays this only in
/// an opaque, inert sandbox frame with the archived restrictive CSP.
async fn static_preview(State(state): State<GrowthState>, Path(id): Path<String>) -> ApiResult {
    with_store(state, None, move |store| {
        let CapturedVariant {
            run,
            digest: hash,
            files,
            sealed,
        } = variant_files(store, &id)?;
        let record = run
            .static_preview
            .as_ref()
            .filter(|record| record.status == crate::growth::preview::PreviewStatus::Ready)
            .ok_or_else(|| not_found("Archived static preview"))?;
        let html = std::str::from_utf8(
            files
                .get(crate::growth::preview::DOCUMENT)
                .ok_or_else(|| not_found("Archived static preview document"))?,
        )
        .map_err(|_| bad_request("Archived static preview is not UTF-8."))?;
        value(json!({"html":html,"record":record,"archiveDigest":hash,"sealed":sealed}))
    })
    .await
}

async fn static_preview_screenshot(
    State(state): State<GrowthState>,
    Path((id, viewport)): Path<(String, String)>,
) -> std::result::Result<Response, ApiError> {
    with_store(state, None, move |store| {
        let path = match viewport.as_str() {
            "desktop" => crate::growth::preview::SCREENSHOT_DESKTOP,
            "phone" => crate::growth::preview::SCREENSHOT_PHONE,
            _ => return Err(bad_request("Unknown static preview screenshot viewport.")),
        };
        let CapturedVariant {
            run,
            digest: _hash,
            files,
            sealed,
        } = variant_files(store, &id)?;
        let record = run
            .static_preview
            .as_ref()
            .filter(|record| record.status == crate::growth::preview::PreviewStatus::Ready)
            .ok_or_else(|| not_found("Archived static preview"))?;
        let screenshot = record
            .screenshots
            .iter()
            .find(|screenshot| screenshot.path == path)
            .ok_or_else(|| not_found("Archived static preview screenshot"))?;
        let bytes = files
            .get(path)
            .filter(|bytes| {
                bytes.len() == screenshot.size
                    && crate::growth::archive::digest(bytes) == screenshot.digest
            })
            .ok_or_else(|| {
                bad_request("Archived static preview screenshot failed verification.")
            })?;
        let mut response = bytes.clone().into_response();
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static(if sealed {
                "private, max-age=31536000, immutable"
            } else {
                "private, no-cache"
            }),
        );
        Ok(response)
    })
    .await
}

async fn select_variant(State(state): State<GrowthState>, Path(id): Path<String>) -> ApiResult {
    with_store(state, None, move |store| {
        value(selection::select(store, &id).map_err(domain_error)?)
    })
    .await
}

fn require_selected(store: &Store, id: &str) -> std::result::Result<(), ApiError> {
    let variant = store
        .get_growth_variant(id)?
        .ok_or_else(|| not_found("Growth variant"))?;
    let selected = store
        .growth_selections(&variant.battle_id)?
        .into_iter()
        .rev()
        .find(|receipt| {
            receipt.action == selection::SelectionAction::Select
                && receipt.status == selection::SelectionStatus::Done
        });
    if selected.is_none_or(|receipt| receipt.variant_id != id) {
        return Err(bad_request(
            "Record this variant as the selected candidate before dashboard delivery.",
        ));
    }
    Ok(())
}
async fn apply_preview(State(state): State<GrowthState>, Path(id): Path<String>) -> ApiResult {
    with_store(state, None, move |store| {
        require_selected(store, &id)?;
        value(selection::preview_apply(store, &id).map_err(domain_error)?)
    })
    .await
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyRequest {
    confirmed: bool,
}
async fn apply_variant(
    State(state): State<GrowthState>,
    Path(id): Path<String>,
    Json(request): Json<ApplyRequest>,
) -> ApiResult {
    if !request.confirmed {
        return Err(bad_request(
            "Review the apply preview and explicitly confirm the selected working-tree change.",
        ));
    }
    with_store(state, None, move |store| {
        require_selected(store, &id)?;
        value(selection::apply(store, &id).map_err(domain_error)?)
    })
    .await
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportRequest {
    path: PathBuf,
}
async fn export_variant(
    State(state): State<GrowthState>,
    Path(id): Path<String>,
    Json(request): Json<ExportRequest>,
) -> ApiResult {
    with_store(state, None, move |store| {
        require_selected(store, &id)?;
        value(selection::export(store, &id, &request.path).map_err(domain_error)?)
    })
    .await
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeliveryRecoveryRequest {
    #[serde(default)]
    resume: bool,
}

async fn recover_delivery(
    State(state): State<GrowthState>,
    Path(id): Path<String>,
    Json(request): Json<DeliveryRecoveryRequest>,
) -> ApiResult {
    with_store(state, None, move |store| {
        value(selection::recover_delivery(store, &id, request.resume).map_err(domain_error)?)
    })
    .await
}

async fn growth_events(
    State(state): State<GrowthState>,
) -> Sse<impl Stream<Item = std::result::Result<Event, Infallible>>> {
    let (tx, rx) = mpsc::channel(4);
    tokio::spawn(async move {
        let mut previous = None;
        loop {
            if tx.is_closed() {
                break;
            }
            let snapshot=with_store(state.clone(),None,|store| {
                let battles=store.list_growth_battles(None)?;
                let mut rows=Vec::new();
                for battle in battles {
                    rows.push(json!({"id":battle.id,"status":battle.status,"cancelRequested":battle.cancel_requested,"attempts":store.growth_attempts(&battle.id)?.iter().map(|attempt|(&attempt.run.id,&attempt.run.status,&attempt.checkpoint_digest)).collect::<Vec<_>>(),"selections":store.growth_selections(&battle.id)?.iter().map(|selection|(&selection.id,selection.status)).collect::<Vec<_>>() }));
                }
                Ok(json!({"workspaces":store.list_growth_workspaces()?.iter().map(|workspace|&workspace.project_id).collect::<Vec<_>>(),"battles":rows}))
            }).await;
            let event = match snapshot {
                Ok(snapshot) => {
                    let fingerprint = archive::digest(snapshot.to_string().as_bytes());
                    if previous.as_ref() == Some(&fingerprint) {
                        None
                    } else {
                        previous = Some(fingerprint);
                        Some(json_event("growth.updated", &snapshot))
                    }
                }
                Err(_) => {
                    previous = None;
                    Some(json_event(
                        "resync.required",
                        &json!({"reason":"GrowthLab snapshot unavailable"}),
                    ))
                }
            };
            if let Some(event) = event {
                if tx.send(event).await.is_err() {
                    break;
                }
            }
            tokio::select! {_=tx.closed()=>break,_=tokio::time::sleep(Duration::from_millis(500))=>{}}
        }
    });
    let guard = DashboardClientGuard::new();
    let stream = futures::stream::unfold((rx, guard), |(mut rx, guard)| async move {
        rx.recv().await.map(|event| (Ok(event), (rx, guard)))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
