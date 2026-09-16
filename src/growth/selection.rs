//! Explicit selection and delivery of an eligible sealed candidate. Product
//! apply copies one verified patch to a clean baseline; it never commits/pushes.
use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use super::archive::{self, digest};
use super::battle_model::{GrowthBattle, GrowthVariant, SealedBattleRun};
use super::config::GrowthConfig;
use super::model::Decision;
use super::redaction::{contains_secret, redact};
use crate::error::{anyhow, Result};
use crate::store::{now_ms, Store};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelectionAction {
    Select,
    Apply,
    Export,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelectionStatus {
    Pending,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectionRecord {
    pub id: String,
    pub battle_id: String,
    pub variant_id: String,
    pub run_id: String,
    pub archive_digest: String,
    pub candidate_commit: String,
    pub action: SelectionAction,
    pub decision: Decision,
    pub status: SelectionStatus,
    pub artifact_digest: Option<String>,
    /// Local audit metadata. Never included in a shareable report.
    pub output_path: Option<String>,
    pub created_at: i64,
    pub ended_at: Option<i64>,
    pub error: Option<String>,
}

pub struct SelectedCandidate {
    pub battle: GrowthBattle,
    pub variant: GrowthVariant,
    pub sealed: SealedBattleRun,
    pub files: BTreeMap<String, Vec<u8>>,
}

pub fn candidate(store: &Store, variant_id: &str) -> Result<SelectedCandidate> {
    let variant = store
        .get_growth_variant(variant_id)?
        .ok_or_else(|| anyhow!("Growth variant not found"))?;
    let comparison = super::evaluation::compare(store, &variant.battle_id)?;
    if !comparison
        .recommended_candidates
        .iter()
        .any(|id| id == variant_id)
    {
        return Err(anyhow!(
            "Selected variant is not an eligible sealed candidate; inspect its validation"
        ));
    }
    let battle = store
        .get_growth_battle(&variant.battle_id)?
        .ok_or_else(|| anyhow!("Battle not found"))?;
    let sealed = store
        .sealed_growth_runs(&battle.id)?
        .into_iter()
        .find(|sealed| sealed.run.variant_id == variant.id)
        .ok_or_else(|| anyhow!("Selected candidate evidence is missing"))?;
    let files = archive::verify(store.data_root(), &sealed.archive_digest)?;
    Ok(SelectedCandidate {
        battle,
        variant,
        sealed,
        files,
    })
}

fn record(selected: &SelectedCandidate, action: SelectionAction) -> Result<SelectionRecord> {
    Ok(SelectionRecord {
        id: uuid::Uuid::new_v4().to_string(),
        battle_id: selected.battle.id.clone(),
        variant_id: selected.variant.id.clone(),
        run_id: selected.sealed.run.id.clone(),
        archive_digest: selected.sealed.archive_digest.clone(),
        candidate_commit: selected
            .sealed
            .run
            .candidate_commit
            .clone()
            .ok_or_else(|| anyhow!("Selected candidate commit is missing"))?,
        action,
        decision: Decision::Candidate,
        status: SelectionStatus::Pending,
        artifact_digest: None,
        output_path: None,
        created_at: now_ms(),
        ended_at: None,
        error: None,
    })
}

pub fn select(store: &Store, variant_id: &str) -> Result<SelectionRecord> {
    let selected = candidate(store, variant_id)?;
    let mut receipt = record(&selected, SelectionAction::Select)?;
    receipt.status = SelectionStatus::Done;
    receipt.ended_at = Some(now_ms());
    store.create_growth_selection(&receipt)?;
    Ok(receipt)
}

fn safe_git(repo: &Path, args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>> {
    let mut command = Command::new("git");
    command.current_dir(repo).env_clear();
    for key in [
        "PATH",
        "SystemRoot",
        "COMSPEC",
        "TEMP",
        "TMP",
        "TMPDIR",
        "LANG",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    let disabled =
        std::env::temp_dir().join(format!(".growthlab-no-hooks-{}", uuid::Uuid::new_v4()));
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args([
            "--no-pager",
            "-c",
            &format!("core.hooksPath={}", disabled.display()),
            "-c",
            "core.fsmonitor=false",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|_| anyhow!("Selected candidate Git operation could not start"))?;
    if let Some(bytes) = input {
        if let Err(error) = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Git input is unavailable"))?
            .write_all(bytes)
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error.into());
        }
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(anyhow!("Selected candidate Git operation failed; inspect the baseline and candidate availability"));
    }
    if output.stdout.len() > 4 * 1024 * 1024 {
        return Err(anyhow!(
            "Selected candidate output exceeds its safety limit"
        ));
    }
    Ok(output.stdout)
}

fn git_text(repo: &Path, args: &[&str]) -> Result<String> {
    String::from_utf8(safe_git(repo, args, None)?)
        .map(|text| text.trim().to_owned())
        .map_err(|_| anyhow!("Git metadata is not UTF-8"))
}

fn valid_commit(commit: &str) -> bool {
    matches!(commit.len(), 40 | 64) && commit.chars().all(|c| c.is_ascii_hexdigit())
}

/// Build a portable patch from exact immutable object IDs, verifying changed
/// blobs against the sealed artifacts. Mutable branches/worktrees are ignored.
fn patch(store: &Store, selected: &SelectedCandidate) -> Result<(PathBuf, Vec<u8>, Vec<String>)> {
    let project = store
        .get_local_project(&selected.battle.project_id)?
        .ok_or_else(|| anyhow!("Product workspace is unavailable"))?;
    let repo = crate::paths::canonicalize(Path::new(&project.repo_path))?;
    let source = &selected.battle.contract.source_snapshot_commit;
    let commit = selected
        .sealed
        .run
        .candidate_commit
        .as_deref()
        .ok_or_else(|| anyhow!("Candidate commit is unavailable"))?;
    if !valid_commit(source) || !valid_commit(commit) {
        return Err(anyhow!("Invalid immutable candidate commit"));
    }
    let implementation = selected
        .sealed
        .run
        .implementation
        .as_ref()
        .ok_or_else(|| anyhow!("Candidate implementation is unavailable"))?;
    let expected: HashSet<_> = implementation
        .files
        .iter()
        .map(|edit| edit.path.as_str())
        .collect();
    if expected.len() != implementation.files.len() || expected.is_empty() {
        return Err(anyhow!(
            "Selected candidate contains duplicate or missing file declarations"
        ));
    }
    // Modes and paths come from committed objects, not editable proposal text.
    let raw = safe_git(
        &repo,
        &[
            "diff",
            "--raw",
            "--no-ext-diff",
            "--no-textconv",
            "-z",
            "--no-abbrev",
            "--no-renames",
            source,
            commit,
            "--",
        ],
        None,
    )?;
    let fields: Vec<_> = raw
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .collect();
    let (pairs, remainder) = fields.as_chunks::<2>();
    if pairs.is_empty() || !remainder.is_empty() {
        return Err(anyhow!("Candidate change metadata is missing or malformed"));
    }
    let mut paths = Vec::new();
    for pair in pairs {
        let metadata =
            std::str::from_utf8(pair[0]).map_err(|_| anyhow!("Invalid candidate metadata"))?;
        let path = std::str::from_utf8(pair[1]).map_err(|_| anyhow!("Invalid candidate path"))?;
        selected.battle.contract.config.check_permission(path)?;
        if !expected.contains(path) {
            return Err(anyhow!(
                "Candidate changed an undeclared file; delivery refused"
            ));
        }
        let columns: Vec<_> = metadata
            .trim_start_matches(':')
            .split_whitespace()
            .collect();
        if columns.len() != 5
            || !["000000", "100644", "100755"].contains(&columns[0])
            || !["000000", "100644", "100755"].contains(&columns[1])
        {
            return Err(anyhow!("Candidate contains a non-regular file change"));
        }
        let edit = implementation
            .files
            .iter()
            .find(|edit| edit.path == path)
            .unwrap();
        if columns[1] == "000000" {
            if edit.contents.is_some() {
                return Err(anyhow!(
                    "Candidate deletion does not match sealed implementation"
                ));
            }
        } else {
            let bytes = safe_git(&repo, &["show", &format!("{commit}:{path}")], None)?;
            if selected.files.get(&format!("files/{path}")) != Some(&bytes) {
                return Err(anyhow!(
                    "Candidate file does not match sealed evidence; delivery refused"
                ));
            }
        }
        paths.push(path.to_owned());
    }
    let bytes = safe_git(
        &repo,
        &[
            "diff",
            "--binary",
            "--full-index",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            source,
            commit,
            "--",
        ],
        None,
    )?;
    if bytes.is_empty()
        || bytes.len() > 2 * 1024 * 1024
        || contains_secret(&String::from_utf8_lossy(&bytes))
    {
        return Err(anyhow!(
            "Selected patch is empty, oversized or contains a possible credential"
        ));
    }
    Ok((repo, bytes, paths))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPreview {
    pub variant_id: String,
    pub source_commit: String,
    pub candidate_commit: String,
    pub patch_digest: String,
    pub changed_files: Vec<String>,
    pub operation: String,
}

fn apply_preflight(
    store: &Store,
    selected: &SelectedCandidate,
) -> Result<(PathBuf, Vec<u8>, ApplyPreview)> {
    let (repo, bytes, paths) = patch(store, selected)?;
    let config = GrowthConfig::load(&repo)?;
    if config != selected.battle.contract.config {
        return Err(anyhow!(
            "Product configuration changed; selected apply refused"
        ));
    }
    let toplevel = crate::paths::canonicalize(Path::new(&git_text(
        &repo,
        &["rev-parse", "--show-toplevel"],
    )?))?;
    if toplevel != repo
        || git_text(&repo, &["rev-parse", "HEAD^{commit}"])?
            != selected.battle.contract.source_snapshot_commit
    {
        return Err(anyhow!(
            "Product HEAD differs from the frozen baseline; export or create a new battle"
        ));
    }
    // Status can otherwise execute clean/process filters on local files.
    // Fail closed rather than executing repository-configured programs while
    // inspecting whether delivery would preserve the user's checkout.
    let keys = safe_git(&repo, &["config", "--null", "--name-only", "--list"], None)?;
    if keys.split(|byte| *byte == 0).any(|key| {
        let key = String::from_utf8_lossy(key).to_ascii_lowercase();
        key.starts_with("filter.")
            && [".clean", ".smudge", ".process"]
                .iter()
                .any(|suffix| key.ends_with(suffix))
    }) {
        return Err(anyhow!("Selected apply requires a checkout without configured Git content filters; export remains available"));
    }
    if !safe_git(
        &repo,
        &["status", "--porcelain=v1", "-z", "--untracked-files=normal"],
        None,
    )?
    .is_empty()
    {
        return Err(anyhow!(
            "Product checkout has local changes; selected apply requires a clean baseline"
        ));
    }
    let indexed = safe_git(&repo, &["ls-files", "-v", "-z"], None)?;
    if indexed
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .any(|entry| entry[0] != b'H')
    {
        return Err(anyhow!("Product index has sparse or assume-unchanged entries; selected apply requires an ordinary clean checkout"));
    }
    for path in &paths {
        config.check_write(&repo, path)?;
        let tracked = safe_git(
            &repo,
            &[
                "ls-tree",
                "-z",
                &selected.battle.contract.source_snapshot_commit,
                "--",
                path,
            ],
            None,
        )?;
        let target = repo.join(path);
        if tracked.is_empty() {
            if std::fs::symlink_metadata(&target).is_ok() {
                return Err(anyhow!(
                    "Selected addition would replace an existing local file"
                ));
            }
        } else {
            let metadata = std::fs::symlink_metadata(&target)?;
            if !metadata.is_file() || metadata.len() > 4 * 1024 * 1024 {
                return Err(anyhow!(
                    "Selected file is not a bounded regular baseline file"
                ));
            }
            let baseline = safe_git(
                &repo,
                &[
                    "show",
                    &format!("{}:{path}", selected.battle.contract.source_snapshot_commit),
                ],
                None,
            )?;
            if std::fs::read(&target)? != baseline {
                return Err(anyhow!(
                    "Selected file differs from its exact baseline bytes; local work was preserved"
                ));
            }
        }
    }
    // Git applies all hunks atomically by default. Never use --reject, --3way,
    // --index, --cached or --unsafe-paths; HEAD and the index remain unchanged.
    safe_git(
        &repo,
        &["apply", "--check", "--binary", "--whitespace=nowarn", "-"],
        Some(&bytes),
    )?;
    let preview = ApplyPreview {
        variant_id: selected.variant.id.clone(),
        source_commit: selected.battle.contract.source_snapshot_commit.clone(),
        candidate_commit: selected.sealed.run.candidate_commit.clone().unwrap(),
        patch_digest: digest(&bytes),
        changed_files: paths,
        operation:
            "Copy selected patch into the product working tree; no commit, merge, push or deploy."
                .into(),
    };
    Ok((repo, bytes, preview))
}

pub fn preview_apply(store: &Store, variant_id: &str) -> Result<ApplyPreview> {
    Ok(apply_preflight(store, &candidate(store, variant_id)?)?.2)
}

fn finish(
    store: &Store,
    mut receipt: SelectionRecord,
    result: Result<()>,
) -> Result<SelectionRecord> {
    receipt.ended_at = Some(now_ms());
    match result {
        Ok(()) => {
            receipt.status = SelectionStatus::Done;
            if receipt.action == SelectionAction::Apply {
                receipt.decision = Decision::Ship;
            }
        }
        Err(error) => {
            receipt.status = SelectionStatus::Failed;
            receipt.error = Some(redact(&error.to_string()));
            store.finish_growth_selection(&receipt)?;
            return Err(error);
        }
    }
    store.finish_growth_selection(&receipt).map_err(|_| anyhow!("Local delivery completed, but its audit receipt could not finalize; the pending receipt must be inspected"))?;
    Ok(receipt)
}

pub fn apply(store: &Store, variant_id: &str) -> Result<SelectionRecord> {
    let selected = candidate(store, variant_id)?;
    let (repo, bytes, _) = apply_preflight(store, &selected)?;
    let mut receipt = record(&selected, SelectionAction::Apply)?;
    receipt.artifact_digest = Some(digest(&bytes));
    store.create_growth_selection(&receipt)?;
    // Re-check immediately before writes in case another app edited the checkout
    // while persisting the intent. Git's all-hunk check is repeated on apply.
    let result = (|| {
        apply_preflight(store, &selected)?;
        safe_git(
            &repo,
            &["apply", "--binary", "--whitespace=nowarn", "-"],
            Some(&bytes),
        )?;
        Ok(())
    })();
    finish(store, receipt, result)
}

/// Atomic create-only file publication. Hard linking a completed same-directory
/// temporary file refuses an existing destination, including a dangling symlink.
pub fn write_new_file(target: &Path, bytes: &[u8]) -> Result<PathBuf> {
    let absolute = if target.is_absolute() {
        target.to_path_buf()
    } else {
        std::env::current_dir()?.join(target)
    };
    let parent = crate::paths::canonicalize(
        absolute
            .parent()
            .ok_or_else(|| anyhow!("Output requires a parent directory"))?,
    )?;
    let name = absolute
        .file_name()
        .ok_or_else(|| anyhow!("Output requires a file name"))?;
    let target = parent.join(name);
    let temporary = parent.join(format!(".growthlab-output-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::hard_link(&temporary, &target)
            .map_err(|_| anyhow!("Output cannot be installed without overwrite; choose a new file on a filesystem supporting hard links"))?;
        Ok(target)
    })();
    let _ = std::fs::remove_file(temporary);
    result
}

pub fn refuse_product_output(store: &Store, battle: &GrowthBattle, target: &Path) -> Result<()> {
    let project = store
        .get_local_project(&battle.project_id)?
        .ok_or_else(|| anyhow!("Product not found"))?;
    let original = Path::new(&project.repo_path);
    if !original.is_absolute() {
        return Err(anyhow!(
            "Product output boundary requires its recorded absolute path"
        ));
    }
    let root = match crate::paths::canonicalize(original) {
        Ok(root) => root,
        Err(_) if !original.exists() => original.to_path_buf(),
        Err(error) => return Err(error.into()),
    };
    let absolute = if target.is_absolute() {
        target.to_path_buf()
    } else {
        std::env::current_dir()?.join(target)
    };
    let parent = crate::paths::canonicalize(
        absolute
            .parent()
            .ok_or_else(|| anyhow!("Invalid output directory"))?,
    )?;
    if parent.starts_with(&root) || parent.starts_with(store.data_root().join("growth-worktrees")) {
        return Err(anyhow!(
            "Export/report output must be outside product and variant checkouts"
        ));
    }
    Ok(())
}

pub fn export(store: &Store, variant_id: &str, output: &Path) -> Result<SelectionRecord> {
    let selected = candidate(store, variant_id)?;
    refuse_product_output(store, &selected.battle, output)?;
    let (_, bytes, _) = patch(store, &selected)?;
    let mut receipt = record(&selected, SelectionAction::Export)?;
    receipt.artifact_digest = Some(digest(&bytes));
    receipt.output_path = Some(output.to_string_lossy().into_owned());
    store.create_growth_selection(&receipt)?;
    let result = write_new_file(output, &bytes).map(|_| ());
    finish(store, receipt, result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_output_refuses_existing_files_and_dangling_symlinks() {
        let root = std::env::temp_dir().join(format!("growth-output-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let target = root.join("report.html");
        write_new_file(&target, b"first completed report").unwrap();
        assert!(write_new_file(&target, b"replacement").is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"first completed report");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("missing"), root.join("link.html")).unwrap();
            assert!(write_new_file(&root.join("link.html"), b"replacement").is_err());
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert!(std::fs::read_dir(&root).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".growthlab-output-")));
        std::fs::remove_dir_all(root).unwrap();
    }
}
