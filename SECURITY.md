# Security policy

GrowthLab is in early development. Do not treat it as a sandbox for hostile
repositories or untrusted coding agents. Local agent CLIs and configured shell
commands can execute code with the user's operating-system privileges.
Git worktrees isolate variant files; they are not an operating-system security
boundary. Permission enforcement and archive redaction require their own tests.

The default product contract excludes deployment, purchasing ads, messaging,
social posting, live billing/analytics changes, credential files and customer
data. Only an explicitly selected variant may be applied to a product. Each
future external integration needs scoped authorization at its mutation boundary.

Keep `.env`, API keys, synced secrets, local databases, private prompts and
customer data out of Git, reports, screenshots and issues. Source archives can
contain anything tracked by Git; never track credentials. Reports should share
only approved evidence, sanitized names, and redacted reproducibility metadata.

## Reporting a vulnerability

Use this repository's GitHub private vulnerability reporting capability when
available. Include reproduction steps, affected commit/version and impact,
with synthetic credentials and a minimal nonprivate fixture. Do not open a
public issue containing an exploit against a live service or actual secrets.

Only capabilities described in tagged GrowthLab release notes are supported
release behavior. Inherited upstream version numbers are not GrowthLab security
support promises. There is no supported stable GrowthLab release yet.
