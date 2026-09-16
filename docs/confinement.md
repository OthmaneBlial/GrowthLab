# Validation isolation

Configured validation executes candidate code. GrowthLab requires an operating
system isolation driver; it never falls back to an unrestricted command.
The driver runs after the trusted local supervisor stages the recorded source
archive. The command receives that snapshot, private scratch space and approved
system/tool runtime files. It receives no provider credentials or inherited shell
configuration; the supervisor retains basic operating-system locale/temporary
settings, with the command's scratch and runtime paths supplied explicitly.
This boundary covers validation commands, not native provider
requests or all inherited OpenResearch execution paths.

## Host boundary

The candidate can write its isolated source copy and validation scratch. It
cannot use this permission to modify the product checkout, another attempt,
supervisor PID/exit files, SQLite records or evidence archives. The original
product and GrowthLab data root are expressly excluded from runtime permissions.
The supervisor and independent timeout watchdog stay outside this boundary.
The command emits stdout/stderr through a pipe; a trusted relay captures those
bytes outside the jail. The command is not granted access to the captured log file.

Runtime directories such as system binaries/libraries and Homebrew tool prefixes
are readable and not writable. Home directories, credential stores, general
temporary directories and host configuration directories are not granted. These
approved runtime directories are trusted inputs; they must not contain private
application data. The full concrete policy is captured privately for inspection.
On macOS, exact ancestor directories also permit traversal metadata so runtimes
can resolve snapshot/tool paths component by component. Their directory contents,
extended attributes and writes remain denied.
The literal filesystem root also remains readable for dyld process bootstrap;
this exception grants no descendant paths.

Validation does not download dependencies. Untracked `node_modules`, home-based
toolchains and package caches are not copied from the product or exposed through
host access. Commands needing them may fail until a bounded dependency-staging
workflow exists. `OPENSSL_CONF=/dev/null` avoids loading host OpenSSL configuration.
The private scratch directory supplies `TMPDIR`.

## Platform support

- **macOS:** `/usr/bin/sandbox-exec` applies a deny-by-default profile to the
  command and its children. Network operations are denied, including access to
  host localhost services. Only specific CPU, page-size and OS/uname sysctl
  reads are granted, including the hostname required by Node's OS module;
  host process argument/environment reads are not. Process-information operations
  are explicitly denied with a self-only exception, because a sysctl whitelist
  alone does not block argument reads on the tested host. Signalling
  host processes is denied. Apple's installed `sandbox-exec(1)` manual marks
  this interface deprecated. This source-alpha backend requires current-host
  checks and does not promise future macOS compatibility.
- **Linux:** `/usr/bin/bwrap` is required, with permitted unprivileged user and
  network namespaces. Install a current Bubblewrap through your distribution's
  package manager. The implementation creates filesystem, PID and network
  namespaces, drops capabilities, prevents nested user namespaces and exposes
  only the snapshot, scratch and read-only runtime mounts. Its new root, proc
  and device mounts are remounted read-only; standard device IO remains available.
  The explicit
  `--unshare-user` avoids the optional behavior of `--unshare-all`; unsupported
  flags or denied namespace creation cause failure. See the
  [official Bubblewrap options](https://github.com/containers/bubblewrap/blob/main/bwrap.xml)
  and [security guidance](https://github.com/containers/bubblewrap#security).
  Linux runtime isolation is **unverified** on the current macOS development host.
- **Windows:** validation isolation is **not implemented**. Validation is refused;
  inherited Windows foundations do not establish GrowthLab battle support.

A missing driver is refused before claiming a ready battle or requesting a
proposal. Driver presence alone does not establish working kernel permissions:
runtime setup failures remain failed checks. No Docker or virtual machine is
required by the implementation.

## Evidence and recovery

Before a command starts, its active-validation checkpoint includes the backend
and policy SHA-256. Exact policy bytes are stored as
`validation-<index>.policy.json` in the private checkpoint and final archive.
Policies contain private absolute paths and, on Linux, the command; public reports
show only a recognized backend label and digest. Recovery verifies captured policy
bytes before observing an active job, then retains the original confinement
metadata with the actual exit and log. It never reruns the command.

Older archived checks omit this optional metadata and retain their original
serialized bytes. Their host isolation is **unverified**. Report generation does
not grant them isolation retroactively.

## Limits and local verification

The timeout and bounded captured logs remain enforced. CPU, memory and disk quotas
are not provided. This is not a virtual-machine boundary, kernel security
attestation or proof of growth lift. Native-agent runtime verification, the rare
unregistered-launch interruption window and interrupted selected-delivery recovery
remain separate gates.

The native isolation test uses only synthetic host files, an owned process with
a cleared synthetic environment, and an owned localhost listener. It checks
allowed snapshot/scratch writes and refused host reads/writes, metadata writes,
symlink/hardlink escapes, child access, process argument queries on macOS, host
signals and network connections. It also refuses changed policy bytes.

```sh
cargo test --locked --bin growthlab growth::confinement_tests
cargo build --locked
python3 scripts/test-growth-battle.py target/debug/growthlab
python3 scripts/test-growth-recovery.py target/debug/growthlab
```

Run checks locally. GitHub Actions remains disabled at the user's request.
