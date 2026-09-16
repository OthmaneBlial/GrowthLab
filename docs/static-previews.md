# Archived static previews

For a static landing page, opt in before preparing a battle:

```yaml
static_preview:
  root: website
  entry: index.html
```

Both paths are portable relative paths; `entry` is relative to `root` and must
end in `.html`. The CLI can create this section with `init --preview-root website
--preview-entry index.html`; the entry defaults to `index.html`. Review and commit
the configuration using the product's Git workflow, then import it. The bundled
PatchKit demo opts in automatically. Existing frozen battles and archives stay
unchanged.

After implementation is committed, Rust reads that candidate's Git objects and
archives permitted static source files, a self-contained HTML document and
metadata. It never uses mutable checkout contents for the preview. Supported
files are HTML, CSS, PNG, JPEG, GIF, WebP, WOFF and WOFF2. Sources must also pass
the battle's allowed/denied-path checks. Limits are 128 files, 4 MiB per file,
8 MiB total source and a 4 MiB packaged document. Resource expansion is bounded.

The **Static preview** inspection tab displays that document at a real desktop
1280 × 900 or phone 390 × 844 CSS viewport, scaled to fit the inspector. When a
local Chromium-compatible browser is available, the battle engine also captures
the sealed document at 1280 × 900 and 390 × 844 as
`preview/screenshot-desktop.png` and `preview/screenshot-phone.png`. Their
dimensions, sizes and SHA-256 values are stored in `preview/metadata.json` and
verified with the archive. These PNGs are render artifacts, **not a visual
regression result**, accessibility audit or measured growth outcome. Preview
availability does not change validation eligibility; a failed candidate can
still have an inspectable page.

Phone-sized dashboards initially choose the phone viewport. Controls let you
inspect either size. The inert preview shows one viewport; use captured source
text to inspect content below the fold.

Scripts, frames, forms, event handlers and navigation are removed. Local styles,
images and fonts are embedded as data resources; external, unsupported and cyclic
references are omitted. A restrictive archived CSP and an opaque, inert iframe
with an empty sandbox prevent execution and interaction. HTML reaches the UI as
verified JSON data; ordinary artifact viewing remains escaped text. The server's
HTML response protections are preserved.

Dynamic applications, SVG assets and CSS `image-set()` string sources are not
reproduced. Browser rendering and installed system fonts can vary. Missing,
denied, oversized or unsafe-to-package input records an unavailable preview;
missing browsers or failed local renders leave `screenshots` empty and no
screenshot is fabricated. Omitted resource counts and limitations are
inspectable with the candidate commit, document SHA-256 and archive SHA-256.
Artifact readers, comparison and recovery verify the metadata, archived source,
document and PNG hashes.

Private reports do not include these source documents or renderings by default.
The dashboard exposes a verified desktop PNG only inside the local candidate
inspector; explicit visual report disclosure remains pending.

Real manual captures show the [desktop candidate](screenshots/growth-preview-desktop.jpg),
[deliberately failed candidate](screenshots/growth-preview-failed.jpg) and
[phone preview](screenshots/growth-preview-phone.jpg). Their
[capture metadata](screenshots/growth-preview-capture.json) ties each JPEG to its
fictional candidate commit and verified source document. These documentation
images are separate from the immutable run archive. New battle archives may
also contain automatic desktop and phone PNGs when local browser capture
succeeds. The inspector displays the capture matching the selected viewport.
