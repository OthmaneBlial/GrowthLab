# Repository Guide

## What this repository is

`growthlab` is a local-first Rust growth experimentation product built from alphaXiv/OpenResearch. `growthlab` is the canonical CLI; `orx` is a compatibility entry point sharing the same implementation. It owns the local CLI, dashboard/API, SQLite store, coding-agent integrations, Git worktrees and experiment orchestration.

Read `docs/SPEC.md`, `docs/PROGRESS.md`, `docs/ROADMAP.md` and `docs/openresearch-foundation.md` before substantial changes. The full requested product scope remains binding; a partial scaffold or green test suite is not a credible-release claim. Preserve LICENSE, NOTICE.md and upstream history.

The inherited `openresearch.sh` companion service is not a GrowthLab service. Do not edit it or redirect local product data there. Inherited remote integrations remain optional. Upstream release/signing workflows are isolated and the upstream updater is refused. Never require Docker for local install, development, tests or demos.

## Development guidelines

- Rust code lives in `src/`; the dashboard lives in `ui/src/`. Keep local-only behavior local and use the production API client only for capabilities owned by `openresearch.sh`.
- Growth domain types/configuration live in `src/growth/`; growth SQLite extensions live in `src/store/growth.rs`. Preserve inherited generic primitives. New migration errors must propagate and new migration versions must not collide with transcript `user_version`.
- Record MEASURED/OBSERVED/ESTIMATED/SIMULATED/UNTESTED provenance on results. Template proposals are untested; replay proposals are simulated; actual deterministic checks are observed. Without real outcome telemetry, recommend a candidate and do not claim measured growth or a Winner.
- Permissions are explicit. Product changes belong in isolated worktrees and only allowed paths. Apply only an explicitly selected candidate. No automatic deploy, push to a product repository, messaging, ads, live billing/analytics changes, credentials or customer-data access.
- Run local app instances through `scripts/dev-slot.mjs` so development data, ports, and processes stay isolated.
- `ui/dist` is committed and embedded in release builds. After UI changes, run `pnpm build` in `ui/` and include the regenerated assets.
- Prefer canonical Tailwind utilities (`flex flex-col h-full min-h-0`) and project theme aliases (`bg-background`, `text-subtext`, `border-border`). Use arbitrary values only when no project utility exists, and preserve semantic marker classes when selectors or runtime behavior depend on them.
- Before shipping, run the relevant checks from `.github/workflows/ci.yml` locally.

## CI and release gates

- Latest user instruction (2026-09-16): GitHub CI is disabled for now; validation
  runs locally only. All six GitHub Actions workflows are manually disabled,
  including release workflows that could invoke CI. Keep their definitions for
  future use, but do not enable or dispatch them without a new user instruction.

- Latest publishing instruction (2026-09-16): keep the README and project website
  current after every meaningful validated milestone. Update `docs/status.json`
  and the detailed `docs/PROGRESS.md` from actual evidence, then run
  `node scripts/sync-project-status.mjs` and its `--check` mode. Include the
  synchronized README, badges and `site/` status copies in the focused commit.
  Publish only the canonical `GrowthLab/` website folder and verify the live
  status. Percentages stay explicitly subjective; never promote pending work to
  passed or enable GitHub CI as part of synchronization.

- For this task the user explicitly authorizes direct-main work with focused commits and frequent pushes to `OthmaneBlial/GrowthLab`. Inspect status/diffs and run relevant validation before each coherent commit. Verify the exact remote/head after pushing. Do not configure protection that prevents the authorized workflow.
- If the user re-enables GitHub automation later, PR CI must test GitHub's simulated merge (`refs/pull/<number>/merge`), which `actions/checkout` selects by default for `pull_request` events, rather than checking out the PR head alone. Each run tests its merge candidate; subsequent changes to `main` do not automatically rerun open PRs.
- Preserved workflow definitions include `main` CI and release checks on the commit being packaged. These are disabled now. If release automation is restored later, publishing requires its checks to succeed; keep `./ci` in cargo-dist's `global-artifacts-jobs` when regenerating the release workflow.
