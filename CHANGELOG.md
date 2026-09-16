# GrowthLab release notes

## Unreleased — source alpha (0.1.0-alpha.1)

- Preserved OpenResearch history and MIT attribution; audited its Rust, SQLite,
  worktree, snapshot, run, agent-harness and dashboard foundations.
- Added canonical `growthlab` CLI and retained the `orx` compatibility entry point.
- Added schema-v1 `growthlab.yaml` with explicit product/goal, permission prefixes,
  validation commands/timeouts, metrics/guardrails and bounded parallelism.
- Added atomic config creation, safe parse errors, protected-path/symlink checks
  and common credential detection/redaction.
- Added local Git workspace import pinned to committed product configuration,
  publication disabled, and transactional SQLite growth extensions.
- Added three untested starter hypothesis templates with explicit context,
  mechanism, baseline, source commit, confidence, risks and mandatory provenance.
- Isolated default GrowthLab data/settings/cache and refused the upstream
  updater/release comparison. Upstream publishing/signing jobs are gated away.

This is not a tagged credible release. The growth-native dashboard, executable
battles, native-agent contracts, sealed run evidence, candidate comparison,
selected apply/export, reports, bundled replay demo and real outcome measurement
remain work in the roadmap. No telemetry lift, production readiness, public
installer or demonstrated adoption is claimed.
