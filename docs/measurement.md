# Local CSV measurement

GrowthLab can summarize a user-supplied telemetry export without contacting an
analytics provider. From the dashboard Home screen, choose **Telemetry CSV**
and press **Summarize locally**; the browser keeps the selected rows in memory.
For a repeatable terminal report, use:

```sh
growthlab measure --csv ./telemetry.csv --format markdown
```

The CSV is one numeric observation per row. `variant` and `value` are required;
`metric`, `distribution` and `timestamp` are optional. The default baseline value is
`baseline`, and another value can be selected with `--baseline`:

```csv
timestamp,distribution,variant,metric,value
2026-09-01,search,baseline,qualified_signup,12
2026-09-02,search,baseline,qualified_signup,10
2026-09-03,search,outcome-hero,qualified_signup,15
```

The command reports each metric/distribution/variant mean, sample size, total, the supplied
baseline mean, arithmetic difference and relative change. When a distribution
column is absent, every row is assigned the `all` distribution. Comparisons stay
within the same metric and distribution, so a search baseline is never mixed
with a social baseline. Groups with at least
two rows also include a sample standard deviation and an exploratory 95% mean
interval. A variant/baseline difference interval is shown only when both groups
have at least two rows. It also preserves the first and last timestamp labels
when a timestamp column is present. Use
`--metric qualified_signup` to inspect one metric from a multi-metric export.
Column names can be changed with `--variant-column`, `--value-column`,
`--metric-column`, `--distribution-column` and `--timestamp-column`.

The interval method is a 95% normal approximation over independent observations
(z = 1.96). It is deliberately descriptive: small samples, assignment quality,
non-normal outcomes and repeated measurements can make the interval unsuitable.
No p-value, significance decision or automatic winner is produced; a missing
interval is shown when the sample-size requirement is not met.

Results are labelled **MEASURED** because they are calculated from numeric rows
the user supplied. That label does not verify the analytics provider, identity,
experiment assignment or data quality. The summary is descriptive: it does not
claim causality, statistical significance, ranking lift, traffic, revenue or
conversion lift, and it never declares a winner. Missing timestamps produce an
explicit warning; missing baseline rows leave a variant without a comparison.

The importer is local and bounded: it accepts a regular UTF-8 file up to 8 MiB,
rejects symlinks, limits rows and columns, handles quoted CSV fields, and makes
no network request. The JSON output is suitable for downstream local analysis; no provider integration
is enabled by either path.
