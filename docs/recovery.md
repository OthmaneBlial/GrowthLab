# Recover an interrupted CLI battle

Recovery inspects an existing battle and its registered validation jobs. It never
reruns an agent or command, resets a worktree, or delivers a product patch.

```sh
growthlab battle-status BATTLE_ID
growthlab recover BATTLE_ID
growthlab compare BATTLE_ID
growthlab report BATTLE_ID --output recovered.html
```

`battle-status` includes all attempts as well as finalized run seals. An attempt's
`checkpointDigest` identifies a verified private checkpoint; `archiveDigest`
identifies a finalized immutable outcome. A checkpoint is evidence of captured
state, not evidence that the candidate succeeded.

## What recovery preserves

Growth schema v4 stores checkpoint digests independently of terminal seals.
Execution checkpoints the frozen contract and proposal context before invoking an
agent; captures the committed candidate, diff and changed file artifacts before
validation; and durably links each registered validation before launching it.
Collected command records and bounded, redacted logs are checkpointed too.
A checkpoint failure before launch prevents the command from starting.

Recovery first acquires the battle's controller lease and verifies every retained
checkpoint and existing terminal seal. A live Growth controller refuses recovery.
Tampered archives or mismatched registered jobs refuse recovery before changing
outcomes. Mutable candidate worktrees are never used as recovered evidence.
The original product checkout can be unavailable during recovery and reporting.

When a registered command is still alive, the result is `waiting`, with its
recorded job ID. Nothing is finalized. Its existing independent watchdog remains
responsible for the configured deadline; repeat `recover` after the same job ends.
Cancellation intent is preserved, but recovery does not signal guessed processes
or turn an observation timeout into proof of termination.

After jobs end, their actual recorded exit codes and captured logs can be retained.
An attempt interrupted before its terminal checkpoint becomes **failed** or
**cancelled**, even if its recovered command exited zero. It cannot become an
eligible candidate. Proposal/check/outcome provenance remains distinct; actual
command observations do not prove growth outcomes. The recovered completion time
is the inspection time, and the command may have ended earlier.

A complete terminal checkpoint whose final database seal was interrupted can be
reattached using exactly its original digest and run record. A successful terminal
checkpoint must satisfy the frozen command evaluator. Previously sealed siblings
remain unchanged. Repeated recovery is safe and returns `unchanged` once finished.

## Compatibility and current limits

Existing schema-v3 sealed archives retain their serialized bytes, comparisons
and selected patch delivery after migration. Interrupted legacy attempts without
checkpoints cannot recover uncaptured context and never become successful
candidates. Registered orphan job observations can be archived separately; they
are explicitly excluded from the evaluation rubric.

If a launcher directory exists without valid process or exit registration,
termination is **unverified** and recovery stays `waiting`. This rare interruption
window still needs a stronger launch protocol. Recovery does not clean up or
rerun an unknown launcher. Checkpoints are retained as private content-addressed
bundles; automatic checkpoint garbage collection is not implemented.

This workflow covers CLI attempts and recorded jobs. Interrupted selected-delivery
receipts, data-root relocation, native-agent runtime verification and operating
system filesystem/network confinement remain release gates. Validation runs use
minimal environments, but they are not yet an OS sandbox.

## Local verification

The recovery smoke uses temporary synthetic repositories and real Node commands.
It kills only the CLI controller it created, confirms its child jobs remain live,
releases their test gates, recovers sealed context/logs without the product checkout,
and checks repeat recovery and Git preservation. It requires no API key or Docker.

```sh
cargo build --locked
python3 scripts/test-growth-recovery.py target/debug/growthlab
```

An optional second argument is a pre-checkpoint GrowthLab binary. It creates real
legacy seals and verifies that the current binary preserves comparisons and patch
delivery through the schema upgrade. All GitHub Actions workflows are disabled
at the user's request; run validation locally only.
