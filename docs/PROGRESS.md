# GrowthLab progress

Current milestone: **Phase 1 — Product context and experiment model** (2026-09-16).
Overall completion: **foundation and first domain slice complete; about 15%, subjective estimate**.
No GrowthLab battle vertical slice or credible release has passed yet.

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
- Added the native `growthlab` binary and compatible `orx` entry point, with
  separate default GrowthLab data/settings/cache namespaces and no upstream updater.
- Added strict versioned `growthlab.yaml`, atomic creation, three permission
  modes, allowed/denied path checks, symlink rejection and credential redaction.
  These checks are not yet agent process isolation.
- Added typed workspace/hypothesis/evidence/provenance records and transactional
  SQLite extensions with an independent migration ledger.
- Added local Git product import pinned to committed context/HEAD, workspace
  inspection, and three deterministic UNTESTED hypothesis templates. Input
  evidence is observed configuration, not market research or outcome evidence.
- Added real-binary CLI smoke validation of permissions, import, persistence,
  untouched product files/HEAD/remotes and refused updater cache preservation.
- Fixed an inherited database lease lifetime issue exposed by concurrent tests;
  a duplicated-descriptor regression failed before the fix and passes afterward.

## Work in progress

Phase 1 remains partial: URL/manual brief/non-Git onboarding, richer product
context and GrowthLab API/dashboard operations are still missing. The inherited
dashboard was inspected in an empty isolated dev slot, then stopped cleanly.
This was baseline behavior, not a Growth Battle demo.

## Next three concrete tasks

1. Implement the landing-page battle contract and three competitors from one
   pinned snapshot, with explicit replay/native execution and validation.
2. Seal run evidence/artifacts and add explainable comparisons, safe selected
   apply/export, failure/cancellation recovery and self-contained reports.
3. Expose the working loop in the GrowthLab dashboard and bundled demo, then
   inspect interactions/mobile and capture real screenshots/video.

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
- `cargo test --locked`: **passed**, 868 tests per binary on macOS; 2 inherited
  tests ignored per binary (production telemetry contract and live Slurm cluster).
- `cargo clippy --all-targets -- -D warnings`: **passed**.
- `cargo build --locked` and real-binary `scripts/test-growth-cli.py`: **passed**.
  Three UNTESTED templates persisted; permissions rejected invalid targets;
  product files/HEAD/remotes and existing cache on refused update were preserved.
- UI localized generation/typecheck/unit tests: **passed**, 163 tests.
- Upstream baseline `node scripts/dev-slot.mjs start --db empty`: **passed**,
  backend 4901/UI 5201; slot subsequently stopped cleanly.
- Upstream baseline `/api/health`: **passed**, protocol 2, version 0.2.3.
- Upstream baseline Chrome initial onboarding render: **passed**, no captured error/warning logs;
  scrollWidth=innerWidth=1280. No broader interaction/mobile claim.
- Foundation/documentation pushes: **verified**; GitHub CI for
  `53e40bcd77567fb11faafbe5f9987397dc891156`: **passed**. This is not CI evidence
  for the subsequent domain changes; their new remote run must be checked separately.
- Community issue templates: **passed**, parsed with Ruby standard YAML.
- GitHub topics/discussions/private vulnerability reporting: **verified enabled**.

Known environment warning: installed external Claude CLI `--version` failed
during inherited harness detection. Native-agent execution is not verified.
Local CLI/domain validation passed. Growth battle/dashboard behavior, Linux and
Windows runtime, native-agent battles, release installers and telemetry adapters
remain **unverified / not implemented**. No growth lift, adoption, agent execution
or public release is claimed.
