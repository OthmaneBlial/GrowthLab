use super::*;

struct Fixture {
    root: PathBuf,
    product: PathBuf,
    state: GrowthState,
    server: tokio::task::JoinHandle<()>,
    client: reqwest::Client,
    url: String,
}

impl Fixture {
    async fn new(gated: bool) -> Self {
        let root = std::env::temp_dir().join(format!("growth-api-owned-{}", uuid::Uuid::new_v4()));
        archive::private_directory(&root).unwrap();
        let product = root.join("product");
        std::fs::create_dir_all(product.join("website")).unwrap();
        let mut config = crate::growth::config::fixture();
        config.validation.commands = vec![if gated {
            r#"node -e "const fs=require('node:fs');fs.writeFileSync('ready','started');setInterval(()=>{},50)""#.into()
        } else {
            r#"node -e "const fs=require('node:fs');fs.writeFileSync('ready','started');const timer=setInterval(()=>{if(fs.existsSync('release')){clearInterval(timer);console.log('actual fixture check');process.exit(fs.readFileSync('website/index.html','utf8').includes('<h1>')?0:2)}},20)""#.into()
        }];
        config.validation.timeout_seconds = 60;
        config.write_new(&product).unwrap();
        std::fs::write(product.join("website/index.html"), "<h1>Baseline</h1>").unwrap();
        crate::local::git::git(Some(&product), &["init", "-b", "main"]).unwrap();
        crate::local::git::git(
            Some(&product),
            &["add", "growthlab.yaml", "website/index.html"],
        )
        .unwrap();
        crate::local::git::git(
            Some(&product),
            &[
                "-c",
                "user.name=Growth fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "core.hooksPath=.owned-disabled-hooks",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "Owned API fixture",
            ],
        )
        .unwrap();
        let state = GrowthState {
            host: Arc::new(GrowthHost::default()),
            lifecycle: Arc::new(ProjectLifecycle::default()),
            gate: Arc::new(tokio::sync::Mutex::new(())),
            moving: Arc::new(AtomicBool::new(false)),
            stopping: Arc::new(AtomicBool::new(false)),
            root: Some(root.join("lab")),
        };
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let app = build_routes()
            .with_state(state.clone())
            .layer(middleware::from_fn(
                crate::commands::up_remote::loopback_guard,
            ));
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            root,
            product,
            state,
            server,
            url,
            client: reqwest::Client::new(),
        }
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
        expected: u16,
    ) -> Value {
        let mut request = self
            .client
            .request(method, format!("{}/api/growth{path}", self.url));
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        let text = response.text().await.unwrap();
        assert_eq!(status, expected, "{path}: {text}");
        serde_json::from_str(&text).unwrap()
    }
    async fn get(&self, path: &str) -> Value {
        self.request(reqwest::Method::GET, path, None, 200).await
    }
    async fn post(&self, path: &str, body: Option<Value>, expected: u16) -> Value {
        self.request(reqwest::Method::POST, path, body, expected)
            .await
    }
    async fn prepare(&self) -> Value {
        let workspace = self
            .post("/workspaces", Some(json!({"path":self.product})), 200)
            .await;
        self.post("/battles", Some(json!({"projectId":workspace["projectId"],"goal":"Compare synthetic activation approaches"})), 200).await
    }
    fn replay() -> Value {
        let implementations = ["<h1>Outcome first</h1>", "<p>Missing heading</p>", "<h1>Quick start</h1>"].map(|html| json!({"summary":"Declared synthetic proposal","files":[{"path":"website/index.html","contents":html}],"risks":["No outcome measurement"]}));
        json!({"mode":"replay","plan":{"version":1,"implementations":implementations}})
    }
    async fn wait(&self, id: &str, terminal: bool) -> Value {
        self.wait_with_timeout(id, terminal, Duration::from_secs(20))
            .await
    }
    async fn wait_with_timeout(&self, id: &str, terminal: bool, timeout: Duration) -> Value {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let status = self.get(&format!("/battles/{id}")).await;
            let reached = if terminal {
                status["controller"]["running"] == false
                    && status["runs"]
                        .as_array()
                        .is_some_and(|runs| runs.len() == 3)
            } else {
                status["attempts"].as_array().is_some_and(|attempts| {
                    attempts.len() == 3
                        && attempts.iter().all(|attempt| {
                            let Some(id) = attempt["run"]["activeValidation"]["runId"].as_str()
                            else {
                                return false;
                            };
                            let directory = self.state.data_root().join("growth-jobs").join(id);
                            directory.join("pid").is_file()
                                && crate::jobs::localbox::inspect_job(&directory).stage == "RUNNING"
                                && directory.join("repo/ready").exists()
                        })
                })
            };
            if reached {
                return status;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "controller did not reach requested state: {status}"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
        let store = Store::open_at(self.state.data_root()).unwrap();
        for battle in store.list_growth_battles(None).unwrap() {
            let _ = store.request_growth_battle_cancel(&battle.id);
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while self
            .state
            .host
            .workers
            .lock()
            .unwrap()
            .values()
            .any(|worker| worker.running)
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        if !self
            .state
            .host
            .workers
            .lock()
            .unwrap()
            .values()
            .any(|worker| worker.running)
        {
            drop(store);
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

#[tokio::test]
async fn api_bundled_demo_runs_real_checks_without_touching_an_existing_product() {
    let fixture = Fixture::new(false).await;
    let baseline = crate::local::git::git(Some(&fixture.product), &["rev-parse", "HEAD"]).unwrap();
    let invalid = fixture
        .client
        .post(format!("{}/api/growth/demo", fixture.url))
        .json(&json!({"unexpected": true}))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status().as_u16(), 422);
    assert!(fixture
        .get("/workspaces")
        .await
        .as_array()
        .unwrap()
        .is_empty());
    let launched = fixture.post("/demo", Some(json!({})), 202).await;
    assert_eq!(launched["accepted"], true);
    let id = launched["battleId"].as_str().unwrap();
    // Nine real subprocess checks compete with the full parallel Rust suite.
    // Keep every terminal/seal/exit-code assertion, but allow the fixture's
    // configured 60-second command budget before declaring controller failure.
    let terminal = fixture
        .wait_with_timeout(id, true, Duration::from_secs(60))
        .await;
    assert_eq!(terminal["battle"]["status"], "failed");
    assert!(terminal["selections"].as_array().unwrap().is_empty());
    assert_eq!(terminal["runs"].as_array().unwrap().len(), 3);
    let comparison = fixture.get(&format!("/battles/{id}/compare")).await;
    assert_eq!(
        comparison["recommendedCandidates"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    for (index, row) in comparison["rows"].as_array().unwrap().iter().enumerate() {
        assert_eq!(row["implementationProvenance"], "SIMULATED");
        assert_eq!(row["checkProvenance"], "OBSERVED");
        assert_eq!(row["outcomeProvenance"], "UNTESTED");
        assert_eq!(row["eligible"], index != 1);
        assert_eq!(row["requiredCommands"], 3);
        assert_eq!(row["passedCommands"], if index == 1 { 2 } else { 3 });
        for (check_index, check) in row["checks"].as_array().unwrap().iter().enumerate() {
            assert_eq!(
                check["exitCode"],
                if index == 1 && check_index == 0 { 2 } else { 0 }
            );
            assert!(!check["confinement"]["policyDigest"]
                .as_str()
                .unwrap()
                .is_empty());
        }
        let variant = row["variantId"].as_str().unwrap();
        let response = fixture
            .client
            .get(format!(
                "{}/api/growth/variants/{variant}/static-preview",
                fixture.url
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 200);
        assert!(response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("application/json"));
        let preview: Value = response.json().await.unwrap();
        let html = preview["html"].as_str().unwrap();
        assert!(html.contains(crate::growth::preview::CSP));
        assert!(html.contains("data:text/css;charset=utf-8;base64,"));
        assert_eq!(preview["record"]["status"], "ready");
        assert_eq!(
            preview["record"]["documentDigest"],
            archive::digest(html.as_bytes())
        );
        assert_eq!(preview["record"]["blockedResources"], 0);
        assert_eq!(preview["record"]["sources"].as_array().unwrap().len(), 2);
        assert_eq!(preview["sealed"], true);
        assert_eq!(preview["archiveDigest"], row["archiveDigest"]);
        let run = terminal["runs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|seal| seal["run"]["variantId"] == variant)
            .unwrap();
        assert_eq!(preview["record"], run["run"]["staticPreview"]);
        assert_eq!(
            preview["record"]["sourceCommit"],
            run["run"]["candidateCommit"]
        );
        let artifacts = fixture.get(&format!("/variants/{variant}/artifacts")).await;
        assert_eq!(artifacts["sealed"], true);
        assert!(artifacts["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|artifact| !artifact["name"]
                .as_str()
                .unwrap()
                .starts_with("preview/source/")));
        let document = fixture
            .get(&format!(
                "/variants/{variant}/artifact?name=preview/document.html"
            ))
            .await;
        assert_eq!(document["text"], html);
        let source = fixture
            .get(&format!(
                "/variants/{variant}/artifact?name=files/website/index.html"
            ))
            .await;
        assert_eq!(
            source["text"],
            crate::growth::demo::replay().implementations[index].files[0]
                .contents
                .as_deref()
                .unwrap()
        );
        let log = fixture
            .get(&format!(
                "/variants/{variant}/artifact?name=validation-0.log"
            ))
            .await;
        assert!(log["text"]
            .as_str()
            .unwrap()
            .contains("exactlyOnePrimaryHeading"));
    }
    let store = Store::open_at(fixture.state.data_root()).unwrap();
    let product = PathBuf::from(
        store
            .get_local_project(launched["projectId"].as_str().unwrap())
            .unwrap()
            .unwrap()
            .repo_path,
    );
    assert!(product.canonicalize().unwrap().starts_with(
        fixture
            .state
            .data_root()
            .join("growth-demo")
            .canonicalize()
            .unwrap()
    ));
    assert_eq!(
        std::fs::read_to_string(product.join("website/index.html")).unwrap(),
        include_str!("../../../demo/patchkit/website/index.html")
    );
    assert!(
        crate::local::git::git(Some(&product), &["status", "--porcelain"])
            .unwrap()
            .trim()
            .is_empty()
    );
    assert!(crate::local::git::git(Some(&product), &["remote"])
        .unwrap()
        .trim()
        .is_empty());
    assert_eq!(
        crate::local::git::git(Some(&product), &["rev-parse", "HEAD"])
            .unwrap()
            .trim(),
        terminal["battle"]["contract"]["sourceSnapshotCommit"]
            .as_str()
            .unwrap()
    );
    assert_eq!(
        crate::local::git::git(Some(&fixture.product), &["rev-parse", "HEAD"]).unwrap(),
        baseline
    );
    assert!(
        crate::local::git::git(Some(&fixture.product), &["status", "--porcelain"])
            .unwrap()
            .trim()
            .is_empty()
    );
    // Even the static-preview JSON route refuses changed sealed bytes; neither
    // comparison nor selection may consume the altered candidate evidence.
    let row = &comparison["rows"][0];
    let variant = row["variantId"].as_str().unwrap();
    let document = store
        .data_root()
        .join("growth-archives")
        .join(row["archiveDigest"].as_str().unwrap())
        .join(crate::growth::preview::DOCUMENT);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&document, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    std::fs::write(document, "<h1>Tampered archived preview</h1>").unwrap();
    fixture
        .request(
            reqwest::Method::GET,
            &format!("/variants/{variant}/static-preview"),
            None,
            400,
        )
        .await;
    fixture
        .request(reqwest::Method::GET, &format!("/battles/{id}"), None, 400)
        .await;
    fixture
        .request(
            reqwest::Method::GET,
            &format!("/battles/{id}/compare"),
            None,
            400,
        )
        .await;
    fixture
        .post(&format!("/variants/{variant}/select"), None, 400)
        .await;
}

#[tokio::test]
async fn api_runs_real_variants_and_serves_verified_evidence_and_explicit_delivery() {
    let fixture = Fixture::new(false).await;
    let baseline = crate::local::git::git(Some(&fixture.product), &["rev-parse", "HEAD"]).unwrap();
    let prepared = fixture.prepare().await;
    let id = prepared["battle"]["id"].as_str().unwrap();
    fixture
        .post(&format!("/battles/{id}/run"), Some(Fixture::replay()), 202)
        .await;
    let live = fixture.wait(id, false).await;
    assert_eq!(live["battle"]["status"], "running");
    assert_eq!(live["controller"]["running"], true);
    let other_dashboard = Store::open_at(fixture.state.data_root()).unwrap();
    assert!(
        other_dashboard.acquire_data_dir_move_lock().is_err(),
        "a live API worker pins storage across dashboards"
    );
    assert!(fixture
        .state
        .lifecycle
        .begin_delete(prepared["battle"]["projectId"].as_str().unwrap())
        .is_none());
    assert!(
        fixture.state.gate.try_lock().is_ok(),
        "live readers must not wait for battle completion"
    );
    fixture
        .post(&format!("/battles/{id}/run"), Some(Fixture::replay()), 409)
        .await;
    // Release only the three owned registered snapshots after proving all jobs
    // are live. Timers cannot guarantee overlap during synchronous Git setup.
    for attempt in live["attempts"].as_array().unwrap() {
        let job_id = attempt["run"]["activeValidation"]["runId"]
            .as_str()
            .unwrap();
        std::fs::write(
            fixture
                .state
                .data_root()
                .join("growth-jobs")
                .join(job_id)
                .join("repo/release"),
            "owned fixture release",
        )
        .unwrap();
    }
    let terminal = fixture.wait(id, true).await;
    assert_eq!(terminal["battle"]["status"], "failed");
    assert!(
        other_dashboard.acquire_data_dir_move_lock().is_ok(),
        "finished workers release their storage lease"
    );
    let comparison = fixture.get(&format!("/battles/{id}/compare")).await;
    assert_eq!(
        comparison["recommendedCandidates"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(comparison["rows"][1]["checks"][0]["exitCode"], 2);
    assert_eq!(
        std::fs::read_to_string(fixture.product.join("website/index.html")).unwrap(),
        "<h1>Baseline</h1>"
    );
    let variant = prepared["variants"][0]["id"].as_str().unwrap();
    let prefix = format!("/variants/{variant}");
    let artifacts = fixture.get(&format!("{prefix}/artifacts")).await;
    assert_eq!(artifacts["sealed"], true);
    assert!(artifacts["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|artifact| artifact["name"] == "implementation.diff"));
    let diff = fixture
        .get(&format!("{prefix}/artifact?name=implementation.diff"))
        .await;
    assert!(diff["text"]
        .as_str()
        .unwrap()
        .contains("+<h1>Outcome first</h1>"));
    let log = fixture
        .get(&format!("{prefix}/artifact?name=validation-0.log"))
        .await;
    assert!(log["text"]
        .as_str()
        .unwrap()
        .contains("actual fixture check"));
    for name in [
        "../run.json",
        "proposal-input.json",
        "validation-0.policy.json",
    ] {
        fixture
            .request(
                reqwest::Method::GET,
                &format!("{prefix}/artifact?name={name}"),
                None,
                400,
            )
            .await;
    }
    fixture
        .request(
            reqwest::Method::GET,
            &format!("{prefix}/apply-preview"),
            None,
            400,
        )
        .await;
    fixture.post(&format!("{prefix}/select"), None, 200).await;
    let preview = fixture.get(&format!("{prefix}/apply-preview")).await;
    assert_eq!(preview["sourceCommit"], baseline.trim());
    fixture
        .post(
            &format!("{prefix}/apply"),
            Some(json!({"confirmed":false})),
            400,
        )
        .await;
    assert_eq!(
        std::fs::read_to_string(fixture.product.join("website/index.html")).unwrap(),
        "<h1>Baseline</h1>"
    );
    fixture
        .post(
            &format!("{prefix}/export"),
            Some(json!({"path":fixture.root.join("selected.patch")})),
            200,
        )
        .await;
    assert!(std::fs::read_to_string(fixture.root.join("selected.patch"))
        .unwrap()
        .contains("+<h1>Outcome first</h1>"));
    fixture
        .post(
            &format!("{prefix}/export"),
            Some(json!({"path":fixture.root.join("selected.patch")})),
            400,
        )
        .await;
    let report = fixture
        .client
        .get(format!("{}/api/growth/battles/{id}/report", fixture.url))
        .send()
        .await
        .unwrap();
    assert_eq!(report.status().as_u16(), 200);
    assert!(report.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .contains("attachment"));
    let report = report.text().await.unwrap();
    assert!(!report.contains(fixture.root.to_str().unwrap()));
    assert!(
        report.contains("Policy SHA-256")
            && report.contains("SIMULATED")
            && report.contains("UNTESTED")
    );
    fixture
        .post(
            &format!("{prefix}/apply"),
            Some(json!({"confirmed":true})),
            200,
        )
        .await;
    assert_eq!(
        std::fs::read_to_string(fixture.product.join("website/index.html")).unwrap(),
        "<h1>Outcome first</h1>"
    );
    assert_eq!(
        crate::local::git::git(Some(&fixture.product), &["rev-parse", "HEAD"]).unwrap(),
        baseline
    );
    assert_eq!(
        crate::local::git::git(Some(&fixture.product), &["diff", "--cached", "--name-only"])
            .unwrap(),
        ""
    );
    assert_eq!(
        crate::local::git::git(Some(&fixture.product), &["remote"]).unwrap(),
        ""
    );
    let store = Store::open_at(fixture.state.data_root()).unwrap();
    let path = store
        .data_root()
        .join("growth-archives")
        .join(artifacts["archiveDigest"].as_str().unwrap())
        .join("implementation.diff");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    std::fs::write(path, "tampered synthetic evidence").unwrap();
    fixture
        .request(
            reqwest::Method::GET,
            &format!("{prefix}/artifact?name=implementation.diff"),
            None,
            400,
        )
        .await;
    fixture
        .request(
            reqwest::Method::GET,
            &format!("/battles/{id}/compare"),
            None,
            400,
        )
        .await;
    fixture
        .request(reqwest::Method::GET, &format!("/battles/{id}"), None, 400)
        .await;
}

#[tokio::test]
async fn cancellation_shutdown_and_storage_guards_preserve_actual_jobs_and_product() {
    let fixture = Fixture::new(true).await;
    let prepared = fixture.prepare().await;
    let id = prepared["battle"]["id"].as_str().unwrap();
    fixture.state.moving.store(true, Ordering::SeqCst);
    fixture
        .post(&format!("/battles/{id}/run"), Some(Fixture::replay()), 409)
        .await;
    assert!(fixture.state.host.status(id).is_none());
    fixture.state.moving.store(false, Ordering::SeqCst);
    fixture
        .post(&format!("/battles/{id}/run"), Some(Fixture::replay()), 202)
        .await;
    let live = fixture.wait(id, false).await;
    let jobs: Vec<_> = live["attempts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|attempt| {
            fixture.state.data_root().join("growth-jobs").join(
                attempt["run"]["activeValidation"]["runId"]
                    .as_str()
                    .unwrap(),
            )
        })
        .collect();
    assert_eq!(jobs.len(), 3);
    fixture
        .post(&format!("/battles/{id}/recover"), None, 400)
        .await;
    fixture
        .post(&format!("/battles/{id}/cancel"), None, 200)
        .await;
    fixture.state.stopping.store(true, Ordering::SeqCst);
    fixture.state.host.shutdown().await;
    assert_eq!(
        Store::open_at(fixture.state.data_root())
            .unwrap()
            .get_growth_battle(id)
            .unwrap()
            .unwrap()
            .status,
        BattleStatus::Cancelled
    );
    assert!(jobs
        .iter()
        .all(|directory| crate::jobs::localbox::inspect_job(directory).stage != "RUNNING"));
    assert!(fixture
        .state
        .lifecycle
        .begin_delete(prepared["battle"]["projectId"].as_str().unwrap())
        .is_some());
    fixture
        .post(&format!("/battles/{id}/run"), Some(Fixture::replay()), 503)
        .await;
    assert_eq!(
        std::fs::read_to_string(fixture.product.join("website/index.html")).unwrap(),
        "<h1>Baseline</h1>"
    );
}

#[tokio::test]
async fn events_resync_persisted_state_and_cross_origin_mutations_are_refused() {
    let fixture = Fixture::new(false).await;
    let response = fixture
        .client
        .post(format!("{}/api/growth/workspaces", fixture.url))
        .header(header::ORIGIN, "https://unrelated.example.invalid")
        .json(&json!({"path":fixture.product}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 403);
    assert!(fixture
        .get("/workspaces")
        .await
        .as_array()
        .unwrap()
        .is_empty());
    let mut events = fixture
        .client
        .get(format!("{}/api/growth/events", fixture.url))
        .send()
        .await
        .unwrap();
    let first = tokio::time::timeout(Duration::from_secs(5), events.chunk())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(String::from_utf8_lossy(&first).contains("event: growth.updated"));
    let prepared = fixture.prepare().await;
    let id = prepared["battle"]["id"].as_str().unwrap();
    let mut text = String::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !text.contains(id) {
            text.push_str(&String::from_utf8_lossy(
                &events.chunk().await.unwrap().unwrap(),
            ));
        }
    })
    .await
    .unwrap();
    assert!(text.contains("growth.updated") && text.contains("ready"));
    assert!(!text.contains(fixture.root.to_str().unwrap()));
    fixture
        .post(
            &format!("/battles/{id}/run"),
            Some(json!({"mode":"replay","plan":{"version":2,"implementations":[]}})),
            400,
        )
        .await;
    fixture
        .post(
            &format!("/battles/{id}/run"),
            Some(json!({"mode":"native","harness":"unknown-provider","model":null})),
            400,
        )
        .await;
    assert_eq!(
        fixture.get(&format!("/battles/{id}")).await["battle"]["status"],
        "ready"
    );
    assert!(fixture.state.host.status(id).is_none());
}

#[tokio::test]
async fn playbook_catalog_is_read_only_and_covers_the_initial_growth_roles() {
    let fixture = Fixture::new(false).await;
    let response = fixture.get("/playbooks").await;
    let playbooks = response.as_array().unwrap();
    assert_eq!(playbooks.len(), 10);
    let ids: std::collections::HashSet<_> = playbooks
        .iter()
        .map(|playbook| playbook["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 10);
    assert!(playbooks.iter().any(|playbook| {
        playbook["id"] == "seo"
            && playbook["role"] == "seo"
            && playbook["guardrails"]
                .as_array()
                .unwrap()
                .iter()
                .any(|guardrail| guardrail.as_str().unwrap().contains("keyword"))
    }));
    for playbook in playbooks {
        assert!(!playbook["questions"].as_array().unwrap().is_empty());
        assert!(!playbook["outputs"].as_array().unwrap().is_empty());
        assert!(!playbook["guardrails"].as_array().unwrap().is_empty());
    }
    assert!(fixture
        .get("/workspaces")
        .await
        .as_array()
        .unwrap()
        .is_empty());
    assert!(fixture.state.host.workers.lock().unwrap().is_empty());
}

#[tokio::test]
async fn api_initializes_a_local_folder_only_when_requested() {
    let fixture = Fixture::new(false).await;
    let product = fixture.root.join("unversioned-product");
    std::fs::create_dir_all(product.join("website")).unwrap();
    crate::growth::config::fixture()
        .write_new(&product)
        .unwrap();
    std::fs::write(product.join("website/index.html"), "<h1>Local product</h1>").unwrap();

    fixture
        .post("/workspaces", Some(json!({"path":product})), 400)
        .await;
    let workspace = fixture
        .post(
            "/workspaces",
            Some(json!({"path":product,"initializeGit":true})),
            200,
        )
        .await;
    assert!(!workspace["projectId"].as_str().unwrap().is_empty());
    assert_eq!(
        crate::local::git::git(Some(&product), &["status", "--porcelain"]).unwrap(),
        ""
    );
    assert_eq!(
        crate::local::git::git(Some(&product), &["remote"]).unwrap(),
        ""
    );
}

#[tokio::test]
async fn url_audit_refuses_non_https_input_without_network_access() {
    let fixture = Fixture::new(false).await;
    let response = fixture
        .post(
            "/url-audit",
            Some(json!({"url":"http://example.com/"})),
            400,
        )
        .await;
    assert!(response["error"]
        .as_str()
        .is_some_and(|message| message.contains("HTTPS URLs only")));
    assert!(fixture
        .get("/workspaces")
        .await
        .as_array()
        .unwrap()
        .is_empty());
}
