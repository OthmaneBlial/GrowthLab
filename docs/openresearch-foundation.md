# OpenResearch foundation audit

Audited source: `325eb509dc8e4ca7074568cf0ae1f0f98704eac0` on upstream `main`,
2026-09-16. This note describes inspected source; runtime validation is recorded
separately in [PROGRESS.md](PROGRESS.md). GrowthLab preserves upstream history.

## Instructions and license

Read `AGENTS.md`, `CLAUDE.md`, `SYSTEM_PROMPT.md`, `SKILL.md`, `Cargo.toml`,
`LICENSE`, `build.rs`, and the CI/release workflows before implementation.
`CLAUDE.md` delegates to `AGENTS.md`. Source is MIT, copyright 2026 alphaXiv.
Keep `LICENSE` intact and include `NOTICE.md` with distributions.
GrowthLab follows the user's direct-main commit/push workflow; inherited branch
protection guidance does not override that explicit instruction.

## Persistence and domain

`src/store.rs` owns one bundled SQLite connection per Store, WAL, a 5-second
busy timeout, local projects/experiments, runs, chat sessions/messages/turns,
turn leases, queue admission, spawns, run wakeups, and persisted UI state.
`Store::open_at` supports isolated tests without process-global environment edits.
Migrations include idempotent table creation, legacy column additions, and a
transactional `user_version=1` backfill for transcript lineage. New GrowthLab
migrations must propagate errors and use their own version ledger so they do not
collide with the transcript migration.

`src/local/model.rs` defines API-serializable LocalProject and LocalExperiment.
An experiment has a project, parent, stable UUID, slug, branch, fixed run command,
timestamps, and originating chat session. `StoredRun` records execution status,
backend descriptor, source commit, exit code, cancellation intent, and result text.
These generic nodes remain the foundation. Growth context, hypotheses, evidence,
contracts, provenance, rubric evaluations, and battle decisions need typed
extensions rather than encoding everything in a description string.

## Git and lineage

`src/local/projects.rs` prepares/imports local paths and public clones, creates a
local project, and optionally configures publication. `src/local/git.rs` owns
Git operations, branch/diff/file inspection, public-clone validation, baseline
branches, worktrees, and legacy storage migration. Session worktrees are detached
from a baseline initially; an agent can check out its experiment branch.
`ensure_worktree_at` verifies an existing worktree's expected commit and cleanliness.
Missing registrations can be pruned; broken links preserve files and return an
error. Cleanup may force-remove session worktrees, so GrowthLab must not reuse
that destructive policy on a user's product checkout.

`src/local/experiments.rs::create_experiment` creates an `orx/<slug>` branch and
row, inheriting the parent's validation/run contract. Multiple independent
baselines are allowed. Agent skills freeze answered experiment nodes and descend
through child hypotheses. A battle must additionally pin all siblings to the
same source commit; using a moving parent branch tip for each competitor is not
sufficient. Branch creation plus SQLite persistence needs compensating cleanup.

## Execution, snapshots, evidence

`src/compute.rs` defines backend-agnostic preflight, staging, submission, logs,
cancellation, and recovery. `SourceSnapshot::create` uses `git archive` on the
recorded experiment commit and addresses archives by SHA-256. Owner-only snapshot
files are digest-checked when restored. A run executes the archived commit,
never a mutable agent worktree. Preserve this invariant.

`src/local/localrun.rs` submits local controller jobs through
`src/jobs/localbox.rs`. `src/commands/supervise.rs` supervises detached jobs,
logs and persisted cancellation. `src/commands/exp.rs` exposes status/run/cancel/
wait/wake. Backend descriptors retain transport metadata and source identity.
SSH, Slurm, Ray, Hugging Face, Modal, Kubernetes, Tinker, and upstream managed
compute are optional inherited adapters, not requirements for local use.

Upstream immutable *source* snapshots do not make all results immutable:
run rows/results are updated, logs append, and project artifacts are mutable.
GrowthLab needs sealed per-run evidence/evaluation/artifact manifests, digest
verification, atomic finalization, and redacted reproducibility records.
`src/local/files.rs`, `git.rs`, and the dashboard expose files, diffs, artifacts,
raw content, logs, and result references. Reuse viewers; add growth provenance
and evidence objects. Source snapshots must never accidentally archive tracked
credentials simply because Git tracked them.

## Harnesses and autonomous loop

`src/local/harness/mod.rs` provides a registry and object-safe async Harness
trait for detection, normalized chat turns, and skill installation. Claude Code,
Codex, OpenCode, and Cursor adapters retain native capabilities and permission
identifiers. Detection does not imply every harness supports every capability.
`src/local/chat/` owns turn context, host coordination, cancellation, recovery,
idempotency, persisted transcripts, steering, and permission/question prompts.
Recovery snapshots and watchdog/retry budgets are shared primitives.

`SYSTEM_PROMPT.md` is injected via each native harness channel, substituting
project context/state and artifact paths. `src/local/agent_skills.rs` embeds and
installs the packages in `agent-skills/`. The tree, Git, compute, delegation,
evidence, and reports skills encode an autonomous research loop. Literature,
paper, and figure procedures are science-specific and must not be mechanically
renamed into growth playbooks. GrowthLab needs focused role instructions and an
explicit shared battle contract; proposal and evaluation contexts must differ.

## Dashboard and lifecycle

`src/commands/up.rs` owns an Axum loopback server, embedded `ui/dist`, JSON APIs,
SSE, terminal/websocket routes, local agent hosts, run recovery and shutdown.
`/api/events` compares store/log state; UI clients must use domain APIs.
The persistent remote-host mode has a separate bearer boundary; ordinary
loopback operation is not multiuser authentication.

`ui/src/` is React/TypeScript, TanStack Router/Query, Tailwind, Paraglide,
XYFlow, xterm, and diff/Markdown/file viewers. Routes wrap `RemoteRuntime` and
workspace screens; `App.tsx` composes the existing research workspace. Keep
generic tree, console, viewers and query invalidation; build a growth-native
home, goal composer, battle, evidence, rubric and permissions flow backed by Rust.
Existing demo session state is a UI fixture, not proof of executed growth agents.
GrowthLab's domain API now lives in `src/commands/up/growth_api.rs`, merged into
the inherited guarded Axum router. It reuses the GrowthLab CLI battle, recovery,
evaluation, selection and report modules. Owned workers retain their Store root,
project admission and shared storage lease; native !Send harness futures run on
a local runtime inside the worker thread. GrowthLab React screens consume these
APIs and verified checkpoints/seals rather than inheriting research results or
inventing frontend execution state. The underlying generic routes remain compatible.
Use `scripts/dev-slot.mjs --db empty` for isolated runtime verification, never
copy a normal user's database into a public demo. Build and commit `ui/dist`
after UI changes because release binaries embed it.

## Privacy, installer and release boundaries

`src/telemetry.rs` gates upstream opt-out telemetry to the compile-time production
channel. `build.rs` only permits that channel inside alphaXiv/OpenResearch Actions;
source builds are development. GrowthLab will keep telemetry disabled by default
and never repurpose the upstream endpoint. Settings also include automatic update
and optional new-project GitHub publication behavior: both require review before
exposing them through GrowthLab's happy path.

Upstream README points at `openresearch.sh/install.sh`; update/install modules,
cargo-dist metadata, macOS branding, signing workflows and readme download assets
refer to upstream releases. These are not GrowthLab installers. Inherited release
jobs are gated to upstream until a GrowthLab-specific, validated release path
exists. Do not dispatch upstream telemetry-contract checks or signing jobs here.
No Dockerfile/Compose is needed. Kubernetes and container runner configuration
are inherited optional remote/release artifacts, isolated from local startup.

## Migration map

| OpenResearch concept | GrowthLab concept | Decision |
|---|---|---|
| Research project | Product workspace | Extend LocalProject with typed product context |
| Hypothesis | Growth hypothesis | New typed audience/mechanism/metric contract |
| Experiment branch | Growth variant | Reuse branch/tree; pin battle snapshot |
| Run | Evaluation/live experiment run | Reuse supervisor; add provenance and seals |
| Research artifact | Copy/page/campaign/report artifact | Reuse viewer; snapshot per run |
| Evidence | Source/audit/event/user fact | New typed evidence store and claim links |
| Experiment tree | Growth experiment lineage | Reuse tree with battle grouping and decisions |
| Autoresearch loop | Autonomous growth loop | Reuse harness host; add growth roles/contracts |
| Scholarly discovery | Permitted product/market research | New boundary; primary sources and pending state |
| Research score | Explainable candidate evaluation | New evaluators, weights, inputs and limitations |

## First complete landing-page slice

Bundle a tiny static developer-tool product with committed validation commands.
Create three hypotheses and worktrees from one commit. A declared deterministic
replay adapter makes real file changes without API keys; native harness execution
uses the same structured contract when explicitly selected. Execute validation,
capture actual logs/diffs/artifacts, seal runs, compare decomposable checks and
estimated copy rubrics, export a sanitized HTML/Markdown report, and apply only
an explicitly selected candidate to a clean matching product snapshot.
No traffic, conversion, revenue or statistical significance is simulated into a
real outcome. Demo proposals are SIMULATED; actual checks are OBSERVED; model
judgments are ESTIMATED. Without telemetry, show Recommended candidate.

## Verification scope

Inherited CI checks localization/style, dev-slot tests, UI typecheck/unit tests,
Rust format/clippy/build/test, development telemetry on debug/release binaries,
Windows build/tests, and version sanity. Source contains substantial harness,
Git, persistence, recovery, source transport and UI fixtures. New modules need
unit/integration/migration/failure/cancellation tests in addition to these gates.
Local macOS checks do not prove Linux/Windows runtime or published installers.
