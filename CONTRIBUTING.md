# Contributing to GrowthLab

Start with [current progress](docs/PROGRESS.md), the [roadmap](docs/ROADMAP.md),
and the [foundation audit](docs/openresearch-foundation.md). The full product
contract is [SPEC.md](docs/SPEC.md). The project is under active development;
document working behavior separately from planned behavior.

## Local setup

Install Git, stable Rust, Node.js 22+ and pnpm 10. No Docker is needed.

```sh
pnpm -C ui install --frozen-lockfile
node scripts/dev-slot.mjs start --db empty
node scripts/dev-slot.mjs status
```

Use an empty isolated dev slot for fixtures. Never copy private product inputs,
real databases, credentials, customer data, or prompts into public test evidence.
Stop the slot with `node scripts/dev-slot.mjs stop`.

## Checks

Follow `.github/workflows/ci.yml`, including:

```sh
node ui/scripts/check-i18n.mjs
node ui/scripts/check-styles.mjs
node --test scripts/dev-slot.test.mjs
pnpm -C ui exec paraglide-js compile --silent --emit-ts-declarations
pnpm -C ui typecheck
pnpm -C ui test
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked
```

After UI edits, run `pnpm -C ui build` and commit the regenerated `ui/dist`:
release binaries embed these assets. Inspect actual rendered interactions,
keyboard focus, responsive layout, console errors and horizontal overflow.

Cover meaningful domain changes with unit, integration, migration, failure,
cancellation and cleanup tests. Preserve inherited assertions; a failed check
is evidence to fix or report, not something to suppress. Use deterministic
fixtures and isolated stores instead of mutating a contributor's defaults.

## Contribution boundaries

Every result needs provenance. Real deterministic checks are OBSERVED; model
rubrics are ESTIMATED; demo replay proposals are SIMULATED; real connected
telemetry is MEASURED; missing evidence is UNTESTED. Proxy evaluations recommend
candidates; they do not establish conversion lift or statistical significance.

Keep generic upstream primitives and MIT attribution. Prefer complete local
workflows over shallow provider integrations. Preserve a shared source snapshot
and validation contract across competing variants. External mutations require
explicit scoped user authorization. Do not add spam, deceptive SEO, dark
patterns, automatic publication or credential/customer-data collection.

Open a focused issue for a bug or proposal using the templates. Pull requests
should state the concrete resulting behavior, relevant validation, and remaining
limits. Never include confidential product context or tokens in an issue.
