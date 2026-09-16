# GrowthLab progress

Current milestone: **Phase 1 — Product context and experiment model** (2026-09-16).
Overall completion: **foundation complete; about 10%, subjective estimate**.
No GrowthLab vertical slice or credible release has passed yet.

## Completed

- Verified authenticated GitHub account and absence of `OthmaneBlial/GrowthLab`.
- Created the public repository and preserved full upstream Git history locally.
- Set origin to `git@github.com:OthmaneBlial/GrowthLab.git` and upstream to
  `https://github.com/alphaXiv/OpenResearch.git`.
- Audited MIT permission/notice obligations; retained LICENSE and added NOTICE.
- Inspected Rust store, tree, Git/worktree, snapshot, run, harness/chat, prompt,
  skill, dashboard, UI, privacy and CI/release foundations. Recorded migration
  decisions in `openresearch-foundation.md`.
- Gated inherited upstream release dispatch/signing so this fork cannot publish
  an upstream-branded release on its initial push.
- Retained the full requested scope in SPEC.md and phase/release gates in ROADMAP.

## Work in progress

GrowthLab configuration, provenance, and typed domain extensions are next.
The inherited dashboard runs in an empty isolated dev slot; its initial onboarding
was inspected in Chrome. This is baseline behavior, not a Growth Battle demo.

## Next three concrete tasks

1. Add validated growthlab.yaml and typed product context/permissions/provenance.
2. Persist growth hypotheses/battles as transactional extensions to the existing
   store and expose GrowthLab-native CLI/API operations.
3. Build the complete bundled landing-page battle: shared snapshot, real replay
   edits, validations, sealed archives, explainable comparison, safe apply/export.

## Architectural decisions

- Keep Rust/Axum/SQLite and inherited generic Git, run and harness primitives.
- Extend domain records; do not replace the upstream persistence model.
- Immutable source archives are insufficient for immutable evaluation evidence.
- GrowthLab defaults must not contact upstream telemetry/update/publishing
  services. Inherited compatibility screens are not a GrowthLab experience.
- Results require provenance per input/check; proxy ranking is a Recommended
  candidate, not a claim of measured conversion success.
- Docker is absent from the local workflow; inherited optional remote/release
  container artifacts are isolated pending GrowthLab release tooling.

## Latest validation

- `pnpm install --frozen-lockfile` in ui: **passed**.
- `node --test scripts/dev-slot.test.mjs`: **passed**.
- `node ui/scripts/check-i18n.mjs`: **passed**.
- `node ui/scripts/check-styles.mjs`: **passed**.
- `cargo fmt --all --check`: **passed**.
- `cargo test --locked`: **passed**, 855 tests; 2 inherited tests ignored by
  upstream (production telemetry contract and live Slurm cluster).
- UI localized generation/typecheck/unit tests: **passed**, 163 tests.
- `node scripts/dev-slot.mjs start --db empty`: **passed**, backend 4901/UI 5201.
- `/api/health`: **passed**, protocol 2, upstream version 0.2.3.
- Chrome initial onboarding render: **passed**, no captured error/warning logs;
  scrollWidth=innerWidth=1280. No broader interaction/mobile claim.
- First push: **verified**, remote main = local `3bf7343de9fec674695dd9f8e03df1799506f974`.
- Community issue templates: **passed**, parsed with Ruby standard YAML.
- GitHub topics/discussions/private vulnerability reporting: **verified enabled**.

Known environment warning: installed external Claude CLI `--version` failed
during inherited harness detection. Native-agent execution is not verified.
No code/test failure is established; local API and initial UI still started. Growth-specific functionality, Linux and
Windows runtime, native-agent battles, release installers and telemetry adapters
are **unverified / not implemented**.
