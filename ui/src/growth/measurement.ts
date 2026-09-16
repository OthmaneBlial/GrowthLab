export interface MeasurementInterval {
  lower: number;
  upper: number;
}

export interface MeasurementAnalysis {
  method: "normal_approximation";
  confidenceLevelPercent: 95;
  assumptions: string[];
}

export interface MeasurementComparison {
  baselineMean: number;
  difference: number;
  relativeChangePercent: number | null;
  differenceInterval95: MeasurementInterval | null;
}

export interface MeasurementGroup {
  metric: string;
  variant: string;
  sampleSize: number;
  total: number;
  mean: number;
  sampleStddev: number | null;
  meanInterval95: MeasurementInterval | null;
  comparison: MeasurementComparison | null;
}

export interface LocalMeasurementReport {
  provenance: "MEASURED";
  rowsRead: number;
  rowsIncluded: number;
  baselineVariant: string;
  dateRange: { from: string; to: string } | null;
  analysis: MeasurementAnalysis;
  groups: MeasurementGroup[];
  warnings: string[];
}

const MAX_BYTES = 8 * 1024 * 1024;
const MAX_ROWS = 100_000;
const MAX_COLUMNS = 64;
const MAX_FIELD_BYTES = 4096;
const NORMAL_95_Z = 1.959963984540054;
const encoder = new TextEncoder();

function fail(message: string): never { throw new Error(message); }

function parseCsv(input: string): string[][] {
  if (!input) fail("Measurement CSV input is empty.");
  if (encoder.encode(input).byteLength > MAX_BYTES) fail("Measurement CSV input must be at most 8 MiB.");
  const rows: string[][] = [];
  let row: string[] = [];
  let field = "";
  let quoted = false;
  let afterQuote = false;
  for (let index = 0; index < input.length; index += 1) {
    const character = input[index];
    const next = input[index + 1];
    if (quoted) {
      if (character === '"' && next === '"') { field += '"'; index += 1; }
      else if (character === '"') { quoted = false; afterQuote = true; }
      else field += character;
      if (encoder.encode(field).byteLength > MAX_FIELD_BYTES) fail("Measurement CSV field exceeds 4096 bytes.");
      continue;
    }
    if (afterQuote) {
      if (character === ",") { row.push(field); field = ""; afterQuote = false; }
      else if (character === "\n") { row.push(field); rows.push(row); row = []; field = ""; afterQuote = false; }
      else if (character === "\r" && next === "\n") { row.push(field); rows.push(row); row = []; field = ""; afterQuote = false; index += 1; }
      else fail("Measurement CSV has characters after a quoted field.");
      continue;
    }
    if (character === '"' && field === "") quoted = true;
    else if (character === ",") { row.push(field); field = ""; }
    else if (character === "\n") { row.push(field); rows.push(row); row = []; field = ""; }
    else if (character === "\r" && next === "\n") { row.push(field); rows.push(row); row = []; field = ""; index += 1; }
    else if (character === "\r") fail("Measurement CSV uses a bare carriage return.");
    else if (character === '"') fail("Measurement CSV has an unexpected quote.");
    else field += character;
    if (encoder.encode(field).byteLength > MAX_FIELD_BYTES) fail("Measurement CSV field exceeds 4096 bytes.");
    if (rows.length > MAX_ROWS + 1) fail("Measurement CSV contains more than 100000 data rows.");
  }
  if (quoted) fail("Measurement CSV has an unterminated quoted field.");
  if (afterQuote || row.length > 0 || field.length > 0) { row.push(field); rows.push(row); }
  while (rows.at(-1)?.length === 1 && rows.at(-1)?.[0] === "") rows.pop();
  if (!rows.length) fail("Measurement CSV has no header row.");
  const width = rows[0].length;
  if (!width || width > MAX_COLUMNS || rows[0].some((cell) => !cell.trim())) fail("Measurement CSV header must contain 1–64 nonempty columns.");
  if (rows.some((record) => record.length !== width)) fail("Measurement CSV rows do not have a consistent column count.");
  const normalized = rows[0].map((header) => header.trim().toLocaleLowerCase());
  if (new Set(normalized).size !== normalized.length) fail("Measurement CSV contains duplicate column names.");
  return rows;
}

function findColumn(headers: string[], requested: string, required: boolean): number | null {
  const name = requested.trim().toLocaleLowerCase();
  if (!name || name.length > 128) fail("Measurement column names must be 1–128 characters.");
  const matches = headers.map((header, index) => header.trim().toLocaleLowerCase() === name ? index : -1).filter((index) => index >= 0);
  if (matches.length > 1) fail(`Measurement CSV contains duplicate column names for '${requested}'.`);
  if (!matches.length && required) fail(`Measurement CSV is missing the '${requested}' column.`);
  return matches[0] ?? null;
}

function text(value: string, label: string, optional = false): string | null {
  const trimmed = value.trim();
  if (!trimmed && optional) return null;
  if (!trimmed || encoder.encode(trimmed).byteLength > 256) fail(`Measurement ${label} values must be 1–256 characters.`);
  return trimmed;
}

export function parseMeasurementCsv(input: string, baselineVariant = "baseline", metricFilter?: string): LocalMeasurementReport {
  const baseline = text(baselineVariant, "baseline variant")!;
  const filter = metricFilter === undefined ? undefined : text(metricFilter, "metric filter")!;
  const rows = parseCsv(input);
  const headers = rows[0];
  const variant = findColumn(headers, "variant", true)!;
  const value = findColumn(headers, "value", true)!;
  const metric = findColumn(headers, "metric", false);
  const timestamp = findColumn(headers, "timestamp", false);
  const accumulators = new Map<string, { metric: string; variant: string; sampleSize: number; total: number; sumSquares: number }>();
  const dates: string[] = [];
  let included = 0;
  for (const [rowIndex, record] of rows.slice(1).entries()) {
    const variantValue = text(record[variant], "variant")!;
    const metricValue = metric === null ? "primary" : text(record[metric], "metric")!;
    if (filter !== undefined && metricValue.toLocaleLowerCase() !== filter.toLocaleLowerCase()) continue;
    const numeric = Number(record[value].trim());
    if (!Number.isFinite(numeric)) fail(`Measurement value on CSV row ${rowIndex + 2} is not finite.`);
    const key = `${metricValue}\u0000${variantValue}`;
    const previous = accumulators.get(key) ?? { metric: metricValue, variant: variantValue, sampleSize: 0, total: 0, sumSquares: 0 };
    previous.sampleSize += 1;
    previous.total += numeric;
    previous.sumSquares += numeric * numeric;
    if (!Number.isFinite(previous.total) || !Number.isFinite(previous.sumSquares)) fail("Measurement totals exceed the supported numeric range.");
    accumulators.set(key, previous);
    included += 1;
    const date = timestamp === null ? null : text(record[timestamp], "timestamp", true);
    if (date !== null) dates.push(date);
  }
  if (!included) fail("Measurement CSV contains no observations after filtering.");
  const stats = new Map<string, { sampleSize: number; total: number; mean: number; sampleStddev: number | null }>();
  for (const [key, group] of accumulators) {
    const mean = group.total / group.sampleSize;
    const varianceNumerator = group.sampleSize < 2 ? null : group.sumSquares - (group.total * group.total / group.sampleSize);
    const sampleStddev = varianceNumerator === null || !Number.isFinite(varianceNumerator) ? null : Math.sqrt(Math.max(0, varianceNumerator) / (group.sampleSize - 1));
    stats.set(key, { sampleSize: group.sampleSize, total: group.total, mean, sampleStddev });
  }
  const meanInterval = (group: { sampleSize: number; mean: number; sampleStddev: number | null }): MeasurementInterval | null => {
    if (group.sampleStddev === null || group.sampleSize < 2) return null;
    const margin = NORMAL_95_Z * group.sampleStddev / Math.sqrt(group.sampleSize);
    return Number.isFinite(margin) && Number.isFinite(group.mean - margin) && Number.isFinite(group.mean + margin) ? { lower: group.mean - margin, upper: group.mean + margin } : null;
  };
  const differenceInterval = (variantStats: { sampleSize: number; mean: number; sampleStddev: number | null }, baselineStats: { sampleSize: number; mean: number; sampleStddev: number | null }): MeasurementInterval | null => {
    if (variantStats.sampleSize < 2 || baselineStats.sampleSize < 2 || variantStats.sampleStddev === null || baselineStats.sampleStddev === null) return null;
    const standardError = Math.sqrt((variantStats.sampleStddev ** 2) / variantStats.sampleSize + (baselineStats.sampleStddev ** 2) / baselineStats.sampleSize);
    const difference = variantStats.mean - baselineStats.mean;
    const margin = NORMAL_95_Z * standardError;
    return Number.isFinite(margin) && Number.isFinite(difference - margin) && Number.isFinite(difference + margin) ? { lower: difference - margin, upper: difference + margin } : null;
  };
  const warnings: string[] = [];
  if (timestamp === null) warnings.push("No timestamp column was supplied; date range is unavailable.");
  else if (!dates.length) warnings.push("The timestamp column contained no nonempty values; date range is unavailable.");
  const groups = [...accumulators.values()].sort((left, right) => left.metric.localeCompare(right.metric) || (left.variant === baseline ? -1 : right.variant === baseline ? 1 : left.variant.localeCompare(right.variant))).map((group) => {
    const groupStats = stats.get(`${group.metric}\u0000${group.variant}`)!;
    const baselineStats = stats.get(`${group.metric}\u0000${baseline}`);
    const comparison = baselineStats === undefined ? null : {
      baselineMean: baselineStats.mean,
      difference: groupStats.mean - baselineStats.mean,
      relativeChangePercent: baselineStats.mean === 0 ? null : (groupStats.mean - baselineStats.mean) / Math.abs(baselineStats.mean) * 100,
      differenceInterval95: group.variant === baseline ? null : differenceInterval(groupStats, baselineStats),
    };
    if (group.variant !== baseline && comparison === null) warnings.push(`Metric '${group.metric}' has no '${baseline}' baseline; its variants are shown without comparison.`);
    return { ...group, mean: groupStats.mean, sampleStddev: groupStats.sampleStddev, meanInterval95: meanInterval(groupStats), comparison };
  });
  const sortedDates = [...dates].sort();
  return {
    provenance: "MEASURED",
    rowsRead: rows.length - 1,
    rowsIncluded: included,
    baselineVariant: baseline,
    dateRange: sortedDates.length ? { from: sortedDates[0], to: sortedDates.at(-1)! } : null,
    analysis: {
      method: "normal_approximation",
      confidenceLevelPercent: 95,
      assumptions: [
        "Intervals are descriptive and use an independent-observation normal approximation.",
        "A mean interval requires at least two observations in that group.",
        "A difference interval requires at least two observations in both variant and baseline groups.",
        "Intervals are not a significance test, causal estimate or winner decision.",
      ],
    },
    groups,
    warnings,
  };
}
