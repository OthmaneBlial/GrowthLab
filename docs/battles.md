# Growth Battle CLI — current source alpha

The backend now prepares and executes a three-competitor landing-page battle.
Each uses one frozen product commit, configuration and validation contract.
This is a working CLI slice. [Selected apply/export and reports](delivery.md)
extend it; the bundled visual demo and GrowthLab dashboard exercise the same
backend through the local API.

## Prepare and inspect

Import a product using the [configuration workflow](configuration.md). A
non-Git folder can be initialized explicitly with `workspace import --init-git`;
existing repositories still require a committed configuration.
Its committed configuration must enable implementation with explicit allowed
paths and validation commands. The battle records the imported source commit;
it does not move when you continue working on the product's main branch.

```sh
growthlab battle 'Improve qualified activation' --project <project-id> --prepare-only
growthlab experiments --project <project-id>
growthlab battle-status <battle-id>
growthlab compare <battle-id>
```

Preparation creates three inherited experiment nodes, `growthlab/<battle-id>/…`
branches and lab-owned worktrees. All start at the exact same recorded commit.
Product checkout files, HEAD and remotes stay untouched. Checkout/commit hooks
and commit signing are disabled for controller-generated operations. Failed
registration compensates only its newly created branches/worktrees.

Current source safety limits: 2048 regular tracked files, 4 MiB per blob and
32 MiB total. Tracked symlinks/submodules, protected credential paths, denied
tracked data and recognizable credentials are refused before checkout/archive.
Prepare a product snapshot without that off-limits data. This conservative
check cannot recognize all private data and is not an OS sandbox.

## Declared replay execution

A replay plan is a regular JSON file, at most 1 MiB, with exactly three ordered
implementations. It is deterministic simulation input, not evidence that a
native agent ran. For example, for a permitted `website/index.html`:

```json
{
  "version": 1,
  "implementations": [
    {
      "summary": "Outcome-first fixture",
      "files": [{"path": "website/index.html", "contents": "<h1>First useful outcome</h1>"}],
      "risks": ["Declared simulation; no outcome telemetry"]
    },
    {
      "summary": "Proof-first fixture",
      "files": [{"path": "website/index.html", "contents": "<h1>Inspect the product</h1>"}],
      "risks": ["Use only verifiable product facts"]
    },
    {
      "summary": "Quick-start fixture",
      "files": [{"path": "website/index.html", "contents": "<h1>Reach first success</h1>"}],
      "risks": ["No causal growth result is inferred"]
    }
  ]
}
```

```sh
growthlab run <battle-id> --replay /path/to/plan.json
# Or prepare and execute together:
growthlab battle 'Improve qualified activation' --project <project-id> --replay /path/to/plan.json
growthlab compare <battle-id>
```

Replay makes real isolated edits/commits and runs the actual configured commands.
`contents: null` deletes an allowed regular file. Every proposal is checked before
writes; duplicate/oversized files, symlinks, traversal, protected/denied paths
and recognizable credentials are rejected. Limits are 64 edits, 128 KiB/file
and 1 MiB total implementation text. Competing execution honours parallelism.

Validation executes content-addressed candidate source archives in separate
controller directories, never the mutable agent worktree. It reuses the localbox
backend and generic StoredRun records. The environment excludes inherited and
synced provider credentials. Commands require OS isolation: snapshot/scratch
writes, read-only approved runtime roots and no host network access. Supervisor
files and the original product remain outside the command's permissions.
Captured policy bytes and their digest accompany the checks. Read the
[confinement policy](confinement.md) for platform support and dependency limits;
Linux runtime proof and broader security gates remain pending.

A launcher watchdog enforces the configured timeout independently of the Rust
controller, sends TERM then KILL, and records timeout status. Log capture is
bounded to 1 MiB; truncation/limit failure is explicit. Cancellation intent is
persisted and polled during proposal and validation:

```sh
growthlab battle-status <battle-id> --cancel
```

Cancelled/failed attempts remain inspectable. Terminal battles are not silently
rerun or overwritten; create a new battle for a new attempt. A failed validation
is represented in battle/run JSON even when CLI orchestration itself exits
successfully. Inspect status rather than treating exit zero as all checks passed.
Use [explicit recovery](recovery.md) for interrupted checkpointed attempts;
unregistered launchers and interrupted selected delivery remain separate gates.

## Native adapters

`--harness <id>` explicitly selects the native CLI/provider to receive the allowed
text context and generate structured edits. GrowthLab controls file writes;
the child is a separate proposal context, not its own evaluator. Native prompts,
product inputs, requested model policy and outputs are local run evidence.
Cost/tokens/actual model are not invented when the inherited adapter lacks them.

The registry preserves Claude Code, Codex, OpenCode and Cursor. Current strict
tools-disabled proposal capability is declared only by Claude Code's `--tools ''`
transport. Other adapters fail without a provider request until they enforce that
capability. Native execution is not yet verified in this environment; the installed
Claude CLI failed during baseline detection. Native implementations remain
UNTESTED for growth outcomes; deterministic checks have their own provenance.

## Seals and comparison

Every finished attempt atomically installs a content-addressed archive under
`growth-archives/<sha256>`. Its manifest seals contract, proposal inputs/instructions,
outcome, implementation diff, changed committed files and actual validation logs
when available. Common credentials are redacted from logs; invalid proposals do
not become file artifacts. Directories are private and files read-only on Unix;
the application refuses overwrite and verifies bytes on every comparison.
SQLite prevents updates to a sealed run. A digest mismatch refuses comparison.

The comparison shows configured command results individually and, when a
candidate records an HTML implementation, runs the deterministic
`seo-page-hygiene-v1` rubric. It gives separate 0–20/0–15/0–10/0–5 dimensions
for title, description, headings, language, useful copy, canonical URL, links
and image descriptions. Each dimension exposes its observation in the API and
dashboard; the total is labeled **ESTIMATED**. The rubric reviews archived HTML
only and does not crawl, predict rankings, certify accessibility or measure
traffic/conversion.

The pass fraction is `passed commands / required commands`, normalized only
within that contract. Eligibility requires all exact commands to pass on one
immutable candidate commit and a successful sealed attempt. Ties still require
user review; no combined score is presented as a measured winner.

Proposal provenance is SIMULATED for replay. Command checks are OBSERVED.
Growth outcome provenance remains UNTESTED, with low confidence. The label is
**Recommended candidates**. It never establishes conversion lift or a Winner.

CLI JSON and local archives contain user-provided product context for inspection;
they are not sanitized public/shareable reports. Report export with appropriate
privacy controls is available through the [delivery workflow](delivery.md).
