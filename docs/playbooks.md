# Growth playbooks

GrowthLab includes a small catalog of reusable role contracts. A playbook gives an agent or a human reviewer a focused question, expected outputs and explicit guardrails. It is a starting template, not external research and not an outcome result.

The local dashboard loads the same catalog from `GET /api/growth/playbooks`. No provider or analytics request is made when the catalog is read.

| Role | Focus | Useful outputs |
| --- | --- | --- |
| Strategist | Choose a tractable growth question | Prioritized brief, hypothesis tree, metric and decision rule |
| Researcher | Collect permitted facts | Evidence records, claim-to-source map, open questions |
| Positioning | Make value easy to understand | Value propositions, page messages, claim checklist |
| Conversion | Remove uncertainty before an action | Page hierarchy, CTA alternatives, validation checklist |
| SEO | Match useful pages to search intent | Search-intent brief, on-page suggestions, local audit steps |
| Onboarding | Shorten the path to first value | First-value map, quick-start variants, activation events |
| Pricing | Explore packaging safely | Value-metric hypotheses, non-production variants, billing guardrails |
| Launch | Prepare a truthful channel narrative | Announcement drafts, proof links, launch checklist |
| Evaluator | Apply a stable, inspectable rubric | Evaluation rows, observed checks, confidence rationale |
| Skeptic | Try to falsify the claim | Counter-hypotheses, evidence gaps, inconclusive or revise recommendation |

## Using a playbook

1. Choose the role that matches the question, such as **SEO** for a search-intent page or **Onboarding** for time to first value.
2. Fill the questions from committed product facts and user-supplied evidence. Mark missing facts as unknown.
3. Review the expected outputs and guardrails before authorizing any file change.
4. Turn a concrete proposal into a three-variant Growth Battle when implementation is justified.
5. Keep the result provenance visible: deterministic checks are **OBSERVED**, rubric judgments are **ESTIMATED**, and outcome data is **MEASURED** only when it comes from the user's telemetry.

Playbooks never authorize deployment, live billing changes, messages, scraping, ranking claims or automatic publishing. They work with the existing permission modes and the same evidence, diff and approval boundaries as every other GrowthLab experiment.
