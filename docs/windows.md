# Windows

Windows support is a progressive target for GrowthLab. The source tree contains
Windows-specific process, filesystem and browser handling, and a local
cross-compiled `x86_64-pc-windows-gnu` package can be produced. This repository
does not claim a Windows runtime pass from a macOS host; use the limitations
below when evaluating a build.

## Prerequisites

**Git for Windows is required**, and for more than Git. It supplies the Bash and
coreutils used by local validation; the `bash.exe` in `System32` is the WSL
launcher and is not a substitute. Install the standard Git for Windows package
so `git.exe` and `bash.exe` are on `PATH`.

Native agent execution is optional. If you enable it locally, install only the
harnesses you have explicitly configured. The bundled replay demo needs no
provider account.

For a source checkout that also builds the dashboard assets, install Node.js;
the agent packages below are optional examples, not GrowthLab requirements.

```powershell
winget install --id Git.Git -e
winget install --id OpenJS.NodeJS.LTS -e
```

Open a new terminal afterwards so `PATH` is picked up.

## Build from source

The current public alpha release does not include a Windows runtime-validated
asset. Build the canonical `growthlab.exe` locally from PowerShell instead:

```powershell
winget install --id Git.Git -e
winget install --id Rustlang.Rustup -e
git clone https://github.com/OthmaneBlial/GrowthLab.git
cd GrowthLab
cargo build --release --locked --bin growthlab
.\target\release\growthlab.exe --no-telemetry version
```

The compatibility `orx.exe` entry point remains available where inherited
scripts still use it. For a local bundled replay without an agent key:

```powershell
.\target\release\growthlab.exe demo --no-browser
```

The dashboard URL is printed by the command. Keep the PowerShell window open
while the local server is running.

To inspect a cross-compiled package from macOS or Linux, use the repository's
packager with `x86_64-pc-windows-gnu`; it creates a deterministic ZIP and the
offline verifier checks its contents, notices and checksum. That package is
structure evidence only until a Windows machine runs it.

### The SmartScreen warning

Unsigned local executables may trigger the Windows SmartScreen warning. Inspect
the checksum and source first; a warning is expected for an unsigned build.

Signing is planned, but it will not make this go away immediately: since 2024
even an EV certificate has to earn SmartScreen reputation through download
volume like any other, so early builds will keep showing the warning.

### Long paths

Windows refuses paths over 260 characters. GrowthLab passes `core.longpaths` to
Git itself, but a deep repository can still defeat the agent or your experiment
scripts. If you hit "path too long" from something that is not git, enable long
paths system-wide — once, as administrator, then reboot:

```powershell
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' -Name LongPathsEnabled -Value 1
```

## Known gaps

| | |
|---|---|
| `growthlab up --remote-host` | Refused. The inherited control channel uses a Unix domain socket. |
| Validation isolation | Windows isolation is not implemented; checks that require the confinement backend remain explicitly unavailable. |
| Native provider execution | Installed harnesses and credentials are not proof of a successful provider run; verify each provider on Windows before enabling it. |
| Release runtime | No Windows runtime or installer result is claimed from this macOS development host. |
| Data directory | The Windows path follows the platform-specific GrowthLab resolver; inspect `growthlab config` output before sharing it. |
