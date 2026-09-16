# Attribution

GrowthLab is built from [alphaXiv/OpenResearch](https://github.com/alphaXiv/OpenResearch),
starting at commit `325eb509dc8e4ca7074568cf0ae1f0f98704eac0` (2026-09-16 audit).
Its upstream Git history, MIT license, and alphaXiv copyright notice are preserved.
The MIT license permits modification and redistribution provided the copyright
and permission notices remain in copies or substantial portions of the software.

GrowthLab's product direction and new growth-specific code are maintained by
Othmane Blial. alphaXiv does not endorse GrowthLab. Upstream trademarks, service
accounts, managed compute, release signing, and usage analytics are not GrowthLab
services.

Generic upstream modules and compatibility commands retain their original names
where renaming would break useful behavior. Attribution is not a claim that the
GrowthLab-specific experience is already implemented.

The static-preview subsystem uses Cloudflare's `lol_html` under BSD-3-Clause;
its copyright and license are retained in
[licenses/lol-html-BSD-3-Clause.txt](licenses/lol-html-BSD-3-Clause.txt) and must
accompany binary distributions. It also uses unmodified `cssparser` under
MPL-2.0 and `url`/`percent-encoding` under MIT or Apache-2.0. Exact versions and
source packages are recorded in Cargo.lock; covered dependency source is available
from [crates.io](https://crates.io/). These dependency licenses do not replace
the preserved OpenResearch MIT notice or imply vendor endorsement. A complete
distribution dependency-notice inventory remains a release packaging gate.
