# Product configuration and hypotheses

GrowthLab configuration schema v1 is `growthlab.yaml`, committed in the product
repository. Secrets never belong in it. Unknown fields, invalid modes, duplicate
YAML keys, malformed paths and unsupported versions are rejected. The file must
be regular, at most 64 KiB, and cannot be a symlink. Parser errors give locations
without echoing arbitrary values.

```yaml
version: 1
product:
  name: Acme
  audience: Open-source maintainers
  description: A local developer tool
goal:
  primary: Increase qualified signups
permissions:
  mode: implementation
  allowed_paths:
    - website/
  denied_paths:
    - .env
    - infra/production/
validation:
  commands:
    - pnpm test
    - pnpm build
  timeout_seconds: 120
metrics:
  primary: signup_conversion
  guardrails:
    - bounce_rate
    - page_load_time
agents:
  parallelism: 3
```

## Permissions

- `analyze_only` (alias `analysis`): no product file changes.
- `draft`: artifacts belong to the lab store; no product file changes.
- `implementation` (alias `implement`): only allowed product paths, in isolated
  worktrees. Requires at least one allowed path and validation command.
- Applying a selected variant will be a separate explicit action; a config
  mode never grants automatic deployment, push, merge or messaging permission.

Paths are relative file/directory prefixes using `/`, with optional trailing `/`
on configuration prefixes. They are not globs. Denied paths override allowed
paths. Denials also reject case variants, preventing a `PRIVATE` spelling from
bypassing a `private` denial on macOS/Windows. Allowed prefixes keep exact case.
Prefix checks respect directory boundaries: `website` does not allow
`website-other`. Absolute paths, traversal, drive prefixes, backslashes and
globs are rejected. Credential files (`.env*`, PEM/key files and common secrets/
credentials names), `.git`, `.growthlab`, `.ssh`, `.aws` and `growthlab.yaml`
cannot be implementation targets. Writes through symlink files/directories are
rejected. These checks are not an operating-system sandbox for agent processes.

Validation accepts up to 16 commands, each at most 4096 bytes; a command timeout
is 1–3600 seconds (default 120). Parallelism is 1–8. Product name, audience,
goal and primary metric are nonempty and at most 4096 bytes each. Configured
commands are executable code and must be user-authorized; no automatic outbound
integration permission is inferred from them.

## Current CLI workflow

After building with `cargo build --locked`, use `target/debug/growthlab`, or
install from source with `cargo install --path . --bin growthlab --locked`.

```sh
growthlab init --path /path/to/product \
  --name Acme --audience 'Open-source maintainers' \
  --goal 'Increase qualified signups' \
  --mode implementation --allow website/ \
  --deny infra/production/ --validate 'pnpm test' --validate 'pnpm build'
growthlab config check --path /path/to/product
growthlab config check-path --path /path/to/product website/index.html
```

`init` defaults to analysis only and atomically creates a new configuration;
it never overwrites a file. Review and commit only that file using the product's
own Git workflow, then import it:

```sh
growthlab workspace import --path /path/to/product
growthlab workspace list
growthlab workspace view <project-id>
growthlab hypotheses <project-id>
growthlab hypotheses <project-id> --list
```

Import records the full HEAD commit and committed config, verifies the on-disk
configuration agrees, and registers an inherited LocalProject with publication
disabled. It never changes product files, commits or remotes. Import currently
requires a Git repository root with a checked-out branch and committed context;
non-Git folders, URL analysis, public clones and manual briefs remain onboarding
work in the roadmap.

`hypotheses` creates three deterministic starter templates: outcome-first
positioning, verifiable proof, and faster first success. It does not claim an
agent ran or external research occurred. These records are **UNTESTED**, low
confidence, inconclusive, and have no outcome metric value. Each records the
committed user brief as observed input; that evidence supports the configured
audience/goal, not a causal hypothesis or product-market fit.
Execution, battle ranking and selected apply/export are the next vertical slice.

## Persistence and provenance

Typed growth workspace/hypothesis records extend the inherited SQLite store.
`growth_schema_migrations` is independent of upstream transcript `user_version`.
New migrations and portfolios are transactional and propagate failures. IDs,
source commit, user-provided context, mechanism, metric/guardrails, baseline,
risks, evaluation mode, confidence rationale and result provenance are explicit.
No threshold is invented when the user has not supplied one.

Provenance is mandatory in JSON and will be mandatory in run/report APIs:
`MEASURED` real connected telemetry, `OBSERVED` deterministic check/direct
inspection, `ESTIMATED` stated model/rubric assumptions, `SIMULATED` declared
simulation, `UNTESTED` no outcome evidence. An untested hypothesis cannot be
marked `ship`. Model estimates are not objective outcome measurements.

Default data is under `$XDG_DATA_HOME/growthlab` or `~/.local/share/growthlab`;
override with `GROWTHLAB_DATA_DIR` (legacy `ORX_DATA_DIR` still works). Settings
are under `$XDG_CONFIG_HOME/growthlab` or `~/.config/growthlab`. GrowthLab does
not move an existing OpenResearch default store/cache. Generic compatibility
commands remain available through `orx`, but use this fork's installed alias.
Both binaries report the GrowthLab alpha version; automatic upstream updates
are refused. No GrowthLab stable installer/release is claimed yet.
