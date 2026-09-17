# Distribution

GrowthLab ships source first. Release files must carry the same provenance as
the product: a user can inspect the executable, its checksum, the MIT notice,
the OpenResearch attribution, the demo guide, and the dependency notices
without contacting an analytics or agent provider.

## Current source line and binary evidence

`v0.1.0-alpha.20` includes one manually validated executable archive and one
cross-target archive whose structure was verified locally. The previous alpha.19
archive remains available in its release history:

| Target | Asset | Evidence |
| --- | --- | --- |
| macOS arm64 | [`growthlab-v0.1.0-alpha.20-macos-arm64.tar.gz`](https://github.com/OthmaneBlial/GrowthLab/releases/download/v0.1.0-alpha.20/growthlab-v0.1.0-alpha.20-macos-arm64.tar.gz) | Extracted, checksum-checked, and run locally with `--no-telemetry version`; `growthlab 0.1.0-alpha.20` reported the development build channel. |
| Linux x86_64 (musl) | [`growthlab-v0.1.0-alpha.20-linux-x86_64-musl.tar.gz`](https://github.com/OthmaneBlial/GrowthLab/releases/download/v0.1.0-alpha.20/growthlab-v0.1.0-alpha.20-linux-x86_64-musl.tar.gz) | Built from the exact alpha.20 tag with `cargo-zigbuild`; checksum, traversal/link safety, required notices and archive structure passed the offline verifier. The ELF was not run on Linux here. |

The alpha.20 source line also produces a local `x86_64-unknown-linux-musl`
archive with `cargo-zigbuild` and Zig. Its checksum, archive safety and required
notices passed the offline verifier; the ELF binary was not run on Linux here.
It is evidence of a reproducible cross-target package, not Linux runtime or
installer proof. It is attached to alpha.20 for inspection with that limitation.

The archives are unsigned and not notarized. They are CLI archives rather than
desktop installers. The Linux archive is attached for inspection; Windows
artifacts are not attached, and Linux/Windows runtime behavior remains
unverified on the current macOS host.

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

## Target matrix

The release configuration names the targets below, but a target is called
validated only after its binary has been built and run on that platform:

| Target | Packaging shape | Current state |
| --- | --- | --- |
| `aarch64-apple-darwin` | `.tar.xz` or local `.tar.gz` | **Observed** on macOS arm64 for alpha.20 |
| `x86_64-apple-darwin` | `.tar.xz` | Build and runtime proof pending |
| `x86_64-unknown-linux-musl` | `.tar.xz` | **Built and structure-verified locally**; runtime proof pending |
| `aarch64-unknown-linux-musl` | `.tar.xz` | Build and runtime proof pending |
| `x86_64-pc-windows-msvc` | `.zip` / PowerShell installer | Build and runtime proof pending |

The cargo-dist configuration preserves the intended portable target matrix and
shell/PowerShell installer formats. GitHub Actions remains disabled for this
repository, so those artifacts are not presented as built until a local or
explicitly authorized platform runner supplies the evidence.

For a Linux musl cross-build on macOS, install a Rust target, Zig and
`cargo-zigbuild`, then run:

```sh
rustup target add x86_64-unknown-linux-musl
cargo install cargo-zigbuild --locked
scripts/package-cli-archive.sh --target x86_64-unknown-linux-musl
scripts/verify-release-archive.sh dist/archives/growthlab-v0.1.0-alpha.20-linux-x86_64-musl.tar.gz
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
