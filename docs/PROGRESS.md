# GrowthLab progress

Current milestone: **Phase 2 — Selected delivery and local reports** (2026-09-16).
Overall completion: **about 30%, subjective estimate against the full specification**.
The configuration/import and three-variant replay CLI slices pass locally.
Selected delivery and report behavior pass local Rust, real CLI and browser checks.
The complete dashboard/native-agent/visual-demo vertical slice and credible release
have not passed yet.

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
- Added three competitors with a frozen contract and common imported source
  commit/digest, lab-owned branches/worktrees and transactional registration.
  Failed registration removes only its new worktrees/branches; moving the
  product HEAD does not move a prepared battle's baseline.
- Executed declared replay proposals as real allowed edits/commits, then ran
  configured commands on immutable candidate source archives. Actual failures,
  exit codes, bounded logs, timeout and cancellation remain inspectable.
- Added independent timeout watchdogs, descendant termination, exclusive
  execution leases and terminal-attempt rerun refusal using inherited job/run
  primitives. This is not host filesystem/network confinement.
- Sealed contract, proposal context/instructions, diffs, committed file artifacts
  and validation logs in digest-verified archives. SQLite rejects sealed run
  updates; comparison refuses tampered evidence.
- Added a pluggable transparent command comparison: replay proposals SIMULATED,
  executed checks OBSERVED and growth outcomes UNTESTED. Multiple eligible
  candidates require user review; no measured growth winner is inferred.
- Preserved the native harness registry with strict tools-disabled proposal
  capability checks. Only Claude declares that capability currently; native
  execution has not passed in this environment.
- Added explicit eligible-candidate selection and append-only delivery receipts
  in a transactional schema-v3 migration that preserves existing product data.
- Added selected patch export from exact committed objects, checked against
  sealed file artifacts. Mutable branches/worktrees cannot supply delivery contents.
- Added read-only apply preview and guarded product working-tree apply. Dirty,
  staged, untracked, sparse/assume-unchanged or changed-baseline/policy states are
  refused. External Git diff/fsmonitor execution is disabled and content filters
  refused before inspecting status. No automatic product commit/push occurs.
- Added create-only owner-private Unix patch/report files with collision and
  dangling-symlink refusal. Audit intent must persist before product delivery.
- Added self-contained HTML/Markdown reports with actual check failures, diff
  counts, selected candidate, digests, low confidence and visible provenance.
  Private context is withheld by default; disclosure and footer removal are explicit.
  Report summaries remain exportable when the original checkout is unavailable.

## Work in progress

The Phase 1 identity/domain foundation is implemented. Broader URL/manual
brief/non-Git onboarding and richer context remain required product work.
Phase 2 has a tested replay CLI backend, selected delivery and local reports;
native-agent verification, interrupted-attempt/delivery recovery and host execution
confinement remain pending. GrowthLab API/dashboard operations are missing.
The inherited dashboard was inspected in an empty isolated dev slot, then
stopped cleanly. This was baseline behavior, not a Growth Battle demo.

## Next three concrete tasks

1. Close execution confinement and interrupted-attempt/delivery recovery gates, then
   verify genuine native-agent proposal execution without inventing provider data.
2. Expose the working loop through GrowthLab domain APIs and a dashboard with
   actual run progress, inspectable evidence/diffs and explicit delivery controls.
3. Add the bundled one-command visual demo and archived render artifacts,
   inspect interactions/mobile, capture real screenshots/video and validate releases.

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
- `cargo test --locked`: **passed**, 889 tests per binary on macOS; 2 inherited
  tests ignored per binary (production telemetry contract and live Slurm cluster).
- `cargo clippy --all-targets -- -D warnings`: **passed**.
- `cargo build --locked` and real-binary `scripts/test-growth-cli.py`: **passed**.
  Three UNTESTED templates persisted; permissions rejected invalid targets;
  product files/HEAD/remotes and existing cache on refused update were preserved.
- Real-binary `scripts/test-growth-battle.py`: **passed**. Three real Git
  worktrees shared one baseline; two candidates passed and one failed with
  observed exit code 2. Explicit selection, patch export, apply preview and actual
  selected working-tree apply passed; HEAD/index/remotes were preserved. Default
  reports omitted private context; overwrite, rerun and evidence tampering were
  refused. Product files stayed unchanged until explicit apply. These are synthetic fixtures,
  not native-agent or real growth-outcome evidence.
- HTML report browser review: **passed**, desktop 1600×782 CSS pixels and phone
  390×844 CSS pixels; scrollWidth equaled innerWidth, including expanded command,
  seal and reproducibility details on phone. Keyboard Enter toggled a disclosure;
  actual failed exit 2 and source digests were inspectable. No captured warning/error
  logs. The page contained zero scripts and only a data favicon resource element.
  Temporary viewport override was reset. Real report screenshots are in
  `screenshots/cli-report-desktop.jpg` and `screenshots/cli-report-phone.jpg`;
  these show synthetic CLI report output, not the future dashboard or product renders.
- UI localized generation/typecheck/unit tests: **passed**, 163 tests.
- Upstream baseline `node scripts/dev-slot.mjs start --db empty`: **passed**,
  backend 4901/UI 5201; slot subsequently stopped cleanly.
- Upstream baseline `/api/health`: **passed**, protocol 2, version 0.2.3.
- Upstream baseline Chrome initial onboarding render: **passed**, no captured error/warning logs;
  scrollWidth=innerWidth=1280. No broader interaction/mobile claim.
- Foundation/documentation GitHub CI for
  `53e40bcd77567fb11faafbe5f9987397dc891156`: **passed**.
- Battle backend commit `2eefabc3506bc449f99d970ac9445d1285031bdf`, CI run
  `35088486941`: **Linux passed; Windows failed** in an archive-tamper fixture
  Clippy check for clearing a read-only flag. Linux passed the real CLI smokes,
  874 tests per binary and the inherited private-output/ETXTBSY regressions.
  Removed unnecessary Windows flag clearing while retaining direct tampering
  and verification-refusal assertions. Fresh exact-commit CI must verify Windows
  and the selected delivery/report milestone; older results do not validate it.
- Community issue templates: **passed**, parsed with Ruby standard YAML.
- GitHub topics/discussions/private vulnerability reporting: **verified enabled**.

Known environment warning: installed external Claude CLI `--version` failed
during inherited harness detection. Native-agent execution is not verified.
Local CLI/domain/replay validation passed. Selected delivery/report behavior on Linux/Windows,
GrowthLab dashboard, native-agent battles, host execution confinement,
interrupted-attempt/delivery recovery, archived render artifacts, visual
demo, release installers and telemetry adapters remain **unverified / not
implemented**. No growth lift, adoption, native-agent execution or public release
is claimed.
