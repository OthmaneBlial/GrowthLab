# GrowthLab

**Give AI agents your product. Let them compete to grow it.**

GrowthLab is building isolated, reproducible growth experiments for your product,
with evidence, implementation, and tradeoffs behind every variant.
Local-first. Open source. No Docker required.

> Development status: source alpha. Product configuration, Git workspace import
> and a three-variant CLI replay battle are implemented, with real isolated edits,
> validations, sealed evidence, transparent command comparison, explicit candidate
> selection, guarded apply/export, interrupted-attempt recovery and private-by-default
> HTML/Markdown reports. The GrowthLab dashboard now exposes local import, goal
> composition, battle execution, captured evidence/diffs and explicit delivery
> through Rust domain APIs. A key-free bundled fictional replay is available through
> `growthlab demo`, including restricted previews of archived static candidate pages.
> Automatic run screenshots and a credible release are being built.
> No conversion lift, winning variant, adoption, or production readiness is claimed.

## Why GrowthLab?

An idea is useful when you can test it. GrowthLab's goal is to give competing
agents the same product context and evaluation contract, let them implement
hypotheses in isolated Git worktrees, and make their evidence and diffs inspectable.
You select a candidate and authorize applying it.

Generic marketing chat tools end at a proposal. GrowthLab is being engineered
around files, commits, runs, validation, reproducibility, and explicit provenance.
Measured telemetry, deterministic observations, model estimates, and declared
simulations must remain distinguishable. Without outcome telemetry, the product
will say **Recommended candidate**, never claim measured growth.

## Source development

Requirements: Git, stable Rust/Cargo, Node.js 22+, and pnpm 10. Validation requires
`/usr/bin/sandbox-exec` on macOS, or `/usr/bin/bwrap` with permitted unprivileged
namespaces on Linux. macOS and Linux are the initial targets; Linux isolation
needs native runtime verification. Windows validation isolation is not implemented.

```sh
git clone https://github.com/OthmaneBlial/GrowthLab.git
cd GrowthLab
pnpm -C ui install --frozen-lockfile
cargo test --locked
node scripts/dev-slot.mjs start --db empty
node scripts/dev-slot.mjs status
```

The dev-slot helper starts an isolated local database, Rust backend, and UI;
its status prints the actual ports. Stop it with
`node scripts/dev-slot.mjs stop`. The canonical binary is `growthlab`, with `orx` retained as a compatibility
entry point. Install only the canonical command with
`cargo install --path . --bin growthlab --locked`. See the
[configuration workflow](docs/configuration.md) for product import and hypotheses.
The [battle workflow](docs/battles.md) documents replay execution and current native limits.
The [delivery workflow](docs/delivery.md) covers selected patches, apply previews
and self-contained reports with explicit context disclosure.
The [recovery workflow](docs/recovery.md) explains verified checkpoints, surviving
jobs and interrupted outcomes without rerunning providers or commands.
The [validation isolation policy](docs/confinement.md) explains permitted runtime
files, offline dependency limits, private policy evidence and platform support.
The [dashboard workflow](docs/dashboard.md) covers the local app and its current
limits; the [domain API](docs/growth-api.md) documents controller and artifact semantics.
Do not use the upstream OpenResearch installer to install GrowthLab.

## Try the bundled battle

With the source prerequisites and installed canonical command:

```sh
growthlab demo
```

It opens a real three-variant battle on an original fictional product without
an agent API key. One proposal deliberately fails a heading check; two remain
eligible for review. Proposals are **SIMULATED**, checks **OBSERVED**, and growth
outcomes **UNTESTED**. Inspect the logs and diffs, then choose whether to select
and export a candidate. See the [demo workflow](docs/demo.md) for source commands,
requirements and current limitations. No candidate is automatically applied.

![Actual bundled battle: two eligible candidates and a deliberate heading failure](docs/screenshots/growth-demo-failure.png)

## Architecture and privacy

Rust/Axum service and CLI, SQLite persistence, Git worktrees, content-addressed
source snapshots, provider-agnostic coding-agent harnesses, and a React dashboard.
The full technical migration is documented in the
[foundation audit](docs/openresearch-foundation.md).

Product work stays local. External publishing, messaging, deployment, ads,
billing changes, and credential/customer-data access are outside the default
agent contract. Upstream telemetry is disabled in source builds; inherited
release jobs are gated to the upstream repository. GrowthLab will not redirect
your product data to an upstream service.

## Build with us

See the [roadmap](docs/ROADMAP.md), [current progress](docs/PROGRESS.md), and
[full product specification](docs/SPEC.md). The first milestone is a complete,
three-variant landing-page battle on a bundled product, including real file
changes, validation, immutable records, explainable evaluation, and safe export.
A deterministic replay mode will be labeled as a demo and require no API key.

## Attribution

Built from [alphaXiv/OpenResearch](https://github.com/alphaXiv/OpenResearch),
with upstream history preserved. MIT licensed; see [LICENSE](LICENSE) and
[NOTICE.md](NOTICE.md). alphaXiv does not endorse GrowthLab.
