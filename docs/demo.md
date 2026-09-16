# Bundled fictional Growth Battle

With an installed `growthlab` and the Git, Node.js and validation-isolation
prerequisites in the README, run:

```sh
growthlab demo
```

From a source checkout, the equivalent is
`cargo run --locked --bin growthlab -- demo`. There is no Docker, package download
or agent API key in this replay. The command opens the real embedded dashboard
at a new loopback port and leaves the server running until Ctrl-C. Use
`growthlab demo --no-browser` for a printed URL, or `--port 5901` to choose a port.
An occupied explicit port is refused rather than opening an unrelated battle.

In the development dashboard started through `scripts/dev-slot.mjs`, Home's
**Run bundled demo** button runs the same backend preparation and worker.
It accepts execution on the bundled fictional product; it does not import or
apply changes to your own product. Each invocation creates a new private UUID
directory under the active data root's `growth-demo/`, initializes its own local
Git baseline and registers three isolated competitor worktrees.

The original bundled product is **PatchKit**, a fictional static homepage with
no customers, testimonials, external assets, scripts or outcome telemetry.
Its source lives in [`demo/patchkit/`](../demo/patchkit/). Three declared proposals
change its actual homepage through the real battle engine:

| Proposal | Structure | Local links | Claims constraints | Eligibility |
|---|---|---|---|---|
| Outcome-first positioning | Exit 0 | Exit 0 | Exit 0 | Eligible for review |
| Proof beside the promise | Exit 2: deliberately missing primary heading | Exit 0 | Exit 0 | Ineligible |
| Faster first success | Exit 0 | Exit 0 | Exit 0 | Eligible for review |

These are expected reproducible fixture checks, not accessibility certification,
full HTML validation, performance scores or conversion evidence. Each check runs
Node against an immutable candidate snapshot under the OS confinement policy.
The deliberately failed competitor makes the overall battle status **failed**;
the other two remain reviewable candidates. Failure does not skip the remaining
configured checks. Missing prerequisites or interrupted jobs can produce different,
honestly recorded results; execution is refused if OS isolation is unavailable.

In the dashboard, expand a check fraction, inspect the real diff and validation
logs, compare the calculation and review the seal, source and policy digests.
Proposals are **SIMULATED**, executed checks **OBSERVED**, and growth outcomes
**UNTESTED**. No candidate is automatically selected. Select an eligible candidate
only after review, then export a new patch or explicitly confirm a working-tree
apply to the fictional baseline. Neither operation commits, pushes or deploys it.
The HTML/Markdown report omits private product names and paths by default.

The CLI demo skips inherited automatic session/run restoration and provider
preflight/authentication monitoring. It refuses repository-scoped Git environment
overrides before creating a fixture, and initializes its own Git baseline with
fictional author metadata, signing and hooks disabled. Existing product repositories
are not demo inputs. The full dashboard retains inherited compatibility routes;
deliberately using them is a separate workflow.

Demo repositories, worktrees and sealed evidence are retained in the active data
root for inspection; the command does not silently delete them afterward. A failed
preparation may leave only its newly owned partial fixture there. macOS runtime
isolation is locally validated; Linux runtime verification remains pending, and
Windows validation isolation is unsupported. Candidate HTML is inspectable
as sealed source text and a restricted desktop/phone preview of archived static
source. See [static-previews.md](static-previews.md); no saved candidate screenshot
or visual quality result is implied. Automatic screenshot archives, richer
evaluators and a release installer remain in progress.

Real local captures show the [Home launch screen](screenshots/growth-demo-home.png),
[running checks](screenshots/growth-demo-live.png),
[deliberate failure and candidate fractions](screenshots/growth-demo-failure.png)
and [phone layout](screenshots/growth-demo-phone.png). These are dashboard captures
of real execution, not automatic candidate-page render archives.

The real-command regression can be run locally after `cargo build --locked`:

```sh
python3 scripts/test-growth-demo.py target/debug/growthlab
```

It uses private temporary data/config/cache, actual Git/Node execution and HTTP
records, verifies expected failures and clean baselines, checks privacy and graceful
shutdown, and detects unexpected provider CLI invocations. Its provider sentinels
do not substitute for the battle engine or configured commands. Failed checks retain
only their owned evidence directory for inspection. GitHub CI remains disabled.
