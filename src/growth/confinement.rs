//! Host isolation for untrusted configured validation commands.
//! The existing supervisor stages the snapshot and owns logs/PIDs outside the jail.
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{anyhow, Result};
use crate::jobs::ssh::sh_quote;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfinementRecord {
    pub backend: String,
    pub policy_digest: String,
    pub network: String,
    pub writable: String,
    pub limitation: String,
}

pub struct ConfinedCommand {
    pub script: String,
    pub environment: HashMap<String, String>,
    pub record: ConfinementRecord,
    pub policy: Vec<u8>,
}

pub fn available() -> Result<()> {
    #[cfg(target_os = "macos")]
    if Path::new("/usr/bin/sandbox-exec").is_file() {
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    if Path::new("/usr/bin/bwrap").is_file() {
        return Ok(());
    }
    Err(anyhow!(
        "Validation isolation unavailable: macOS requires /usr/bin/sandbox-exec; Linux requires /usr/bin/bwrap and permitted unprivileged namespaces. Windows validation isolation is not implemented. Commands are never run without isolation."
    ))
}

fn runtime_roots() -> Vec<PathBuf> {
    let roots = [
        "/bin",
        "/sbin",
        "/lib",
        "/lib64",
        "/usr/bin",
        "/usr/sbin",
        "/usr/lib",
        "/usr/lib64",
        "/usr/share",
        "/usr/libexec",
        "/usr/include",
        "/usr/local/bin",
        "/usr/local/lib",
        "/usr/local/share",
        "/usr/local/Cellar",
        "/usr/local/opt",
        "/System/Library",
        "/Library/Apple/System/Library",
        "/opt/homebrew/bin",
        "/opt/homebrew/Cellar",
        "/opt/homebrew/opt",
        "/opt/homebrew/lib",
        "/opt/homebrew/share",
    ];
    roots
        .iter()
        .map(PathBuf::from)
        .filter(|root| root.is_dir())
        .collect()
}

fn string(path: &Path) -> Result<String> {
    Ok(serde_json::to_string(path.to_str().ok_or_else(|| {
        anyhow!("Isolation paths must be UTF-8")
    })?)?)
}

fn macos_profile(repo: &Path, scratch: &Path, private_roots: &[PathBuf]) -> Result<String> {
    let runtime = runtime_roots();
    let mut ancestors = BTreeSet::new();
    for root in runtime.iter().map(PathBuf::as_path).chain([repo, scratch]) {
        for ancestor in root.ancestors().skip(1) {
            ancestors.insert(ancestor.to_path_buf());
        }
    }
    let mut profile = String::from(
        "(version 1)\n(deny default)\n(deny process-info*)\n(allow process-info* (target self))\n(allow process-fork process-exec)\n(allow signal (target self))\n(allow file-read* (literal \"/\") (literal \"/dev/null\") (literal \"/dev/urandom\") (literal \"/dev/random\")",
    );
    for root in runtime {
        profile.push_str(&format!(" (subpath {})", string(&root)?));
    }
    profile.push_str(&format!(" (subpath {}) (subpath {}))\n(allow file-write* (literal \"/dev/null\") (subpath {}) (subpath {}))\n", string(repo)?, string(scratch)?, string(repo)?, string(scratch)?));
    // Runtime sizing and CPU/OS features only. In particular, never grant
    // kern.proc* queries that could disclose another process's arguments/env.
    profile.push_str("(allow sysctl-read");
    for key in [
        "hw.pagesize",
        "hw.pagesize_compat",
        "hw.ncpu",
        "hw.activecpu",
        "hw.logicalcpu",
        "hw.logicalcpu_max",
        "hw.physicalcpu",
        "hw.physicalcpu_max",
        "hw.memsize",
        "hw.machine",
        "hw.ephemeral_storage",
        "hw.optional.armv8_2_sha512",
        "hw.optional.armv8_2_sha3",
        "kern.osrelease",
        "kern.ostype",
        "kern.osversion",
        "kern.version",
        "kern.hostname",
        "kern.osproductversion",
        "kern.boottime",
        "kern.argmax",
        "kern.maxfilesperproc",
    ] {
        profile.push_str(&format!(" (sysctl-name \"{key}\")"));
    }
    profile.push_str(")\n");
    // Node and other runtimes resolve paths one component at a time. Permit
    // only metadata on exact ancestors, never their contents, xattrs or writes.
    profile.push_str("(allow file-read-metadata");
    for ancestor in &ancestors {
        profile.push_str(&format!(" (literal {})", string(ancestor)?));
    }
    profile.push_str(")\n(deny file-read-data file-read-xattr file-write*");
    for ancestor in &ancestors {
        // dyld opens the literal filesystem root during process bootstrap.
        // Keep that existing exception; it grants no descendants.
        if ancestor == Path::new("/") {
            continue;
        }
        profile.push_str(&format!(" (literal {})", string(ancestor)?));
    }
    profile.push_str(")\n");
    // Approved tool prefixes must never expose the original product or lab data,
    // even when a user places them inside a tool prefix.
    for root in private_roots {
        profile.push_str(&format!("(deny file-read* file-write* (require-all (subpath {}) (require-not (subpath {})) (require-not (subpath {}))", string(root)?, string(repo)?, string(scratch)?));
        for ancestor in &ancestors {
            profile.push_str(&format!(" (require-not (literal {}))", string(ancestor)?));
        }
        profile.push_str("))\n");
    }
    Ok(profile)
}

fn linux_arguments(repo: &Path, scratch: &Path, private_roots: &[PathBuf]) -> Result<Vec<String>> {
    let mut args = vec![
        "--unshare-all",
        "--unshare-user",
        "--disable-userns",
        "--die-with-parent",
        "--new-session",
        "--cap-drop",
        "ALL",
        "--clearenv",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    for root in runtime_roots() {
        // /bin and /lib may be symlinks on merged-/usr systems. Canonical
        // sources are bound to the expected runtime path in the empty namespace.
        let canonical = crate::paths::canonicalize(&root)?;
        if private_roots
            .iter()
            .any(|private| canonical.starts_with(private) || private.starts_with(&canonical))
        {
            continue;
        }
        args.extend([
            "--ro-bind".into(),
            canonical.to_string_lossy().into_owned(),
            root.to_string_lossy().into_owned(),
        ]);
    }
    args.extend([
        "--proc".into(),
        "/proc".into(),
        "--dev".into(),
        "/dev".into(),
        "--bind".into(),
        repo.to_string_lossy().into_owned(),
        "/workspace".into(),
        "--bind".into(),
        scratch.to_string_lossy().into_owned(),
        "/tmp".into(),
        "--remount-ro".into(),
        "/proc".into(),
        "--remount-ro".into(),
        "/dev".into(),
        "--remount-ro".into(),
        "/".into(),
        "--chdir".into(),
        "/workspace".into(),
        "--setenv".into(),
        "PATH".into(),
        "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".into(),
        "--setenv".into(),
        "TMPDIR".into(),
        "/tmp".into(),
        "--setenv".into(),
        "OPENSSL_CONF".into(),
        "/dev/null".into(),
    ]);
    Ok(args)
}

pub fn command(
    directory: &Path,
    archive: &Path,
    command: &str,
    private_roots: &[PathBuf],
) -> Result<ConfinedCommand> {
    available()?;
    let parent = directory
        .parent()
        .ok_or_else(|| anyhow!("Validation directory requires a parent"))?;
    let name = directory
        .file_name()
        .ok_or_else(|| anyhow!("Validation directory requires an ID"))?;
    // Construct the physical path without creating the job parent before its
    // active-validation checkpoint has persisted. Only that one known missing
    // component is permitted; the data root must already exist.
    let parent = if parent.exists() {
        crate::paths::canonicalize(parent)?
    } else {
        if parent.file_name() != Some(std::ffi::OsStr::new("growth-jobs")) {
            return Err(anyhow!("Validation parent must exist or be growth-jobs"));
        }
        crate::paths::canonicalize(
            parent
                .parent()
                .ok_or_else(|| anyhow!("Validation parent requires a data root"))?,
        )?
        .join("growth-jobs")
    };
    let directory = parent.join(name);
    let repo = directory.join("repo");
    let scratch = directory.join("validation-tmp");
    let private_roots = private_roots
        .iter()
        .map(crate::paths::canonicalize)
        .collect::<std::io::Result<Vec<_>>>()?;
    let (backend, network, policy, invocation) = if cfg!(target_os = "macos") {
        let profile = macos_profile(&repo, &scratch, &private_roots)?;
        let invocation = format!(
            "/usr/bin/sandbox-exec -p {} /bin/bash --noprofile --norc -c {}",
            sh_quote(&profile),
            sh_quote(command)
        );
        (
            "macos-seatbelt-v1",
            "denied",
            serde_json::to_vec(
                &serde_json::json!({"version":1,"backend":"macos-seatbelt-v1","profile":profile}),
            )?,
            invocation,
        )
    } else {
        let mut arguments = linux_arguments(&repo, &scratch, &private_roots)?;
        arguments.extend([
            "--".into(),
            "/bin/bash".into(),
            "--noprofile".into(),
            "--norc".into(),
            "-c".into(),
            command.into(),
        ]);
        let invocation = format!(
            "/usr/bin/bwrap {}",
            arguments
                .iter()
                .map(|argument| sh_quote(argument))
                .collect::<Vec<_>>()
                .join(" ")
        );
        (
            "linux-bubblewrap-v1",
            "host network isolated",
            serde_json::to_vec(
                &serde_json::json!({"version":1,"backend":"linux-bubblewrap-v1","arguments":arguments}),
            )?,
            invocation,
        )
    };
    let digest = format!("{:x}", Sha256::digest(&policy));
    let mut environment = HashMap::from([
        ("OPENSSL_CONF".into(), "/dev/null".into()),
        ("TMPDIR".into(), scratch.to_string_lossy().into_owned()),
    ]);
    environment.insert(
        "PATH".into(),
        if cfg!(target_os = "macos") {
            "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
        } else {
            "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
        }
        .into(),
    );
    Ok(ConfinedCommand {
        // Keep supervisor files outside the jail, including stdio metadata.
        // The trusted cat relays bytes from a pipe to the private captured log;
        // pipefail preserves the confined command's nonzero exit status.
        script: format!("set -eo pipefail; /bin/mkdir -p repo validation-tmp; /usr/bin/tar -xf {} -C repo; cd repo; {} 2>&1 | /bin/cat", sh_quote(&crate::local::bash::bash_path(archive)), invocation),
        environment, policy,
        record: ConfinementRecord { backend:backend.into(), policy_digest:digest, network:network.into(), writable:"isolated source snapshot and private validation scratch only".into(), limitation:"Configured command isolation does not prove growth outcomes or provide CPU/memory/disk quotas. Only approved system/tool runtime roots are readable; dependencies and home-installed toolchains may require additional preparation.".into() },
    })
}

pub fn verify_record(record: &ConfinementRecord, policy: &[u8]) -> Result<()> {
    if format!("{:x}", Sha256::digest(policy)) != record.policy_digest {
        return Err(anyhow!(
            "Validation confinement policy digest does not match captured bytes"
        ));
    }
    Ok(())
}
