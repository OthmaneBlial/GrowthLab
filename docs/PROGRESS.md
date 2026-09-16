# GrowthLab progress

Current milestone: **Phase 0 — Foundation audit** (2026-09-16).
Overall completion: **early foundation; below 10%, subjective estimate**.
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

Inherited Rust tests and isolated dashboard build are running; UI dependencies
were installed with the frozen lockfile. UI typecheck/tests are running.

## Next three concrete tasks

1. Record completed baseline test/runtime results; commit and push this audited
   foundation, verifying the exact remote head.
2. Add typed, validated product configuration and provenance/hypothesis entities
   with transactional SQLite extensions and failure-path tests.
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
- `cargo test --locked`: **running**; no pass claim yet.
- UI localized generation/typecheck/unit tests: **running**.
- `node scripts/dev-slot.mjs start --db empty`: **building** in isolated slot 1.

Known failures: none established yet. Growth-specific functionality, Linux and
Windows runtime, native-agent battles, release installers and telemetry adapters
are **unverified / not implemented**.
