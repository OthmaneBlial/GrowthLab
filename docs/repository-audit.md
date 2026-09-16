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

The Home screen and `POST /api/growth/repository-audit` expose the same
read-only path. Invalid hosts, credentials, query strings, fragments, `.git`
suffixes and non-HTTPS URLs are refused before any network request.
