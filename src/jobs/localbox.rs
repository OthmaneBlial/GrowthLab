//! Local backend — run an experiment as a detached process on this machine.
//!
//! The no-transport twin of `jobs/ssh.rs`: same run-dir layout
//!   run.sh      the launcher (exported env + clone-and-run payload)
//!   log         merged stdout/stderr
//!   pid         the detached process-group leader
//!   exit_code   written when the payload finishes
//! but under the orx data dir (`<data dir>/local-runs/<run_id>/`) instead of a
//! remote `~/.orx/runs/`. A restarted `orx supervise` reattaches purely from
//! that directory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{anyhow, Result};
use crate::jobs::ssh::{sh_quote, JobState};

/// The run's working directory: `<data dir>/local-runs/<run id>`.
pub fn run_dir(run_id: &str) -> PathBuf {
    // Run ids are locally-minted UUIDs; sanitize anyway (same as log_path).
    let safe: String = run_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    crate::store::data_dir().join("local-runs").join(safe)
}

pub struct LocalJobSpec {
    /// Names the run dir `<data dir>/local-runs/<run_id>`.
    pub run_id: String,
    /// The shared clone-and-run payload (`bash` script body).
    pub script: String,
    /// Exported inside run.sh (tokens, synced env) — written owner-only.
    pub env: HashMap<String, String>,
    /// Inherited by the controller without being written into run.sh.
    pub secret_env: HashMap<String, String>,
}

/// Submit the job: write run.sh, launch it detached in its own process group
/// (pid == pgid, so cancel can TERM the whole tree), record the pid. Returns
/// the run dir — the reattach handle stored on the descriptor.
pub fn run_job(spec: &LocalJobSpec) -> Result<PathBuf> {
    let dir = run_dir(&spec.run_id);
    run_job_at(spec, &dir, true)
}

/// Reuse the controller lifecycle with an explicit run directory. Growth
/// validation uses a minimal environment, without synced provider credentials.
pub fn run_job_at(spec: &LocalJobSpec, dir: &Path, inherit_environment: bool) -> Result<PathBuf> {
    run_job_with_timeout(spec, dir, inherit_environment, None)
}

pub fn run_job_with_timeout(
    spec: &LocalJobSpec,
    dir: &Path,
    inherit_environment: bool,
    timeout_seconds: Option<u64>,
) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)
        .map_err(|e| anyhow!("Could not create {}: {}", dir.display(), e))?;
    let env = super::default_python_env(&spec.env);
    let exports: String = env
        .iter()
        .map(|(k, v)| format!("export {}={}", k, sh_quote(v)))
        .collect::<Vec<_>>()
        .join("\n");
    // Same subshell shape as the ssh backend: an `exit`/`set -e` failure inside
    // `( … )` ends the subshell, not run.sh, so exit_code is always written.
    let run_sh = if let Some(timeout_seconds) = timeout_seconds {
        if timeout_seconds == 0 || timeout_seconds > 3600 {
            return Err(anyhow!("Local job timeout must be 1–3600 seconds"));
        }
        // Job control gives payload/watchdog independent process groups. The
        // watchdog survives a stalled or crashed Rust controller and finishes
        // escalation even when the payload's leader exits on TERM.
        format!("#!/usr/bin/env bash\n{exports}\ncd {dir} || exit 97\nset -m\n(\n{script}\n) > log 2>&1 &\npayload_pid=$!\necho $payload_pid > payload_pid\n(\n sleep {timeout_seconds}\n echo 1 > timed_out\n kill -TERM -- -$payload_pid 2>/dev/null || true\n sleep 0.5\n kill -KILL -- -$payload_pid 2>/dev/null || true\n) >/dev/null 2>&1 &\nwatchdog_pid=$!\necho $watchdog_pid > watchdog_pid\nwait $payload_pid\nlocal_job_exit_code=$?\nif [ -e timed_out ]; then\n wait $watchdog_pid 2>/dev/null || true\n local_job_exit_code=124\nelse\n kill -TERM -- -$watchdog_pid 2>/dev/null || true\n wait $watchdog_pid 2>/dev/null || true\nfi\necho $local_job_exit_code > exit_code\n",dir=sh_quote(&crate::local::bash::bash_path(dir)),script=spec.script)
    } else {
        format!(
        "#!/usr/bin/env bash\n{exports}\ncd {dir} || exit 97\n(\n{script}\n) > log 2>&1\necho $? > exit_code\n",
        dir = sh_quote(&crate::local::bash::bash_path(dir)),
        script = spec.script,
    )
    };
    let run_sh_path = dir.join("run.sh");
    std::fs::write(&run_sh_path, run_sh)
        .map_err(|e| anyhow!("Could not write {}: {}", run_sh_path.display(), e))?;
    #[cfg(unix)]
    {
        // run.sh carries exported tokens — keep both it and the dir owner-only.
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        let _ = std::fs::set_permissions(&run_sh_path, std::fs::Permissions::from_mode(0o600));
    }

    #[cfg(unix)]
    let controller_program = if inherit_environment {
        crate::local::bash::program()
    } else {
        std::ffi::OsString::from("/bin/bash")
    };
    #[cfg(not(unix))]
    let controller_program = crate::local::bash::program();
    let mut cmd = std::process::Command::new(controller_program);
    if !inherit_environment {
        cmd.env_clear();
        for key in ["SystemRoot", "COMSPEC", "TEMP", "TMP", "TMPDIR", "LANG"] {
            if let Some(value) = std::env::var_os(key) {
                cmd.env(key, value);
            }
        }
    }
    if let Some(path) =
        crate::local::bash::path_with_toolchain(crate::local::shell_env::search_path())
    {
        cmd.env("PATH", path);
    }
    cmd.arg("run.sh")
        .envs(&spec.secret_env)
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow!("Could not launch the local run: {}", e))?;
    if let Err(error) = std::fs::write(dir.join("pid"), format!("{}\n", child.id())) {
        #[cfg(not(windows))]
        let _ = terminate_group(&child.id().to_string());
        #[cfg(windows)]
        let _ = terminate_tree(&child.id().to_string());
        let _ = child.kill();
        let _ = child.wait();
        return Err(anyhow!("Could not record the run's pid: {error}"));
    }
    // The detached handle remains its directory/PID, but this submitting
    // process still owns the OS child and must reap it when it ends.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(dir.to_path_buf())
}

/// Is the recorded process still alive? `ps` rather than `kill -0`: a zombie
/// (dead but not yet reaped by a still-living spawner) answers `kill -0` yet
/// is not running. No libc dependency; works on macOS and Linux.
#[cfg(not(windows))]
fn pid_alive(pid: &str) -> bool {
    match std::process::Command::new("/bin/ps")
        .args(["-o", "stat=", "-p", pid])
        .stderr(std::process::Stdio::null())
        .output()
    {
        Ok(o) if o.status.success() => {
            let stat = String::from_utf8_lossy(&o.stdout);
            let stat = stat.trim();
            !stat.is_empty() && !stat.starts_with('Z')
        }
        _ => false,
    }
}

/// Windows has no `ps`. A zero-timeout wait, not the exit code, where a real 259 reads as live.
#[cfg(windows)]
fn pid_alive(pid: &str) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    };

    let Ok(pid) = pid.trim().parse::<u32>() else {
        return false;
    };
    // SAFETY: plain syscalls; the handle is closed on every path out.
    unsafe {
        let process = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if process.is_null() {
            return false;
        }
        let waited = WaitForSingleObject(process, 0);
        CloseHandle(process);
        waited == WAIT_TIMEOUT
    }
}

/// The terminal state recorded in exit_code, if any. An empty file is run.sh
/// mid-write (`>` truncates before the code lands) — not terminal yet.
fn exit_code_state(dir: &Path) -> Option<JobState> {
    let raw = std::fs::read_to_string(dir.join("exit_code")).ok()?;
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let code: i32 = raw.parse().unwrap_or(-1);
    Some(if code == 0 {
        JobState {
            stage: "COMPLETED".into(),
            message: None,
        }
    } else {
        JobState {
            stage: "ERROR".into(),
            message: Some(format!("exited with code {code}")),
        }
    })
}

/// Job state in the shared stage vocabulary (see `jobs::stage_to_run_status`).
/// exit_code present -> finished; pid alive -> running; pid dead & no
/// exit_code -> killed/crashed.
pub fn inspect_job(dir: &Path) -> JobState {
    if let Some(state) = exit_code_state(dir) {
        return state;
    }
    match std::fs::read_to_string(dir.join("pid")) {
        Ok(pid) if pid_alive(pid.trim()) => JobState {
            stage: "RUNNING".into(),
            message: None,
        },
        // Dead pid: run.sh may have written exit_code and exited between the
        // check above and the ps probe — re-read before calling it killed.
        Ok(_) => {
            for _ in 0..3 {
                if let Some(state) = exit_code_state(dir) {
                    return state;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            if std::fs::metadata(dir.join("pid"))
                .and_then(|metadata| metadata.modified())
                .and_then(|modified| modified.elapsed().map_err(std::io::Error::other))
                .is_ok_and(|age| age < std::time::Duration::from_secs(1))
            {
                return JobState {
                    stage: "RUNNING".into(),
                    message: None,
                };
            }
            JobState {
                stage: "ERROR".into(),
                message: Some("process died without an exit code (killed?)".into()),
            }
        }
        // pid not written yet — just starting.
        Err(_) => JobState {
            stage: "RUNNING".into(),
            message: None,
        },
    }
}

/// One poll of the log past `skip` lines (the supervisor loops every ~2s).
/// A missing log file just means the payload hasn't printed yet.
pub fn stream_logs(dir: &Path, skip: u64, sink: &mut (dyn FnMut(&str) + Send)) -> Result<u64> {
    let content = match std::fs::read_to_string(dir.join("log")) {
        Ok(c) => c,
        Err(_) => return Ok(skip),
    };
    let mut seen = skip;
    for line in content.lines().skip(skip as usize) {
        seen += 1;
        sink(line);
    }
    Ok(seen)
}

/// TERM the fresh controller process group; on Windows, the process tree.
pub fn cancel_job(dir: &Path) -> Result<()> {
    let pid = std::fs::read_to_string(dir.join("pid"))
        .map_err(|e| anyhow!("Could not read the run's pid: {}", e))?;
    let pid = pid.trim().to_string();
    #[cfg(windows)]
    {
        terminate_tree(&pid)
    }
    #[cfg(not(windows))]
    {
        for name in ["payload_pid", "watchdog_pid"] {
            if let Ok(child) = std::fs::read_to_string(dir.join(name)) {
                terminate_group(child.trim())?;
            }
        }
        terminate_group(&pid)
    }
}

/// `/T` also kills the python the launcher started, as TERMing the group does on unix.
#[cfg(windows)]
fn terminate_tree(pid: &str) -> Result<()> {
    // `/T` fails if any descendant already exited, so success is read from the leader's liveness.
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", pid, "/T", "/F"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    for _ in 0..50 {
        if !pid_alive(pid) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Err(anyhow!("Could not terminate local process tree {pid}"))
}

#[cfg(not(windows))]
fn terminate_group(pid: &str) -> Result<()> {
    let leader: i32 = pid
        .parse()
        .map_err(|_| anyhow!("Invalid local process id"))?;
    if leader <= 0 {
        return Err(anyhow!("Invalid local process id"));
    }
    // Direct OS signals avoid mutable PATH and completion/tool lookup races.
    // ESRCH means this recorded group already ended; other failures propagate.
    if !signal_group(leader, 0)? {
        return Ok(());
    }
    if !signal_group(leader, libc::SIGTERM)? {
        return Ok(());
    }
    // TERM alone leaves children that ignore it running. A controller owns a
    // fresh process group; bound cancellation and kill remaining descendants.
    for _ in 0..25 {
        if !signal_group(leader, 0)? {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    signal_group(leader, libc::SIGKILL)?;
    Ok(())
}

#[cfg(not(windows))]
fn signal_group(leader: i32, signal: i32) -> Result<bool> {
    // SAFETY: terminate_group validates the locally recorded positive leader;
    // negation targets that owned group, not all processes or another PID.
    if unsafe { libc::kill(-leader, signal) } == 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        return Ok(false);
    }
    Err(anyhow!(
        "Could not signal local process group {leader} (signal {signal}): {error}"
    ))
}

/// What "this machine" is, for the Compute settings card: the hardware a
/// `--backend local` run gets. Matters most when the dashboard is reached over
/// port forwarding from a GPU box — the card is how the user sees they're
/// sitting on real GPUs.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalHardware {
    pub hostname: String,
    pub os: &'static str,
    pub arch: &'static str,
    /// CPU brand string on macOS (e.g. "Apple M2 Pro"); NVIDIA-less Linux
    /// boxes just show cores/RAM.
    pub chip: Option<String>,
    pub cpu_count: usize,
    pub mem_bytes: Option<u64>,
    pub gpus: Vec<Gpu>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    pub name: String,
    pub mem_mib: Option<u64>,
}

/// Best-effort hardware probe. Every field degrades independently — a missing
/// `nvidia-smi` (or any probe failure) is an empty GPU list, never an error.
/// Blocking (subprocesses); call via `spawn_blocking` from async handlers.
pub fn hardware_info() -> LocalHardware {
    let cmd = |name: &str, args: &[&str]| -> Option<String> {
        let out = std::process::Command::new(name).args(args).output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let mem_bytes = if cfg!(target_os = "macos") {
        cmd("sysctl", &["-n", "hw.memsize"]).and_then(|s| s.parse().ok())
    } else {
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|raw| {
                // "MemTotal:       32763528 kB"
                raw.lines()
                    .find(|l| l.starts_with("MemTotal:"))?
                    .split_whitespace()
                    .nth(1)?
                    .parse::<u64>()
                    .ok()
                    .map(|kb| kb * 1024)
            })
    };
    LocalHardware {
        hostname: cmd("hostname", &[]).unwrap_or_else(|| "unknown".to_string()),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        chip: if cfg!(target_os = "macos") {
            cmd("sysctl", &["-n", "machdep.cpu.brand_string"])
        } else {
            None
        },
        cpu_count: std::thread::available_parallelism().map_or(0, |n| n.get()),
        mem_bytes,
        gpus: cmd(
            "nvidia-smi",
            &[
                "--query-gpu=name,memory.total",
                "--format=csv,noheader,nounits",
            ],
        )
        .map(|out| parse_nvidia_smi_csv(&out))
        .unwrap_or_default(),
    }
}

/// Parse `nvidia-smi --query-gpu=name,memory.total --format=csv,noheader,nounits`:
/// one `name, mem_mib` line per GPU. Names may contain no commas in this
/// format (nvidia-smi separates fields with ", "), but tolerate odd lines by
/// keeping the name and dropping the memory rather than dropping the GPU.
fn parse_nvidia_smi_csv(out: &str) -> Vec<Gpu> {
    out.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| match line.rsplit_once(',') {
            Some((name, mem)) => Gpu {
                name: name.trim().to_string(),
                mem_mib: mem.trim().parse().ok(),
            },
            None => Gpu {
                name: line.trim().to_string(),
                mem_mib: None,
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvidia_smi_csv_two_gpus() {
        let parsed =
            parse_nvidia_smi_csv("NVIDIA A100-SXM4-80GB, 81920\nNVIDIA A100-SXM4-80GB, 81920\n");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "NVIDIA A100-SXM4-80GB");
        assert_eq!(parsed[0].mem_mib, Some(81920));
    }

    #[test]
    fn nvidia_smi_csv_empty_and_malformed() {
        assert!(parse_nvidia_smi_csv("").is_empty());
        assert!(parse_nvidia_smi_csv("\n  \n").is_empty());
        let odd = parse_nvidia_smi_csv("Tesla T4");
        assert_eq!(odd.len(), 1, "GPU kept even without a memory field");
        assert_eq!(odd[0].name, "Tesla T4");
        assert_eq!(odd[0].mem_mib, None);
    }

    fn wait_terminal(dir: &Path) -> JobState {
        let mut state = inspect_job(dir);
        for _ in 0..100 {
            if state.stage != "RUNNING" {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
            state = inspect_job(dir);
        }
        state
    }

    #[cfg(unix)]
    #[test]
    fn cancellation_does_not_require_external_tools_on_path() {
        const MARKER: &str = "GROWTHLAB_OWNED_CANCEL_PATH_FIXTURE";
        struct OwnedFixture {
            root: PathBuf,
            group: Option<i32>,
        }
        impl Drop for OwnedFixture {
            fn drop(&mut self) {
                if let Some(group) = self.group {
                    // SAFETY: a fresh controller group created by this fixture;
                    // cleanup is needed if the old cancellation path fails.
                    let _ = unsafe { libc::kill(-group, libc::SIGKILL) };
                }
                let _ = std::fs::remove_dir_all(&self.root);
            }
        }
        let Some(root) = std::env::var_os(MARKER) else {
            let fixture = OwnedFixture {
                root: std::env::temp_dir()
                    .join(format!("growth-owned-cancel-path-{}", uuid::Uuid::new_v4())),
                group: None,
            };
            std::fs::create_dir_all(fixture.root.join("empty-path")).unwrap();
            // The subprocess keeps PATH changes away from concurrent tests and
            // receives no inherited credentials or user database/config paths.
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "jobs::localbox::tests::cancellation_does_not_require_external_tools_on_path",
                    "--nocapture",
                ])
                .env_clear()
                .env(MARKER, &fixture.root)
                .env("PATH", fixture.root.join("empty-path"))
                .env("GROWTHLAB_DATA_DIR", fixture.root.join("lab"))
                .env("ORX_DATA_DIR", fixture.root.join("lab"))
                .env("ORX_CACHE_DIR", fixture.root.join("cache"))
                .env("XDG_CONFIG_HOME", fixture.root.join("config"))
                .stdin(std::process::Stdio::null())
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            return;
        };
        let mut fixture = OwnedFixture {
            root: root.into(),
            group: None,
        };
        let directory = fixture.root.join("job");
        run_job_at(
            &LocalJobSpec {
                run_id: "owned-cancel-fixture".into(),
                script: "/bin/sleep 60".into(),
                env: HashMap::from([("PATH".into(), "/usr/bin:/bin:/usr/sbin:/sbin".into())]),
                secret_env: HashMap::new(),
            },
            &directory,
            false,
        )
        .unwrap();
        let group: i32 = std::fs::read_to_string(directory.join("pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(group > 0);
        fixture.group = Some(group);
        // SAFETY: inspect only this fixture's freshly registered group.
        assert_eq!(unsafe { libc::kill(-group, 0) }, 0);
        cancel_job(&directory).unwrap();
        fixture.group = None;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while unsafe { libc::kill(-group, 0) } == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // SAFETY: inspect only the fixture's recorded group after cancellation.
        assert_eq!(unsafe { libc::kill(-group, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }

    #[test]
    fn local_job_lifecycle() {
        // The only test that touches ORX_DATA_DIR, so the global env is safe.
        let base = std::env::temp_dir().join(format!("orx-localbox-test-{}", std::process::id()));
        std::env::set_var("ORX_DATA_DIR", &base);

        let dir = run_job(&LocalJobSpec {
            run_id: "lifecycle".into(),
            script: "[ -n \"$TINKER_API_KEY\" ] && echo hello-$ORX_TEST_VAR".into(),
            env: HashMap::from([("ORX_TEST_VAR".to_string(), "42".to_string())]),
            secret_env: HashMap::from([("TINKER_API_KEY".to_string(), "s3cr3t-value".to_string())]),
        })
        .unwrap();
        let state = wait_terminal(&dir);
        assert_eq!(state.stage, "COMPLETED", "message: {:?}", state.message);
        // Python is defaulted to unbuffered so tailed-`log` output streams live.
        let run_sh = std::fs::read_to_string(dir.join("run.sh")).unwrap();
        assert!(run_sh.contains("export PYTHONUNBUFFERED='1'\n"));
        assert!(!run_sh.contains("s3cr3t-value"));

        let mut lines = Vec::new();
        let seen = stream_logs(&dir, 0, &mut |l| lines.push(l.to_string())).unwrap();
        assert_eq!(seen, 1);
        assert_eq!(lines, ["hello-42"]);
        // Re-poll past the consumed lines: nothing new.
        assert_eq!(stream_logs(&dir, seen, &mut |_| ()).unwrap(), seen);

        let failed = run_job(&LocalJobSpec {
            run_id: "failing".into(),
            script: "exit 3".into(),
            env: HashMap::new(),
            secret_env: HashMap::new(),
        })
        .unwrap();
        let state = wait_terminal(&failed);
        assert_eq!(state.stage, "ERROR");
        assert_eq!(state.message.as_deref(), Some("exited with code 3"));

        let cancelled = run_job(&LocalJobSpec {
            run_id: "cancelled".into(),
            script: "sleep 60".into(),
            env: HashMap::new(),
            secret_env: HashMap::new(),
        })
        .unwrap();
        assert_eq!(inspect_job(&cancelled).stage, "RUNNING");
        cancel_job(&cancelled).unwrap();
        let state = wait_terminal(&cancelled);
        // TERM leaves either a dead pid with no exit_code, or a non-zero
        // exit_code if run.sh got to write one — ERROR either way.
        assert_eq!(state.stage, "ERROR");

        std::env::remove_var("ORX_DATA_DIR");
        let _ = std::fs::remove_dir_all(&base);
    }
}
