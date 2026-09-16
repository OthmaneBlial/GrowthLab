# SEO page audit

GrowthLab can review one local HTML page or one public HTTPS page before you
put it into a battle. Both paths print the same explainable page-hygiene rubric
used by the comparison dashboard.

```sh
growthlab seo-audit --html ./website/index.html
growthlab seo-audit --html ./website/index.html --format markdown
growthlab seo-audit --url https://example.com/pricing --format markdown
```

The URL form is read-only and deliberately narrow: it checks the same-origin
`robots.txt`, makes one page request, follows no redirect, sends no cookies or
credentials, accepts HTTPS on the default port only, and bounds the HTML body
to 4 MiB. A disallow rule, non-HTML response, unavailable policy file or
non-UTF-8 body refuses the analysis. No crawl, search API, analytics provider
or external mutation is used.

The JSON result includes the input path, the rubric id
`seo-page-hygiene-v1`, the total score and eight inspectable dimensions:
title, description, headings, document language, useful copy, canonical URL,
useful links and image descriptions. Markdown is intended for a quick review
or a local issue description.

When a dimension is partial or missing, the output adds a concrete next step,
such as adjusting the title range, adding a description or giving images useful
alt text. A page that passes every dimension reports that no structural gaps
were found; it still does not imply search performance.

Every score is labeled **ESTIMATED**. The audit checks returned or supplied HTML
only; it does not crawl, index, rank or measure traffic. It cannot predict
search position or conversion, and it does not replace accessibility,
performance, structured-data, backlink or real-user measurement. A missing or
weak signal is evidence for a page review, not evidence of a business outcome.

The local input must be a regular file of at most 4 MiB. Symlinks, directories,
non-UTF-8 bytes and larger files are refused so a quick audit cannot silently
read an unexpected target. URL retrieval timestamps and the robots decision are
included in JSON/Markdown output so a reviewer can see the exact access scope.
