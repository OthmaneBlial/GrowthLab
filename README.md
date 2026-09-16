# GrowthLab

**Give AI agents your product. Let them compete to grow it.**

GrowthLab is building isolated, reproducible growth experiments for your product,
with evidence, implementation, and tradeoffs behind every variant.
Local-first. Open source. No Docker required.

> Development status: source alpha. Product configuration, Git workspace import
> and untested starter hypotheses are implemented. The dashboard currently retains
> the inherited research UI; Growth Battles and a credible release are being built.
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

Requirements: Git, stable Rust/Cargo, Node.js 22+, and pnpm 10. macOS and Linux
are the initial targets. Windows support inherits upstream foundations and
requires separate runtime validation.

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
Do not use the upstream OpenResearch installer to install GrowthLab.

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
