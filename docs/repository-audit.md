# Public repository review

GrowthLab can inspect a public GitHub repository before you decide what product
context to bring into a local workspace:

```sh
growthlab repo-audit --url https://github.com/owner/product
growthlab repo-audit --url https://github.com/owner/product --format markdown
```

The auditor validates the canonical HTTPS page URL, makes one unauthenticated
request to GitHub's public repository metadata endpoint, bounds the UTF-8 JSON
response to 512 KiB, follows no redirects and sends no cookies or credentials.
It reports the public description, default branch, license and archive/fork
flags as **OBSERVED** metadata.

The request never clones or executes source code, stores a token, registers a
workspace or changes the repository. Metadata is not product evidence and does
not establish stars, traffic, adoption, rankings, conversion or revenue. For
implementation, review a checkout containing a committed `growthlab.yaml` and
use `growthlab workspace import`.

When you explicitly want a local checkout, use the separate import command with
a new destination. A repository that already contains `growthlab.yaml` is
imported as-is:

```sh
growthlab workspace import-url \
  --url https://github.com/owner/product \
  --path ./product \
  --shallow
```

If the public repository has no GrowthLab contract, add `--name`, `--audience`
and `--goal` (plus `--allow` and `--validate` when implementation mode is
needed). GrowthLab writes and commits that configuration only inside the new
local checkout. It never pushes, deploys, executes repository code or sends a
provider request. The destination must not already exist, which prevents an
accidental overwrite.

The Home screen and `POST /api/growth/repository-audit` expose the same
read-only path. `POST /api/growth/repository-import` exposes the explicit local
checkout flow. Invalid hosts, credentials, query strings, fragments, `.git`
suffixes and non-HTTPS URLs are refused before any network request.
