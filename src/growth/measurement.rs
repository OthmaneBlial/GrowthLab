//! Local, user-supplied telemetry summaries.
//!
//! This module deliberately does not fetch analytics or infer causality. It
//! accepts a small RFC-4180 CSV file, computes descriptive summaries and marks
//! the values as measured from a user-supplied source.

use std::path::Path;

use serde::Serialize;

use crate::error::{anyhow, Result};

const MAX_CSV_BYTES: u64 = 8 * 1024 * 1024;
const MAX_ROWS: usize = 100_000;
const MAX_COLUMNS: usize = 64;
const MAX_FIELD_BYTES: usize = 4096;
const NORMAL_95_Z: f64 = 1.959_963_984_540_054;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum MeasurementFormat {
    Json,
    Markdown,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DateRange {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeasurementInterval {
    pub lower: f64,
    pub upper: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeasurementAnalysis {
    pub method: &'static str,
    pub confidence_level_percent: u8,
    pub assumptions: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeasurementComparison {
    pub baseline_mean: f64,
    pub difference: f64,
    pub relative_change_percent: Option<f64>,
    pub difference_interval_95: Option<MeasurementInterval>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeasurementGroup {
    pub metric: String,
    pub variant: String,
    pub sample_size: usize,
    pub total: f64,
    pub mean: f64,
    pub sample_stddev: Option<f64>,
    pub mean_interval_95: Option<MeasurementInterval>,
    pub comparison: Option<MeasurementComparison>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeasurementReport {
    pub path: String,
    pub scope: String,
    pub provenance: &'static str,
    pub rows_read: usize,
    pub rows_included: usize,
    pub baseline_variant: String,
    pub date_range: Option<DateRange>,
    pub analysis: MeasurementAnalysis,
    pub groups: Vec<MeasurementGroup>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct Observation {
    metric: String,
    variant: String,
    value: f64,
    timestamp: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct ColumnIndexes {
    variant: usize,
    value: usize,
    metric: Option<usize>,
    timestamp: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default)]
struct Accumulator {
    sample_size: usize,
    total: f64,
    sum_squares: f64,
}

impl Accumulator {
    fn add(&mut self, value: f64) -> Result<()> {
        self.sample_size += 1;
        self.total += value;
        self.sum_squares += value * value;
        if !self.total.is_finite() || !self.sum_squares.is_finite() {
            return Err(anyhow!(
                "Measurement totals exceed the supported numeric range"
            ));
        }
        Ok(())
    }

    fn sample_stddev(self) -> Option<f64> {
        if self.sample_size < 2 {
            return None;
        }
        let n = self.sample_size as f64;
        let numerator = self.sum_squares - self.total * self.total / n;
        if !numerator.is_finite() {
            return None;
        }
        Some((numerator.max(0.0) / (n - 1.0)).sqrt())
    }
}

#[derive(Debug, Clone, Copy)]
struct GroupStats {
    sample_size: usize,
    total: f64,
    mean: f64,
    sample_stddev: Option<f64>,
}

fn mean_interval(stats: GroupStats) -> Option<MeasurementInterval> {
    let standard_error = stats.sample_stddev? / (stats.sample_size as f64).sqrt();
    let margin = NORMAL_95_Z * standard_error;
    let lower = stats.mean - margin;
    let upper = stats.mean + margin;
    (lower.is_finite() && upper.is_finite()).then_some(MeasurementInterval { lower, upper })
}

fn difference_interval(variant: GroupStats, baseline: GroupStats) -> Option<MeasurementInterval> {
    if variant.sample_size < 2 || baseline.sample_size < 2 {
        return None;
    }
    let variant_variance = variant.sample_stddev?.powi(2);
    let baseline_variance = baseline.sample_stddev?.powi(2);
    let standard_error = (variant_variance / variant.sample_size as f64
        + baseline_variance / baseline.sample_size as f64)
        .sqrt();
    let margin = NORMAL_95_Z * standard_error;
    let difference = variant.mean - baseline.mean;
    let lower = difference - margin;
    let upper = difference + margin;
    (lower.is_finite() && upper.is_finite()).then_some(MeasurementInterval { lower, upper })
}

pub fn read_csv(path: &Path) -> Result<String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| anyhow!("Measurement CSV input was not found"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(anyhow!(
            "Measurement CSV input must be a regular local file, not a directory or symlink"
        ));
    }
    if metadata.len() > MAX_CSV_BYTES {
        return Err(anyhow!("Measurement CSV input must be at most 8 MiB"));
    }
    let bytes =
        std::fs::read(path).map_err(|_| anyhow!("Measurement CSV input could not be read"))?;
    if bytes.contains(&0) {
        return Err(anyhow!("Measurement CSV input contains a NUL byte"));
    }
    String::from_utf8(bytes).map_err(|_| anyhow!("Measurement CSV input must be UTF-8"))
}

/// Parse RFC-4180-style records without accepting malformed quoting. A final
/// newline is optional; embedded newlines are supported only inside quotes.
fn parse_csv(input: &str) -> Result<Vec<Vec<String>>> {
    if input.is_empty() {
        return Err(anyhow!("Measurement CSV input is empty"));
    }
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut after_quote = false;
    let mut chars = input.chars().peekable();

    while let Some(character) = chars.next() {
        if quoted {
            match character {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => {
                    quoted = false;
                    after_quote = true;
                }
                _ => field.push(character),
            }
            if field.len() > MAX_FIELD_BYTES {
                return Err(anyhow!("Measurement CSV field exceeds 4096 bytes"));
            }
            continue;
        }

        if after_quote {
            match character {
                ',' => {
                    row.push(std::mem::take(&mut field));
                    after_quote = false;
                }
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                    after_quote = false;
                }
                '\r' if chars.peek() == Some(&'\n') => {
                    chars.next();
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                    after_quote = false;
                }
                _ => {
                    return Err(anyhow!(
                        "Measurement CSV has characters after a quoted field"
                    ))
                }
            }
            continue;
        }

        match character {
            '"' if field.is_empty() => quoted = true,
            ',' => row.push(std::mem::take(&mut field)),
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            '\r' if chars.peek() == Some(&'\n') => {
                chars.next();
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            '\r' => return Err(anyhow!("Measurement CSV uses a bare carriage return")),
            _ => field.push(character),
        }
        if field.len() > MAX_FIELD_BYTES {
            return Err(anyhow!("Measurement CSV field exceeds 4096 bytes"));
        }
        if rows.len() > MAX_ROWS + 1 {
            return Err(anyhow!(
                "Measurement CSV contains more than 100000 data rows"
            ));
        }
    }

    if quoted {
        return Err(anyhow!("Measurement CSV has an unterminated quoted field"));
    }
    if after_quote || !row.is_empty() || !field.is_empty() {
        row.push(field);
        rows.push(row);
    }
    while rows
        .last()
        .is_some_and(|last| last.len() == 1 && last[0].is_empty())
    {
        rows.pop();
    }
    if rows.is_empty() {
        return Err(anyhow!("Measurement CSV has no header row"));
    }
    let width = rows[0].len();
    if width == 0 || width > MAX_COLUMNS || rows[0].iter().any(|cell| cell.trim().is_empty()) {
        return Err(anyhow!(
            "Measurement CSV header must contain 1–64 nonempty columns"
        ));
    }
    if rows.iter().any(|record| record.len() != width) {
        return Err(anyhow!(
            "Measurement CSV rows do not have a consistent column count"
        ));
    }
    Ok(rows)
}

fn column(headers: &[String], requested: &str, required: bool) -> Result<Option<usize>> {
    if requested.trim().is_empty() || requested.len() > 128 {
        return Err(anyhow!("Measurement column names must be 1–128 bytes"));
    }
    let requested = requested.trim();
    let matches = headers
        .iter()
        .enumerate()
        .filter(|(_, header)| header.trim().eq_ignore_ascii_case(requested))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        return Err(anyhow!("Measurement CSV contains duplicate column names"));
    }
    match matches.first().copied() {
        Some(index) => Ok(Some(index)),
        None if required => Err(anyhow!(
            "Measurement CSV is missing the '{requested}' column"
        )),
        None => Ok(None),
    }
}

fn indexes(
    headers: &[String],
    variant_column: &str,
    value_column: &str,
    metric_column: &str,
    timestamp_column: &str,
) -> Result<ColumnIndexes> {
    for (index, header) in headers.iter().enumerate() {
        if headers
            .iter()
            .skip(index + 1)
            .any(|other| other.trim().eq_ignore_ascii_case(header.trim()))
        {
            return Err(anyhow!("Measurement CSV contains duplicate column names"));
        }
    }
    let variant = column(headers, variant_column, true)?.expect("required column");
    let value = column(headers, value_column, true)?.expect("required column");
    Ok(ColumnIndexes {
        variant,
        value,
        metric: column(headers, metric_column, false)?,
        timestamp: column(headers, timestamp_column, false)?,
    })
}

fn bounded_text(value: &str, label: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 256 {
        return Err(anyhow!("Measurement {label} values must be 1–256 bytes"));
    }
    Ok(value.to_owned())
}

fn optional_text(value: &str, label: &str) -> Result<Option<String>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > 256 {
        return Err(anyhow!("Measurement {label} values must be 1–256 bytes"));
    }
    Ok(Some(value.to_owned()))
}

fn observations(
    rows: &[Vec<String>],
    indexes: ColumnIndexes,
    metric_filter: Option<&str>,
) -> Result<Vec<Observation>> {
    let mut output = Vec::with_capacity(rows.len().saturating_sub(1));
    for (row_index, row) in rows.iter().skip(1).enumerate() {
        let variant = bounded_text(&row[indexes.variant], "variant")?;
        let metric = indexes
            .metric
            .map(|index| bounded_text(&row[index], "metric"))
            .transpose()?
            .unwrap_or_else(|| "primary".into());
        if metric_filter.is_some_and(|filter| !metric.eq_ignore_ascii_case(filter.trim())) {
            continue;
        }
        let value = row[indexes.value].trim().parse::<f64>().map_err(|_| {
            anyhow!(
                "Measurement value on CSV row {} is not numeric",
                row_index + 2
            )
        })?;
        if !value.is_finite() {
            return Err(anyhow!(
                "Measurement value on CSV row {} is not finite",
                row_index + 2
            ));
        }
        let timestamp = indexes
            .timestamp
            .map(|index| optional_text(&row[index], "timestamp"))
            .transpose()?
            .flatten();
        output.push(Observation {
            metric,
            variant,
            value,
            timestamp,
        });
    }
    if output.is_empty() {
        return Err(anyhow!(
            "Measurement CSV contains no observations after filtering"
        ));
    }
    Ok(output)
}

fn summarize(
    path: &Path,
    rows_read: usize,
    observations: &[Observation],
    baseline_variant: &str,
    has_timestamp: bool,
) -> Result<MeasurementReport> {
    let baseline_variant = bounded_text(baseline_variant, "baseline variant")?;
    let mut accumulators = std::collections::BTreeMap::<(String, String), Accumulator>::new();
    let mut dates = observations
        .iter()
        .filter_map(|observation| observation.timestamp.as_deref());
    let date_range = dates.next().map(|first| {
        let (from, to) = observations
            .iter()
            .filter_map(|observation| observation.timestamp.as_deref())
            .fold((first.to_owned(), first.to_owned()), |(from, to), date| {
                (from.min(date.to_owned()), to.max(date.to_owned()))
            });
        DateRange { from, to }
    });
    for observation in observations {
        let key = (observation.metric.clone(), observation.variant.clone());
        accumulators
            .entry(key)
            .or_default()
            .add(observation.value)?;
    }
    let stats = accumulators
        .iter()
        .map(|((metric, variant), accumulator)| {
            let mean = accumulator.total / accumulator.sample_size as f64;
            (
                (metric.clone(), variant.clone()),
                GroupStats {
                    sample_size: accumulator.sample_size,
                    total: accumulator.total,
                    mean,
                    sample_stddev: accumulator.sample_stddev(),
                },
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut warnings = Vec::new();
    if !has_timestamp {
        warnings.push("No timestamp column was supplied; date range is unavailable.".into());
    } else if date_range.is_none() {
        warnings.push(
            "The timestamp column contained no nonempty values; date range is unavailable.".into(),
        );
    }
    let mut groups = Vec::with_capacity(accumulators.len());
    for ((metric, variant), _) in accumulators {
        let group_stats = stats
            .get(&(metric.clone(), variant.clone()))
            .copied()
            .expect("stats for every accumulator");
        let comparison = stats
            .get(&(metric.clone(), baseline_variant.clone()))
            .copied()
            .map(|baseline_stats| MeasurementComparison {
                baseline_mean: baseline_stats.mean,
                difference: group_stats.mean - baseline_stats.mean,
                relative_change_percent: (baseline_stats.mean != 0.0).then_some(
                    (group_stats.mean - baseline_stats.mean) / baseline_stats.mean.abs() * 100.0,
                ),
                difference_interval_95: (variant != baseline_variant)
                    .then(|| difference_interval(group_stats, baseline_stats))
                    .flatten(),
            });
        if variant != baseline_variant && comparison.is_none() {
            warnings.push(format!(
                "Metric '{metric}' has no '{baseline_variant}' baseline; its variants are shown without comparison."
            ));
        }
        groups.push(MeasurementGroup {
            metric,
            variant,
            sample_size: group_stats.sample_size,
            total: group_stats.total,
            mean: group_stats.mean,
            sample_stddev: group_stats.sample_stddev,
            mean_interval_95: mean_interval(group_stats),
            comparison,
        });
    }
    Ok(MeasurementReport {
        path: path.to_string_lossy().into_owned(),
        scope: "User-supplied local CSV; no network request".into(),
        provenance: "MEASURED",
        rows_read,
        rows_included: observations.len(),
        baseline_variant,
        date_range,
        analysis: MeasurementAnalysis {
            method: "normal_approximation",
            confidence_level_percent: 95,
            assumptions: vec![
                "Intervals are descriptive and use an independent-observation normal approximation.",
                "A mean interval requires at least two observations in that group.",
                "A difference interval requires at least two observations in both variant and baseline groups.",
                "Intervals are not a significance test, causal estimate or winner decision.",
            ],
        },
        groups,
        warnings,
    })
}

pub fn import(
    path: &Path,
    baseline_variant: &str,
    metric: Option<&str>,
    variant_column: &str,
    value_column: &str,
    metric_column: &str,
    timestamp_column: &str,
) -> Result<MeasurementReport> {
    if let Some(metric) = metric {
        bounded_text(metric, "metric filter")?;
    }
    let input = read_csv(path)?;
    let rows = parse_csv(&input)?;
    let indexes = indexes(
        &rows[0],
        variant_column,
        value_column,
        metric_column,
        timestamp_column,
    )?;
    let observations = observations(&rows, indexes, metric)?;
    summarize(
        path,
        rows.len().saturating_sub(1),
        &observations,
        baseline_variant,
        indexes.timestamp.is_some(),
    )
}

fn markdown_number(value: f64) -> String {
    format!("{value:.6}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

fn markdown_interval(interval: Option<&MeasurementInterval>) -> String {
    interval
        .map(|interval| {
            format!(
                "[{}, {}]",
                markdown_number(interval.lower),
                markdown_number(interval.upper)
            )
        })
        .unwrap_or_else(|| "—".into())
}

pub fn markdown(report: &MeasurementReport) -> String {
    let cell = |value: &str| value.replace('|', "\\|").replace('\n', " ");
    let mut output = format!(
        "# Local growth measurement\n\n- File: `{}`\n- Scope: {}\n- Provenance: **{}**\n- Rows: {} read, {} included\n- Baseline variant: `{}`\n\n",
        report.path,
        report.scope,
        report.provenance,
        report.rows_read,
        report.rows_included,
        report.baseline_variant
    );
    output.push_str("## Observations\n\n| Metric | Variant | Mean | Sample size | Sample SD | Mean 95% interval | Baseline mean | Difference | Difference 95% interval | Relative change |\n| --- | --- | ---: | ---: | ---: | --- | ---: | ---: | --- | ---: |\n");
    for group in &report.groups {
        let (baseline, difference, difference_interval, relative) = group
            .comparison
            .as_ref()
            .map(|comparison| {
                (
                    markdown_number(comparison.baseline_mean),
                    markdown_number(comparison.difference),
                    markdown_interval(comparison.difference_interval_95.as_ref()),
                    comparison
                        .relative_change_percent
                        .map(|value| format!("{}%", markdown_number(value)))
                        .unwrap_or_else(|| "—".into()),
                )
            })
            .unwrap_or_else(|| ("—".into(), "—".into(), "—".into(), "—".into()));
        output.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            cell(&group.metric),
            cell(&group.variant),
            markdown_number(group.mean),
            group.sample_size,
            group
                .sample_stddev
                .map(markdown_number)
                .unwrap_or_else(|| "—".into()),
            markdown_interval(group.mean_interval_95.as_ref()),
            baseline,
            difference,
            difference_interval,
            relative
        ));
    }
    output.push_str("\n## Exploratory interval method\n\n");
    output.push_str(&format!(
        "{}% normal approximation. Intervals are descriptive only; they do not establish statistical significance, causality or a winner.\n",
        report.analysis.confidence_level_percent
    ));
    for assumption in &report.analysis.assumptions {
        output.push_str(&format!("- {assumption}\n"));
    }
    output.push_str("\n## Date range\n\n");
    if let Some(date_range) = &report.date_range {
        output.push_str(&format!("{} → {}\n", date_range.from, date_range.to));
    } else {
        output.push_str("Unavailable: add a timestamp column to the CSV.\n");
    }
    output.push_str("\n## Limits\n\n- This is a descriptive summary of user-supplied numeric rows; it does not establish causality, statistical significance, ranking lift, traffic, revenue or conversion lift.\n- Relative changes are arithmetic comparisons to the supplied baseline mean; they are not a winner decision.\n");
    if !report.warnings.is_empty() {
        output.push_str("\n## Warnings\n\n");
        for warning in &report.warnings {
            output.push_str(&format!("- {warning}\n"));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_fixture(contents: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("growthlab-measure-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("metrics.csv");
        std::fs::write(&path, contents).unwrap();
        (dir, path)
    }

    #[test]
    fn imports_groups_and_compares_to_baseline() {
        let (dir, path) = write_fixture(
            "timestamp,variant,metric,value\n2026-09-01,baseline,signup,10\n2026-09-02,baseline,signup,20\n2026-09-03,hero,signup,15\n",
        );
        let report = import(
            &path,
            "baseline",
            None,
            "variant",
            "value",
            "metric",
            "timestamp",
        )
        .unwrap();
        assert_eq!(report.provenance, "MEASURED");
        assert_eq!(report.rows_read, 3);
        assert_eq!(report.rows_included, 3);
        assert_eq!(report.date_range.as_ref().unwrap().from, "2026-09-01");
        let hero = report
            .groups
            .iter()
            .find(|group| group.variant == "hero")
            .unwrap();
        assert_eq!(hero.sample_size, 1);
        assert_eq!(hero.mean, 15.0);
        assert_eq!(hero.sample_stddev, None);
        assert_eq!(hero.mean_interval_95, None);
        assert_eq!(hero.comparison.as_ref().unwrap().baseline_mean, 15.0);
        assert_eq!(
            hero.comparison.as_ref().unwrap().relative_change_percent,
            Some(0.0)
        );
        assert_eq!(
            hero.comparison.as_ref().unwrap().difference_interval_95,
            None
        );
        let baseline = report
            .groups
            .iter()
            .find(|group| group.variant == "baseline")
            .unwrap();
        assert_eq!(baseline.sample_stddev, Some(7.0710678118654755));
        let interval = baseline.mean_interval_95.as_ref().unwrap();
        assert!((interval.lower - 5.2).abs() < 0.01);
        assert!((interval.upper - 24.8).abs() < 0.01);
        assert_eq!(report.analysis.method, "normal_approximation");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn computes_difference_interval_only_when_both_groups_have_repeated_observations() {
        let (dir, path) = write_fixture(
            "variant,value
base,10
base,20
hero,14
hero,22
",
        );
        let report = import(
            &path,
            "base",
            None,
            "variant",
            "value",
            "metric",
            "timestamp",
        )
        .unwrap();
        let hero = report
            .groups
            .iter()
            .find(|group| group.variant == "hero")
            .unwrap();
        let interval = hero
            .comparison
            .as_ref()
            .unwrap()
            .difference_interval_95
            .as_ref()
            .unwrap();
        assert!((interval.lower + 9.55).abs() < 0.02);
        assert!((interval.upper - 15.55).abs() < 0.02);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn parses_quoted_commas_and_filters_metric() {
        let (dir, path) = write_fixture(
            "variant,metric,value\n\"hero, proof\",signup,4\nbase,retention,9\nbase,signup,2\n",
        );
        let report = import(
            &path,
            "base",
            Some("signup"),
            "variant",
            "value",
            "metric",
            "timestamp",
        )
        .unwrap();
        assert_eq!(report.groups.len(), 2);
        assert!(report
            .groups
            .iter()
            .any(|group| group.variant == "hero, proof"));
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("timestamp")));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_inconsistent_csv_rows() {
        let (dir, path) = write_fixture("variant,value\nbase,1,extra\n");
        assert!(import(
            &path,
            "base",
            None,
            "variant",
            "value",
            "metric",
            "timestamp",
        )
        .is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
