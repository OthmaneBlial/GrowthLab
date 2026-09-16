# GrowthLab domain API (source alpha)

The local dashboard serves these routes under `/api/growth`. JSON fields use
camelCase for domain records; the committed configuration retains its documented
snake_case schema. Unknown request fields are refused. JSON bodies are limited
to 1 MiB. The existing loopback/origin and remote-host authentication guards apply.

| Method | Path | Behavior |
|---|---|---|
| GET | `/capabilities` | OS isolation availability and tools-disabled harness capabilities; not authentication/provider proof |
| POST | `/demo` | Accept `{}` to create a new owned fictional product and launch its three declared replay proposals; HTTP 202 returns `projectId`, `battleId`, `accepted` |
| GET / POST | `/workspaces` | List imported contexts / import `{ "path": "/local/product", "initializeGit": false }`; set `initializeGit` explicitly to create a local snapshot for a non-Git folder |
| GET | `/workspaces/{id}` | Read the recorded committed product context |
| GET / POST | `/workspaces/{id}/hypotheses` | List / create three UNTESTED starter hypotheses |
| GET / POST | `/battles` | List (optional `projectId` query) / prepare `{ "projectId": "…", "goal": "…" }` |
| GET | `/battles/{id}` | Frozen battle, variants, verified sealed runs/checkpoints, selections and owned-controller status |
| POST | `/battles/{id}/run` | Submit an explicit replay/native request; HTTP 202 means accepted |
| POST | `/battles/{id}/cancel` | Persist a cancellation request; not instant termination proof |
| POST | `/battles/{id}/recover` | Existing verified recovery, without rerunning proposals or commands |
| GET | `/battles/{id}/compare` | Transparent configured-command comparison plus an optional estimated HTML SEO rubric; no measured growth inference |
| GET | `/battles/{id}/report` | Self-contained attachment; `format=html` or `markdown` |
| GET | `/variants/{id}/artifacts` | Verified checkpoint/archive entries, sizes, digests and seal status |
| GET | `/variants/{id}/artifact?name=…` | Verified UTF-8 text artifact; refuses traversal, private prompts and policies |
| GET | `/variants/{id}/static-preview` | Verified JSON `{html, record, archiveDigest, sealed}` for a ready archived static preview; 404 when absent/unavailable |
| GET | `/variants/{id}/static-preview/{viewport}` | Verified `image/png` capture for `desktop` or `phone` when that viewport was archived; 404 when unavailable |
| POST | `/variants/{id}/select` | Record an eligible reviewed candidate |
| GET | `/variants/{id}/apply-preview` | Read-only selected-patch preview and baseline checks |
| POST | `/variants/{id}/apply` | Explicit selected working-tree apply with `{ "confirmed": true }` |
| POST | `/variants/{id}/export` | Create-only selected patch with `{ "path": "/new/selected.patch" }` |
| POST | `/delivery/{receiptId}/recover` | Inspect or explicitly resume a pending selected delivery with `{ "resume": true }` |
| GET | `/events` | SSE invalidations derived from persisted state, with reconnect snapshot |

Replay execution accepts:

```json
{
  "mode": "replay",
  "plan": {
    "version": 1,
    "implementations": [
      {"summary": "Declared proposal A", "files": [], "risks": []},
      {"summary": "Declared proposal B", "files": [], "risks": []},
      {"summary": "Declared proposal C", "files": [], "risks": []}
    ]
  }
}
```

This illustrates the request shape, not useful implementation content; add the
actual allowed file edits documented in [battles.md](battles.md). The engine checks
file paths, secrets, edits and the frozen contract. Native execution accepts
`mode: "native"`, `harness`, optional `model`, and `agentTimeoutSeconds` (1–3600,
default 120). Adapter capability does not establish installation or authentication.

An owned worker retains its Store, project admission and shared storage lease
through execution. The host admits at most two active battles; configured
parallelism controls competitors inside each battle. Duplicate/previously started
battles are refused. Shutdown requests cancellation only for this host's workers,
waits briefly and preserves unfinished checkpoints for explicit recovery.

SSE `growth.updated` carries workspace IDs, battle/attempt statuses, cancellation
intent, checkpoint digests and selection statuses. It is an invalidation snapshot,
not raw logs or an execution claim. Clients fetch authoritative records after
notifications. `resync.required` indicates unavailable metadata; reconnect sends
the current snapshot. Streams are bounded and stop when their client disconnects.

Artifact reads verify every archive byte, frozen contract/run serialization and
recorded confinement policy digests and optional static-preview source metadata.
Only `implementation.diff`, `agent.log`, `validation-{index}.log`, `files/…`,
`preview/document.html` and `preview/metadata.json` are text viewer entries;
the complete raw preview-source bundle and PNG captures are not exposed by this
text viewer. Binary text reads are refused. Never use this text endpoint as an
executable HTML preview.

The separate static-preview route also returns JSON, preserving the server's
HTML response protections. Display its document only in an opaque, inert iframe
with an empty sandbox; the archived CSP blocks scripts and external resources.
The [preview contract](static-previews.md) documents limits and unsupported input.
If present, `GET /api/growth/variants/{id}/static-preview/desktop` returns the
verified archived PNG with `image/png`; it remains a render artifact, not a
visual quality score or measured growth result.

Each comparison row may include `rubric` when the sealed implementation contains
HTML. The `rubric.id` is `seo-page-hygiene-v1`, its `provenance` is `ESTIMATED`,
and `dimensions` contains the inspectable `key`, `label`, `score`, `maxScore`,
`status` and `evidence` for title, description, headings, language, useful copy,
canonical URL, links and image descriptions. `recommendations` contains
deterministic next steps for partial or missing dimensions. The 100-point total
is a structural page review; it is never a ranking, traffic, accessibility or
conversion result.

Dashboard delivery requires the latest completed selection to match the requested
variant. Confirmation does not bypass clean-baseline, permission or seal checks.
The CLI's explicit `apply <variant-id>` authorization remains compatible.

Reports default to private-context omission. Optional `includeContext=true`,
`publicGoal=…` and `withoutAttribution=true` mirror the CLI's explicit report
options. Attachments are `no-store`; sharing remains the user's decision.

Conflicts return HTTP 409 (active controller, storage move, deletion or started
battle); stopping/unavailable controller startup returns 503. Domain validation
errors return 400, absent records 404 and unavailable storage 500. JSON errors
contain an `error` string; framework body/query rejections may be plain text.
No endpoint automatically publishes, messages, commits the original product,
pushes to it, deploys or connects outcome telemetry.

For a single page outside a battle, the local CLI also exposes
`growthlab seo-audit --html ./path/to/index.html` or
`growthlab seo-audit --url https://example.com/path`. It uses the same
`seo-page-hygiene-v1` rubric and supports `--format markdown`. URL analysis is
read-only, checks same-origin `robots.txt`, follows no redirects and makes one
bounded HTTPS request; local-file analysis remains network-free. See
[seo-audit.md](seo-audit.md).

For user-supplied outcome rows, `growthlab measure --csv ./telemetry.csv`
produces a local descriptive comparison with sample sizes, optional date range,
arithmetic baseline differences and exploratory 95% intervals when samples allow.
It is labelled **MEASURED** from the supplied CSV, does not contact a provider,
and does not establish causality or statistical significance. See
[measurement.md](measurement.md) for the documented normal-approximation limits.
