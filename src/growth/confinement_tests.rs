//! Real native isolation checks; all host targets and processes are synthetic.
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use super::archive;
use super::confinement;
use crate::jobs::localbox;

struct Probe {
    root: PathBuf,
    job: PathBuf,
    sentinel: Child,
}

impl Drop for Probe {
    fn drop(&mut self) {
        if localbox::inspect_job(&self.job).stage == "RUNNING" {
            let _ = localbox::cancel_job(&self.job);
        }
        let _ = self.sentinel.kill();
        let _ = self.sentinel.wait();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn real_command_blocks_host_data_metadata_symlink_child_network_and_signal_access() {
    let root =
        std::env::temp_dir().join(format!("growth-isolation-fixture-{}", uuid::Uuid::new_v4()));
    archive::private_directory(&root).unwrap();
    let root = crate::paths::canonicalize(&root).unwrap();
    let product = root.join("product");
    let lab = root.join("lab");
    let sources = root.join("sources");
    for directory in [&product, &lab, &sources, &lab.join("growth-jobs")] {
        archive::private_directory(directory).unwrap();
    }
    let private = product.join(".env");
    std::fs::write(&private, "Synthetic private fixture").unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let job = lab.join("growth-jobs").join(&id);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let sentinel = Command::new("/bin/sleep")
        .arg("15")
        .env_clear()
        .env("GROWTHLAB_SYNTHETIC_SENTINEL", "synthetic-private-value")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut probe = Probe {
        root: root.clone(),
        job: job.clone(),
        sentinel,
    };
    let input = serde_json::json!({"private":private,"pidFile":job.join("pid"),"port":listener.local_addr().unwrap().port(),"sentinel":probe.sentinel.id()});
    #[cfg(target_os = "macos")]
    {
        // Read only the owned process with a synthetic, cleared environment.
        // CTL_KERN=1 and KERN_PROCARGS2=49 are the macOS sysctl MIB.
        let python = r#"import ctypes,errno,os,sys
library=ctypes.CDLL(None,use_errno=True)
mib=(ctypes.c_int*3)(1,49,int(sys.argv[1]))
size=ctypes.c_size_t(262144)
buffer=ctypes.create_string_buffer(size.value)
ctypes.set_errno(0)
result=library.sysctl(mib,3,buffer,ctypes.byref(size),None,0)
error=ctypes.get_errno()
if sys.argv[2]=='host':
 assert result==0
 assert b'/bin/sleep' in buffer.raw[:size.value]
else:
 assert result==-1 and error in [errno.EPERM,errno.EACCES], (result,error)
 mib=(ctypes.c_int*3)(1,49,os.getpid())
 size=ctypes.c_size_t(len(buffer))
 assert library.sysctl(mib,3,buffer,ctypes.byref(size),None,0)==0
 assert b'procargs.py' in buffer.raw[:size.value]
 print('host args blocked')
"#;
        let path = sources.join("procargs.py");
        std::fs::write(&path, python).unwrap();
        assert!(
            Command::new("python3")
                .arg(&path)
                .arg(probe.sentinel.id().to_string())
                .arg("host")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success(),
            "owned synthetic process arguments must be readable before confinement"
        );
    }
    let script = format!(
        r#"
const fs=require('node:fs'), cp=require('node:child_process'), net=require('node:net'), assert=require('node:assert/strict');
const input={input};
function denied(action) {{ assert.throws(action, error => ['EPERM','EACCES','ENOENT','ESRCH'].includes(error.code)); }}
fs.writeFileSync('inside.txt','isolated write');
fs.writeFileSync(require('node:path').join(require('node:os').tmpdir(),'scratch.txt'),'private scratch');
denied(()=>fs.readFileSync(input.private));
denied(()=>fs.writeFileSync(input.private,'synthetic escape'));
denied(()=>fs.writeFileSync(input.pidFile,'synthetic-denied-metadata'));
denied(()=>fs.readFileSync(require('node:path').join(require('node:path').dirname(input.pidFile),'log')));
denied(()=>fs.readdirSync(require('node:path').dirname(require('node:path').dirname(input.pidFile))));
fs.symlinkSync(input.private,'outside-link');
denied(()=>fs.readFileSync('outside-link'));
denied(()=>fs.linkSync(input.private,'outside-hardlink'));
const childProgram='const fs=require("node:fs");try{{fs.readFileSync('+JSON.stringify(input.private)+');process.exit(2)}}catch(e){{if(!["EPERM","EACCES","ENOENT"].includes(e.code))process.exit(3)}}console.log("child confined")';
const child=cp.spawnSync(process.execPath,['-e',childProgram],{{encoding:'utf8'}});
assert.equal(child.status,0);assert.equal(child.stdout.trim(),'child confined');
if(process.platform==='darwin') {{
 const args=cp.spawnSync('python3',['procargs.py',String(input.sentinel),'confined'],{{encoding:'utf8'}});
 assert.equal(args.status,0,args.stderr);assert.equal(args.stdout.trim(),'host args blocked');
}} else {{
 denied(()=>fs.readFileSync('/proc/'+String(input.sentinel)+'/environ'));
}}
denied(()=>process.kill(input.sentinel,'SIGTERM'));
const timer=setTimeout(()=>process.exit(5),2500);
const socket=net.connect(input.port,'127.0.0.1');
socket.on('connect',()=>process.exit(6));
socket.on('error',()=>{{clearTimeout(timer);socket.destroy();console.log('confined probe passed')}});
"#
    );
    std::fs::write(sources.join("probe.cjs"), script).unwrap();
    let tar = root.join("source.tar");
    assert!(Command::new("/usr/bin/tar")
        .arg("-cf")
        .arg(&tar)
        .arg("-C")
        .arg(&sources)
        .arg(".")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success());
    let confined = confinement::command(&job, &tar, "node probe.cjs", &[product, lab]).unwrap();
    confinement::verify_record(&confined.record, &confined.policy).unwrap();
    let mut tampered = confined.policy.clone();
    tampered.push(b' ');
    assert!(confinement::verify_record(&confined.record, &tampered).is_err());
    localbox::run_job_with_timeout(
        &localbox::LocalJobSpec {
            run_id: id,
            script: confined.script,
            env: confined.environment,
            secret_env: Default::default(),
        },
        &job,
        false,
        Some(8),
    )
    .unwrap();
    let started = Instant::now();
    while !job.join("exit_code").exists() && started.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let log = std::fs::read_to_string(job.join("log")).unwrap();
    assert_eq!(
        std::fs::read_to_string(job.join("exit_code"))
            .unwrap()
            .trim(),
        "0",
        "{log}"
    );
    assert!(log.contains("confined probe passed"), "{log}");
    assert_eq!(
        std::fs::read_to_string(private).unwrap(),
        "Synthetic private fixture"
    );
    assert_eq!(
        std::fs::read_to_string(job.join("repo/inside.txt")).unwrap(),
        "isolated write"
    );
    assert_eq!(
        std::fs::read_to_string(job.join("validation-tmp/scratch.txt")).unwrap(),
        "private scratch"
    );
    assert!(
        probe.sentinel.try_wait().unwrap().is_none(),
        "validator must not signal a host process"
    );
}
