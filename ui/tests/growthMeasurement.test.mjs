import assert from "node:assert/strict";
import test from "node:test";
import { parseMeasurementCsv } from "../src/growth/measurement.ts";

test("parses quoted local observations and compares each variant with its baseline", () => {
  const report = parseMeasurementCsv([
    "metric,variant,value,timestamp,note",
    'qualified_signup,baseline,10,2026-09-01,"first, cohort"',
    'qualified_signup,baseline,14,2026-09-02,"second"',
    'qualified_signup,headline-b,18,2026-09-03,"quoted ""copy"""',
  ].join("\n"));
  assert.equal(report.provenance, "MEASURED");
  assert.equal(report.rowsRead, 3);
  assert.equal(report.rowsIncluded, 3);
  assert.deepEqual(report.dateRange, { from: "2026-09-01", to: "2026-09-03" });
  assert.deepEqual(report.groups.map(({ metric, variant, sampleSize, mean, comparison }) => ({ metric, variant, sampleSize, mean, comparison })), [
    { metric: "qualified_signup", variant: "baseline", sampleSize: 2, mean: 12, comparison: { baselineMean: 12, difference: 0, relativeChangePercent: 0 } },
    { metric: "qualified_signup", variant: "headline-b", sampleSize: 1, mean: 18, comparison: { baselineMean: 12, difference: 6, relativeChangePercent: 50 } },
  ]);
  assert.equal(report.warnings.length, 0);
});

test("supports a metric filter and reports missing timestamps or baselines", () => {
  const report = parseMeasurementCsv([
    "metric,variant,value",
    "primary,experiment,4",
    "other,experiment,9",
  ].join("\n"), "control", "primary");
  assert.equal(report.rowsRead, 2);
  assert.equal(report.rowsIncluded, 1);
  assert.equal(report.groups[0].comparison, null);
  assert.match(report.warnings.join(" "), /No timestamp/);
  assert.match(report.warnings.join(" "), /no 'control' baseline/);
});

test("rejects malformed, inconsistent and non-finite local data", () => {
  assert.throws(() => parseMeasurementCsv("variant,value\nbaseline,1\nexperiment,2,extra"), /consistent column count/);
  assert.throws(() => parseMeasurementCsv('variant,value\n"baseline,1'), /unterminated quoted field/);
  assert.throws(() => parseMeasurementCsv("variant,value\nbaseline,Infinity"), /not finite/);
});
