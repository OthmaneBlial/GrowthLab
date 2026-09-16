# Build GrowthLab — an autonomous growth experimentation lab

Build **GrowthLab**, an ambitious, production-grade, local-first platform that turns coding agents into **growth experimentation agents**.

GrowthLab is a new product built from the open-source core and architectural foundations of:

```text
https://github.com/alphaXiv/OpenResearch
```

OpenResearch is not a casual visual inspiration. Its codebase, experiment model, local-first architecture, agent harnesses, Git worktrees, immutable run archives, evidence tracking, and autonomous research loop are the technical foundation from which GrowthLab must evolve.

The transformation is:

> OpenResearch: turn coding agents into research agents.

into:

> GrowthLab: give AI agents your product, let them compete to grow it, and ship the winning experiment.

The short product pitch is:

> **GrowthLab — The open-source growth team that runs experiments while you build.**

An equally important viral one-liner is:

> **Give AI agents your product. Let them compete to grow it.**

Do not build a generic marketing chatbot, an idea generator, a collection of prompts, a thin OpenResearch reskin, or a dashboard full of fake analytics.

Build a real experimentation workspace where every proposed growth change has a hypothesis, evidence, implementation, evaluation, lineage, files, diffs, artifacts, and an honest confidence level.

---

# 1. Non-negotiable product principles

GrowthLab MUST be:

- local-first by default
- open source
- useful without Docker
- installable and runnable directly on macOS and Linux, with Windows support pursued progressively
- agent-agnostic where OpenResearch already supports multiple harnesses
- honest about measured, observed, estimated, and simulated results
- Git-native and experiment-native
- suitable for indie hackers, open-source maintainers, developers, and small product teams
- impressive in a 30-second screen recording
- useful in a real workflow after the demo ends

GrowthLab MUST NOT:

- require Docker for installation, development, tests, demos, or normal usage
- fabricate traffic, conversions, revenue, rankings, user feedback, or statistical significance
- present an LLM opinion as an objective experiment result
- silently publish code, product data, credentials, analytics, or prompts
- perform deceptive SEO, spam, fake reviews, mass unsolicited outreach, dark patterns, or platform abuse
- copy third-party content or branding without permission
- promise guaranteed growth
- claim that generated variants have “won” without a defined evaluation method

When real product telemetry is unavailable, GrowthLab may rank variants using clearly labeled proxy evaluation such as heuristic review, rubric scoring, accessibility checks, performance checks, copy clarity, factual support, or user-supplied constraints. The UI must say **Predicted winner** or **Recommended candidate**, never **Winner**, when no real outcome data exists.

---

# 2. GitHub repository and source strategy

Create a new **public** GitHub repository:

```text
OthmaneBlial/GrowthLab
```

Before creating it, verify that this exact repository does not already exist. If it exists, inspect it and continue safely instead of overwriting it.

Use the connected GitHub integration or authenticated GitHub CLI, whichever is available. Never expose tokens or credentials.

GrowthLab should begin from the OpenResearch codebase so its proven primitives remain the core rather than being needlessly recreated.

Recommended source setup:

```bash
git clone https://github.com/alphaXiv/OpenResearch.git GrowthLab
cd GrowthLab
git remote rename origin upstream

# Create OthmaneBlial/GrowthLab through an authenticated GitHub capability.
git remote add origin git@github.com:OthmaneBlial/GrowthLab.git
git push -u origin main
```

If the upstream default branch is not `main`, inspect it and adapt safely.

Preserve OpenResearch's license, copyright notices, and relevant attribution. Audit the MIT license before making changes. Add a clear `NOTICE.md` or an attribution section explaining that GrowthLab is built from OpenResearch and linking to the upstream project. Do not imply endorsement by alphaXiv.

Keep this remote relationship:

```text
origin   -> OthmaneBlial/GrowthLab
upstream -> alphaXiv/OpenResearch
```

Do not squash away the upstream history merely to make GrowthLab appear unrelated. The originality must come from the product transformation and engineering, not erased attribution.

---

# 3. Required study of OpenResearch

Before major implementation, inspect the current OpenResearch repository deeply. Do not infer its architecture from the README alone.

Study at minimum:

- `AGENTS.md`
- `CLAUDE.md`
- `SYSTEM_PROMPT.md`
- `SKILL.md`
- `Cargo.toml`
- the Rust source under `src/`
- the UI under `ui/`
- `agent-skills/`
- the project/session/run/experiment persistence model
- Git worktree lifecycle
- experiment tree and lineage
- immutable run archival behavior
- logs, diffs, files, results, and artifact handling
- agent harness integration for Claude Code, Codex, OpenCode, and Cursor
- local SQLite persistence
- local dashboard startup and lifecycle
- remote execution boundaries
- telemetry implementation and opt-out behavior
- installer and release workflow
- test coverage and fixtures

Write an internal architecture note at:

```text
docs/openresearch-foundation.md
```

It must map reusable OpenResearch concepts to GrowthLab concepts, for example:

| OpenResearch concept | GrowthLab concept |
|---|---|
| Research project | Product workspace |
| Hypothesis | Growth hypothesis |
| Experiment branch | Growth variant |
| Run | Evaluation or live experiment run |
| Research artifact | Copy, page, campaign, report, or implementation artifact |
| Evidence | Research source, audit result, metric, event, or user input |
| Experiment tree | Growth experiment lineage |
| Autoresearch loop | Autonomous growth loop |

Identify what can be reused, what should be generalized, what must be renamed, and what requires a genuinely new subsystem.

Do not mechanically search-and-replace “research” with “growth.” Preserve generic components when their abstraction is sound. Refactor domain-specific components carefully and keep tests green during the migration.

---

# 4. Core user story

A user points GrowthLab at a product repository, website URL, or local product folder and describes the goal:

```text
Increase qualified signups for my open-source developer tool.
```

GrowthLab creates a structured product workspace, gathers only authorized evidence, and proposes an experiment portfolio such as:

```text
Goal: Increase qualified signups

├── Positioning
│   ├── outcome-led hero
│   └── pain-led hero
├── Activation
│   ├── shorter onboarding
│   └── interactive quick start
├── SEO
│   ├── competitor alternative page
│   └── use-case landing page
└── Launch
    ├── GitHub-first launch narrative
    └── Hacker News launch narrative
```

Independent agents explore selected branches in isolated Git worktrees. Each branch produces:

- a precise hypothesis
- target audience
- proposed channel
- evidence and sources
- expected mechanism
- implementation plan
- actual changed files when authorized
- screenshots or render artifacts where applicable
- validation commands
- outcome metric definition
- guardrail metrics
- risks
- confidence and rationale
- result classification: measured, observed, estimated, simulated, or not yet tested

The user can compare variants, inspect evidence and diffs, select one, export it, or apply the winning change.

---

# 5. The killer feature: Growth Battles

The signature feature is a **Growth Battle**.

A Growth Battle gives multiple agents or strategies the same product context, goal, constraints, and evaluation contract. Each creates a competing, isolated growth experiment.

Example:

```text
Growth Battle: Improve homepage activation

Agent A — Outcome-first positioning
Agent B — Interactive demo above the fold
Agent C — Social-proof-led landing page
Agent D — Faster path to first success
```

The battle screen must make comparison immediate:

```text
                    Clarity  Proof  A11y  Perf  Confidence
Outcome-first         91      78     96    94      Medium
Interactive demo      84      88     90    71      Medium
Social proof          79      93     95    92      Low
Fast first success    89      85     97    90      High
```

The score must be decomposable and inspectable. Never hide all judgment behind one unexplained number.

Where live analytics are connected, the comparison can progressively include real metrics such as conversion rate, activation rate, retention, revenue, sample size, runtime, and confidence intervals. Clearly display date range and data provenance.

Suggested CLI:

```bash
growthlab up
growthlab init
growthlab battle "Improve homepage signup conversion"
growthlab experiments
growthlab run <experiment-id>
growthlab compare <battle-id>
growthlab report <battle-id>
growthlab apply <variant-id>
```

Preserve compatibility aliases only when useful during migration from `orx`; the public product language should progressively become GrowthLab-native.

---

# 6. Product input and onboarding

The first-run experience should take less than three minutes for a normal local project.

Support these inputs progressively:

1. local Git repository
2. local non-Git folder, with a clear option to initialize Git
3. public GitHub repository URL
4. public website URL for read-only analysis
5. manually entered product brief

The onboarding flow asks only what is necessary:

- What is the product?
- Who is it for?
- What outcome matters now?
- What actions may agents perform?
- What commands verify the project?
- Which files or paths are off-limits?
- Is this analysis-only or implementation-enabled?

Create a committed configuration file with a stable, documented schema, for example:

```yaml
version: 1
product:
  name: Acme
  audience: Open-source maintainers
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
metrics:
  primary: signup_conversion
  guardrails:
    - bounce_rate
    - page_load_time
agents:
  parallelism: 3
```

Call the file `growthlab.yaml` unless repository conventions or implementation constraints strongly justify another name.

Secrets never belong in this file.

---

# 7. Experiment model

Generalize or extend the OpenResearch data model rather than replacing it blindly.

A growth experiment should include:

- stable ID
- title
- status
- parent experiment or battle
- goal
- audience
- channel
- hypothesis
- mechanism
- baseline definition
- primary metric
- guardrail metrics
- success threshold
- evaluation mode
- source snapshot commit
- worktree and branch metadata
- agent harness and model metadata
- prompts/instructions required for reproducibility, subject to secret redaction
- evidence records
- implementation diff
- validation runs
- artifacts
- start/end timestamps
- cost and token metadata where available
- result provenance
- confidence label
- decision: reject, revise, candidate, ship, or inconclusive

Result provenance is mandatory:

```text
MEASURED   = computed from connected real telemetry
OBSERVED   = produced by a deterministic check or direct inspection
ESTIMATED  = a model/rubric estimate with stated assumptions
SIMULATED  = generated in a declared simulation
UNTESTED   = no outcome evidence yet
```

This status must be visible wherever results are shown or exported.

---

# 8. Growth agent roles

Provide focused agent roles built on the existing agent-harness system. Do not hard-code the product to one model provider.

Initial roles:

- **Strategist** — decomposes goals and selects plausible growth levers
- **Researcher** — gathers permitted evidence and records sources
- **Positioning agent** — creates and critiques value propositions
- **Conversion agent** — improves landing pages and activation paths
- **SEO agent** — proposes ethical, evidence-based search opportunities
- **Onboarding agent** — reduces time to first value
- **Pricing agent** — explores packaging and pricing hypotheses without inventing market facts
- **Launch agent** — produces channel-specific launch plans and assets
- **Evaluator** — applies explicit rubrics and deterministic checks
- **Skeptic** — tries to falsify hypotheses and identify missing evidence

Agents must share the same structured product context but operate in isolated sessions/worktrees for competing variants.

The evaluator must not silently reward its own proposal. Prefer separate proposal and evaluation contexts, stable rubrics, deterministic checks, and user-defined weights.

---

# 9. Evidence and research

GrowthLab must make evidence a first-class object, as OpenResearch does.

Evidence records should capture:

- title
- source URL or local path
- retrieval date
- relevant excerpt or structured observation
- author/publisher when available
- evidence type
- which claim it supports or challenges
- confidence and limitations

Prefer primary sources for factual claims. Distinguish user-provided product facts from external research and model inference.

Respect robots.txt, site terms, rate limits, copyright, and access boundaries. Do not bypass authentication or scraping restrictions.

If web access is unavailable, continue with local/product evidence and clearly mark external research as pending.

---

# 10. First vertical slice

Do not begin by building every channel or integration. Deliver one complete end-to-end vertical slice:

1. start GrowthLab locally without Docker
2. create or import a product workspace
3. enter a growth goal
4. generate three structured hypotheses
5. select a landing-page battle
6. create three isolated worktrees from the same source snapshot
7. launch agents against the same contract
8. make real file changes in each worktree
9. run configured tests/builds and basic quality checks
10. archive every run immutably
11. show evidence, logs, artifacts, screenshots, and diffs
12. compare variants using a transparent rubric
13. label the result honestly as observed/estimated unless real telemetry exists
14. apply or export the selected variant
15. generate a shareable local report

The first vertical slice must work on a bundled demo product so contributors can reproduce the experience without API keys where technically possible. If an agent key is required, provide a deterministic fixture/replay mode for UI and end-to-end testing, clearly labeled as a demo.

Do not fake backend execution in the frontend.

---

# 11. Architecture direction

Retain OpenResearch's Rust-first, local-first foundation unless careful inspection proves a component should change.

Expected major subsystems:

- Rust application/service core
- SQLite persistence with migrations
- project and product-context management
- agent harness adapters
- worktree manager
- experiment/battle orchestration
- evidence store
- run scheduler and process supervisor
- immutable run archive
- evaluation engine
- artifact manager
- event stream for live UI updates
- local web dashboard
- CLI
- optional integration adapters

The UI should remain a client of stable domain APIs. Core behavior must not live only inside frontend components.

Do not introduce microservices, Kubernetes, message brokers, or distributed infrastructure for fashion. Keep the default architecture simple and local. Remote execution may remain available where inherited and useful, but must not complicate the local happy path.

No Dockerfiles, Docker Compose files, Docker-only instructions, or container dependency should be added unless the repository already contains an upstream artifact that cannot safely be removed immediately. If inherited Docker-related files exist, document whether they are unused and progressively remove or isolate them. Docker is not part of the GrowthLab product experience.

---

# 12. UI and user experience

The application must feel like a serious experimentation laboratory, not an admin template.

Core screens:

- Home / recent workspaces
- Product workspace overview
- Goal composer
- Experiment tree
- Growth Battle comparison
- Individual variant detail
- Live run console
- Evidence browser
- Diff and artifact viewer
- Evaluation breakdown
- Settings, permissions, and integrations

The experiment tree should preserve the most powerful OpenResearch mental model while adopting GrowthLab language.

Use strong information hierarchy, restrained color, excellent typography, dense but readable data, keyboard navigation, responsive layouts, accessible contrast, loading/error/empty states, and real-time feedback.

Do not fill the UI with decorative gradients, meaningless KPI cards, fake charts, fake testimonials, or placeholder metrics.

Every score should be clickable to reveal:

- calculation or rubric
- inputs
- evidence
- evaluator
- limitations
- provenance

---

# 13. Virality as a product feature

Do not treat “viral” as a README adjective. Engineer shareable moments.

## 13.1 Shareable Growth Battle report

Generate a beautiful self-contained report that can be exported as Markdown and HTML. It should show:

- goal
- competing variants
- screenshots
- small diff summaries
- transparent score breakdown
- result provenance
- selected candidate
- reproducibility metadata
- “Built with GrowthLab” attribution that users may remove

Never include secrets, private repository names, full private prompts, or hidden file paths without explicit user approval.

## 13.2 README badge

Support an honest badge such as:

```markdown
[![GrowthLab Battle](https://img.shields.io/badge/GrowthLab-3_variants_tested-72F1B8)](...)
```

Do not display performance claims in a badge unless they derive from published evidence.

## 13.3 One-command demo

Provide a compelling local demo:

```bash
growthlab demo
```

It should open a bundled, reproducible Growth Battle within minutes and require no Docker.

## 13.4 Launch-quality repository

The GitHub repository must include:

- a killer README hero
- a concise animated demo or short video
- real screenshots
- a clear “Why GrowthLab?” section
- comparison with generic AI marketing tools
- architecture overview
- local-first/privacy explanation
- quick start
- demo workflow
- roadmap
- contribution guide
- security policy
- code of conduct
- issue templates
- discussion prompts
- accurate project topics
- release notes

Suggested GitHub topics:

```text
growth
growth-hacking
ai-agents
experimentation
local-first
rust
git-worktrees
indie-hackers
marketing
open-source
```

Suggested README opening:

```text
# GrowthLab

Give AI agents your product. Let them compete to grow it.

GrowthLab creates isolated, reproducible growth experiments for your product,
then shows you the evidence, implementation, and tradeoffs behind every variant.
Local-first. Open source. No Docker required.
```

Do not claim adoption, speed, conversion lift, customer logos, or production readiness before evidence exists.

---

# 14. Evaluation engine

Build evaluation as a pluggable, explainable system.

Initial deterministic evaluators may include:

- project test command success
- production build success
- broken-link detection
- HTML validity
- accessibility checks
- Lighthouse or equivalent local performance checks where available
- bundle-size delta
- visual overflow/regression checks
- copy length and reading level
- presence of required proof/elements
- prohibited-claim detection
- user-supplied rubric

Model-based evaluation is allowed only when:

- the rubric is shown
- model/harness metadata is recorded
- output is labeled estimated
- reasoning is summarized without exposing private chain-of-thought
- the user can override weights and decisions

Never combine incomparable metrics into a magic score without normalization and an explanation.

---

# 15. Integrations roadmap

The MVP must not depend on third-party analytics or SaaS accounts.

Design stable integration boundaries for later support of:

- generic CSV event import
- privacy-friendly analytics
- web analytics
- product analytics
- A/B testing providers
- search performance data
- GitHub repository signals

Do not implement a dozen shallow integrations before the local experiment loop is excellent.

All external mutations require explicit authorization. Read-only analysis must stay read-only.

---

# 16. Safety, permissions, and privacy

Growth agents can modify persuasive product surfaces, so permissions must be explicit.

Provide modes:

```text
ANALYZE_ONLY       no file modifications
DRAFT              create artifacts, do not apply to product
IMPLEMENT          modify only allowed paths in isolated worktrees
APPLY_SELECTED     merge/copy only an explicitly selected variant
```

Never automatically:

- deploy to production
- purchase ads
- send emails or messages
- post to social networks
- change billing or pricing in a live system
- modify production analytics
- merge or push changes to another repository
- access `.env`, credentials, customer data, or production secrets

Those capabilities, if ever added, require scoped integration permissions and an explicit confirmation at the moment of action.

Redact secrets from logs and archives. Add tests for common token and credential patterns.

---

# 17. Quality requirements

Maintain or improve the inherited engineering quality.

Every meaningful subsystem needs:

- unit tests
- integration tests
- migration tests where persistence changes
- failure-path tests
- cancellation and cleanup tests for runs/worktrees
- deterministic fixtures
- documentation
- structured errors
- useful logs without secret leakage

Required recurring validation should include the relevant existing OpenResearch checks plus new GrowthLab checks. Inspect the repository before deciding exact commands.

Do not silence failing tests, weaken assertions, delete coverage, or mark failures ignored merely to make CI green.

The application must recover cleanly from:

- agent process crashes
- invalid model output
- interrupted runs
- failed builds
- stale worktrees
- locked SQLite database
- malformed configuration
- unavailable network
- missing external CLI
- cancellation during streaming
- partially written artifacts

---

# 18. Development phases

Use milestones but keep the product runnable after each one.

## Phase 0 — Foundation audit

- inspect OpenResearch deeply
- run its tests and application
- record architecture and baseline behavior
- verify license obligations
- create GrowthLab repository/remotes
- add attribution
- define migration map

## Phase 1 — Product identity and domain model

- introduce GrowthLab naming and configuration
- implement product workspace and growth-hypothesis entities
- preserve generic OpenResearch primitives
- update CLI entry points progressively
- add database migrations and tests

## Phase 2 — End-to-end landing-page battle

- implement goal composer
- generate structured hypotheses
- create isolated variants
- execute agents
- validate builds/tests
- archive evidence and artifacts
- compare variants transparently
- apply/export selection

## Phase 3 — Killer UX and demo

- polish experiment tree and battle screen
- add deterministic demo product
- create real screenshots and demo video
- build shareable HTML/Markdown report
- produce exceptional README

## Phase 4 — Broader growth playbooks

- positioning
- activation/onboarding
- ethical SEO
- pricing research
- launch planning
- reusable experiment templates

## Phase 5 — Real measurement

- CSV telemetry import first
- baseline and variant metric comparison
- sample-size display
- date-range/provenance display
- cautious statistical analysis with documented assumptions
- integration adapters only after the generic model is stable

Do not build Phase 5 UI with fake data and present it as complete.

---

# 19. Git workflow: commit and push frequently

Work directly on `main` unless a genuinely risky migration requires a temporary local branch. The user explicitly wants frequent visible progress and does not require pull requests for normal work.

After every coherent, tested unit of work:

1. inspect `git status` and the diff
2. run the smallest relevant validation
3. update documentation/progress if needed
4. create a focused commit
5. push to `origin main`
6. verify the push succeeded

Examples of good commits:

```text
chore: establish GrowthLab identity and attribution
docs: map OpenResearch primitives to growth experiments
feat: add product workspace configuration
feat: model growth hypotheses and result provenance
feat: create isolated Growth Battle variants
feat: add transparent rubric evaluation
ui: build Growth Battle comparison screen
test: cover interrupted battle cleanup
docs: add reproducible local demo
```

Prefer many meaningful commits over one giant commit, but do not create empty commits, misleading commits, one-line noise commits, or knowingly broken commits merely to inflate the count.

Never commit:

- secrets
- `.env` values
- API keys
- local databases containing private data
- generated dependency directories
- build output unless intentionally distributed
- private product inputs

Before the first push, verify the remote points exactly to:

```text
git@github.com:OthmaneBlial/GrowthLab.git
```

If SSH is unavailable but authenticated HTTPS is configured, use the safe authenticated alternative.

---

# 20. Progress tracking

Maintain:

```text
docs/ROADMAP.md
docs/PROGRESS.md
```

`PROGRESS.md` should contain:

- current milestone
- completed vertical slices
- work in progress
- next three concrete tasks
- known failures
- important architectural decisions
- latest validation commands/results
- an honest overall completion estimate

The README may show a concise status, but do not turn a subjective percentage into a fake scientific measurement. Clearly label it as an estimate.

Update progress only when reality changes.

---

# 21. Decision rules for autonomous work

You have broad autonomy over architecture, crate selection, migrations, UI structure, test strategy, and sequencing.

Do not stop for trivial decisions that can be resolved by inspecting the repository, running a small experiment, or choosing the safest reversible option.

Stop and ask only when:

- credentials or new external authorization are required
- a destructive operation could affect unrelated user data or repositories
- license compatibility is genuinely unclear
- two product directions would materially change the scope
- a production-facing action is requested but not explicitly authorized

When blocked, continue with independent work that remains safe and useful.

Prefer working software over speculative architecture documents, but use short decision records for choices that future contributors must understand.

---

# 22. Definition of the first credible release

The first release is credible only when a new user can:

- install GrowthLab without Docker
- launch the local dashboard
- load the bundled demo or a real local repository
- define a product and growth goal
- run a three-variant Growth Battle
- observe real agent/run progress
- inspect each worktree's diff, evidence, logs, and artifacts
- see validation failures honestly
- compare candidates through a transparent rubric
- understand whether results are measured, observed, estimated, or simulated
- apply or export a selected candidate safely
- reproduce the demo from documented commands

The repository must also have:

- preserved upstream attribution and license compliance
- passing relevant tests
- no committed secrets
- no Docker requirement
- accurate installation docs
- a short real demo video or GIF
- real screenshots
- a clear roadmap
- contribution and security documentation
- tagged release notes that describe actual capabilities only

---

# 23. Start now

Begin by:

1. verifying repository/authentication state
2. checking whether `OthmaneBlial/GrowthLab` already exists
3. cloning and running OpenResearch
4. reading its instructions and architecture
5. establishing the `origin` and `upstream` remotes safely
6. preserving license and adding attribution
7. writing `docs/openresearch-foundation.md`
8. defining the smallest complete landing-page Growth Battle
9. implementing it vertically
10. committing and pushing each coherent, validated milestone directly to `main`

Do not merely produce another plan after this prompt. Inspect, implement, test, document, commit, and push.

Keep the product honest, local-first, technically serious, visually memorable, and easy to demonstrate.

The end state should feel inevitable:

> Research agents explore hypotheses for science.
>
> **GrowthLab agents explore hypotheses for products.**
