# Distribution

GrowthLab ships source first. Release files must carry the same provenance as
the product: a user can inspect the executable, its checksum, the MIT notice,
the OpenResearch attribution, the demo guide, and the dependency notices
without contacting an analytics or agent provider.

## Current source line and binary evidence

`v0.1.0-alpha.20` includes one manually validated executable archive and four
cross-target archives whose structure was verified locally. The previous alpha.19
archive remains available in its release history:

| Target | Asset | Evidence |
| --- | --- | --- |
| macOS arm64 | [`growthlab-v0.1.0-alpha.20-macos-arm64.tar.gz`](https://github.com/OthmaneBlial/GrowthLab/releases/download/v0.1.0-alpha.20/growthlab-v0.1.0-alpha.20-macos-arm64.tar.gz) | Extracted, checksum-checked, and run locally with `--no-telemetry version`; `growthlab 0.1.0-alpha.20` reported the development build channel. |
| macOS x86_64 | [`growthlab-v0.1.0-alpha.20-macos-x86_64.tar.gz`](https://github.com/OthmaneBlial/GrowthLab/releases/download/v0.1.0-alpha.20/growthlab-v0.1.0-alpha.20-macos-x86_64.tar.gz) | Built from the exact alpha.20 tag with Cargo; checksum, traversal/link safety, required notices and archive structure passed the offline verifier. Its version command also passed under Rosetta on macOS arm64. The current source x86_64-apple-darwin suite passes under Rosetta after the sandbox admits the Apple runtime read-only; native x86_64 hardware was not tested. |
| Linux x86_64 (musl) | [`growthlab-v0.1.0-alpha.20-linux-x86_64-musl.tar.gz`](https://github.com/OthmaneBlial/GrowthLab/releases/download/v0.1.0-alpha.20/growthlab-v0.1.0-alpha.20-linux-x86_64-musl.tar.gz) | Built from the exact alpha.20 tag with `cargo-zigbuild`; checksum, traversal/link safety, required notices and archive structure passed the offline verifier. The ELF was not run on Linux here. |
| Linux arm64 (musl) | [`growthlab-v0.1.0-alpha.20-linux-arm64-musl.tar.gz`](https://github.com/OthmaneBlial/GrowthLab/releases/download/v0.1.0-alpha.20/growthlab-v0.1.0-alpha.20-linux-arm64-musl.tar.gz) | Built from the exact alpha.20 tag with `cargo-zigbuild`; checksum, traversal/link safety, required notices and archive structure passed the offline verifier. The ELF was not run on Linux here. |
| Windows x86_64 (GNU) | [`growthlab-v0.1.0-alpha.20-windows-x86_64-gnu.zip`](https://github.com/OthmaneBlial/GrowthLab/releases/download/v0.1.0-alpha.20/growthlab-v0.1.0-alpha.20-windows-x86_64-gnu.zip) | Built from the exact alpha.20 tag with `cargo-zigbuild`; checksum, traversal/link safety, required notices and ZIP structure passed the offline verifier. The executable was not run on Windows here. |

The current `main` source line (still versioned alpha.20) also produces local
`x86_64-unknown-linux-musl` and `x86_64-pc-windows-gnu` packages with
`cargo-zigbuild` and Zig. Their checksums,
archive safety and required notices passed the offline verifier; neither binary
was run on its target operating system here. They are evidence of reproducible
cross-target packages, not runtime or installer proof. The exact-tag Linux,
Windows GNU, macOS x86_64 and Linux arm64 archives are attached to alpha.20;
the current-main packages remain separate reproducibility checks.

The archives are unsigned and not notarized. They are CLI archives rather than
desktop installers. Linux and Windows runtime behavior remains unverified on the
current macOS host.

## Local installer generation

The current alpha.20 configuration can generate the cargo-dist shell and
PowerShell installer scripts locally, without contacting GitHub or uploading
anything:

```sh
dist build --artifacts global --installer shell,powershell \
  --tag v0.1.0-alpha.20 --allow-dirty
sh -n target/distrib/growthlab-installer.sh
pwsh -NoLogo -NoProfile -NonInteractive -Command \
  '$p = Get-Content -Raw target/distrib/growthlab-installer.ps1; [System.Management.Automation.Language.Parser]::ParseInput($p, [ref]$null, [ref]$null) | Out-Null'
```

The 2026-09-17 local generation produced `growthlab-installer.sh` (55,250
bytes) and `growthlab-installer.ps1` (22,419 bytes), both syntax-checked. The
shell installer was also run end to end against a local HTTP server serving the
`aarch64-apple-darwin` archive (SHA-256
`247ce54db01d2c1f6fb881dd4647789955ee01b715be397a60d2a5c495ba113b`): it
verified the sidecar, installed both `growthlab` and `orx` into an isolated
prefix, and reported version `0.1.0-alpha.20` without editing the test profile.
The templates still remain local evidence only: the release lacks a Windows
MSVC artifact, and no installer is advertised as portable or runtime-validated
until each matching target runner proves it.

## Reproduce and inspect an archive locally

Build the host-target archive with all checked-in notices:

```sh
scripts/package-cli-archive.sh
scripts/verify-release-archive.sh dist/archives/growthlab-v0.1.0-alpha.20-macos-arm64.tar.gz
```

The packager creates deterministic tar and gzip metadata so two builds from the
same source and toolchain can be compared. The verifier rejects symlinks,
absolute paths, traversal entries and missing notices. It executes the binary
only when the archive target matches the current operating system and CPU; for
other targets it still verifies the archive structure and reports runtime as
skipped.

To verify the assets currently attached to the public alpha.20 release, run the
explicit read-only network check:

```sh
python3 scripts/test-release-assets.py
```

It compares GitHub's asset digests with downloaded bytes, validates all five
checksum sidecars and runs the local archive safety/notices verifier. It does
not contact an analytics or agent provider and does not turn a skipped Linux
runtime into a support claim.

## Target matrix

The release configuration names the targets below, but a target is called
validated only after its binary has been built and run on that platform:

| Target | Packaging shape | Current state |
| --- | --- | --- |
| `aarch64-apple-darwin` | `.tar.xz` or local `.tar.gz` | **Observed** on macOS arm64 for alpha.20 |
| `x86_64-apple-darwin` | `.tar.xz` or local `.tar.gz` | **Built and structure-verified locally**; version command passed under Rosetta on arm64, native x86_64 runtime proof pending |
| `x86_64-unknown-linux-musl` | `.tar.xz` | **Built and structure-verified locally**; runtime proof pending |
| `aarch64-unknown-linux-musl` | `.tar.xz` or local `.tar.gz` | **Built and structure-verified locally**; exact alpha.20 archive attached, runtime proof pending |
| `x86_64-pc-windows-gnu` | `.zip` | **Built and structure-verified locally**; exact alpha.20 archive attached, runtime proof pending |
| `x86_64-pc-windows-msvc` | `.zip` / PowerShell installer | Build and runtime proof pending |

The cargo-dist configuration preserves the intended portable target matrix and
shell/PowerShell installer formats. GitHub Actions remains disabled for this
repository, so those artifacts are not presented as built until a local or
explicitly authorized platform runner supplies the evidence.

For cross-builds on macOS, install the matching Rust target. Linux musl and
Windows GNU use Zig through `cargo-zigbuild`; macOS Intel uses Cargo's Apple
linker:

```sh
rustup target add x86_64-unknown-linux-musl
cargo install cargo-zigbuild --locked
scripts/package-cli-archive.sh --target x86_64-unknown-linux-musl
scripts/verify-release-archive.sh dist/archives/growthlab-v0.1.0-alpha.20-linux-x86_64-musl.tar.gz
```

```sh
rustup target add x86_64-pc-windows-gnu
scripts/package-cli-archive.sh --target x86_64-pc-windows-gnu
scripts/verify-release-archive.sh dist/archives/growthlab-v0.1.0-alpha.20-windows-x86_64-gnu.zip
```

```sh
rustup target add x86_64-apple-darwin
scripts/package-cli-archive.sh --target x86_64-apple-darwin
scripts/verify-release-archive.sh dist/archives/growthlab-v0.1.0-alpha.20-macos-x86_64.tar.gz
```

```sh
rustup target add aarch64-unknown-linux-musl
scripts/package-cli-archive.sh --target aarch64-unknown-linux-musl
scripts/verify-release-archive.sh dist/archives/growthlab-v0.1.0-alpha.20-linux-arm64-musl.tar.gz
```

## Notices and trust boundary

Every generated archive includes `LICENSE`, `NOTICE.md`, `README.md`,
`docs/demo.md`, and the files under `licenses/`. `NOTICE.md` identifies the
OpenResearch foundation and the embedded dashboard/dependency notices. The
archive contains no provider credentials, telemetry payloads, signing keys or
user data. A checksum authenticates the downloaded bytes; it does not replace
code signing or notarization.

Source builds remain the fallback on any supported Rust platform:

```sh
cargo build --release --locked --bin growthlab
./target/release/growthlab --no-telemetry version
```

See [the alpha.20 release notes](releases/v0.1.0-alpha.20.md) for the current
feature and limitation record, and [alpha.19](releases/v0.1.0-alpha.19.md) for
the previous archive history.
