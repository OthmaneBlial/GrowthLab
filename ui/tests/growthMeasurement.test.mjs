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
  const baseline = report.groups.find((group) => group.variant === "baseline");
  const headline = report.groups.find((group) => group.variant === "headline-b");
  assert.equal(baseline?.sampleStddev, 2.8284271247461903);
  assert.ok(baseline?.meanInterval95);
  assert.ok(Math.abs(baseline.meanInterval95.lower - 8.08) < 0.01);
  assert.ok(Math.abs(baseline.meanInterval95.upper - 15.92) < 0.01);
  assert.deepEqual(headline?.comparison, { baselineMean: 12, difference: 6, relativeChangePercent: 50, differenceInterval95: null });
  assert.equal(report.analysis.method, "normal_approximation");
  assert.equal(report.analysis.confidenceLevelPercent, 95);
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

test("computes an exploratory difference interval only with repeated baseline and variant observations", () => {
  const report = parseMeasurementCsv("variant,value\nbase,10\nbase,20\nhero,14\nhero,22\n", "base");
  const hero = report.groups.find((group) => group.variant === "hero");
  assert.ok(hero?.comparison?.differenceInterval95);
  assert.ok(Math.abs(hero.comparison.differenceInterval95.lower + 9.55) < 0.02);
  assert.ok(Math.abs(hero.comparison.differenceInterval95.upper - 15.55) < 0.02);
});
