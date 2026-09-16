//! Original key-free product fixture. Execution is always the real battle engine.
use std::path::Path;
use std::process::Command;

use crate::error::{anyhow, Result};
use crate::store::Store;

use super::battle_model::{FileEdit, GrowthBattle, Implementation, ReplayPlan};
use super::config::{
    Agents, Goal, GrowthConfig, Metrics, PermissionMode, Permissions, Product, Validation,
};

const BASELINE: &str = include_str!("../../demo/patchkit/website/index.html");
const STYLE: &str = include_str!("../../demo/patchkit/website/style.css");
const VALIDATOR: &str = include_str!("../../demo/patchkit/website/validate.mjs");

pub fn replay() -> ReplayPlan {
    ReplayPlan {
        version: 1,
        implementations: [
            ("Outcome-first positioning", include_str!("../../demo/patchkit/outcome-first.html")),
            ("Proof beside the promise with a deliberate heading regression", include_str!("../../demo/patchkit/proof-first.html")),
            ("Faster first success", include_str!("../../demo/patchkit/first-success.html")),
        ].into_iter().map(|(title, contents)| Implementation {
            summary: format!("Declared fictional PatchKit replay: {title}. No provider was called."),
            files: vec![FileEdit { path: "website/index.html".into(), contents: Some(contents.into()) }],
            risks: vec!["Explicit fixture checks do not establish comprehensive accessibility, performance or measured activation.".into()],
        }).collect(),
    }
}

pub fn prepare(store: &Store) -> Result<GrowthBattle> {
    super::confinement::available()?;
    // The battle engine uses generic Git operations. Refuse repository overrides
    // that could redirect them away from this newly owned fictional product.
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_CONFIG_COUNT",
        "GIT_CONFIG_PARAMETERS",
    ] {
        if crate::local::shell_env::var(key).is_some_and(|value| !value.is_empty()) {
            return Err(anyhow!("Unset repository-scoped Git environment overrides before running the bundled demo."));
        }
    }
    let parent = store.data_root().join("growth-demo");
    super::archive::private_directory(&parent)?;
    let owned = parent.join(uuid::Uuid::new_v4().to_string());
    super::archive::private_directory(&owned)?;
    let product = owned.join("product");
    super::archive::private_directory(&product)?;
    super::archive::private_directory(&product.join("website"))?;
    let config = GrowthConfig {
        version: 1,
        product: Product { name: "PatchKit · fictional demo".into(), audience: "Open-source maintainers".into(), description: "Original bundled static product for a declared, key-free replay; no customers or outcome telemetry.".into() },
        goal: Goal { primary: "Make the first useful action clear on the homepage".into() },
        permissions: Permissions { mode: PermissionMode::Implement, allowed_paths: vec!["website".into()], denied_paths: vec!["website/private".into()] },
        validation: Validation { commands: ["structure", "links", "claims"].map(|check| format!("node website/validate.mjs {check}")).to_vec(), timeout_seconds: 60 },
        metrics: Metrics { primary: "qualified_activation".into(), guardrails: vec!["page_load_time".into()] },
        agents: Agents { parallelism: 3 },
        static_preview: Some(super::config::StaticPreview { root: "website".into(), entry: "index.html".into() }),
    };
    config.write_new(&product)?;
    for (name, contents) in [
        ("index.html", BASELINE),
        ("style.css", STYLE),
        ("validate.mjs", VALIDATOR),
    ] {
        std::fs::write(product.join("website").join(name), contents)?;
    }
    git(&product, &["init", "-b", "main"])?;
    for (key, value) in [
        ("user.name", "GrowthLab fictional fixture"),
        ("user.email", "fixture@example.invalid"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", ".growthlab-disabled-hooks"),
        ("core.fsmonitor", "false"),
        (
            "core.attributesFile",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        ),
        ("core.autocrlf", "false"),
    ] {
        git(&product, &["config", "--local", key, value])?;
    }
    git(&product, &["add", "growthlab.yaml", "website"])?;
    git(
        &product,
        &["commit", "-m", "Original fictional GrowthLab demo baseline"],
    )?;
    let workspace = super::cli::import(store, &product)?;
    super::battle::prepare(store, &workspace.project_id, &config.goal.primary)
}

fn git(product: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .current_dir(product)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .args(args)
        .output()
        .map_err(|_| anyhow!("Git is required to initialize the bundled fictional product."))?;
    if !output.status.success() {
        return Err(anyhow!("Could not initialize the owned fictional demo repository; original product repositories were not modified."));
    }
    Ok(())
}
