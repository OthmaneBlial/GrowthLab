import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const uiRoot = new URL("..", import.meta.url);
const growthKeys = [
  "growth_language",
  "growth_local_workspace",
  "growth_product_workspaces",
  "growth_your_lab",
  "growth_no_products",
  "growth_experimentation_laboratory",
  "growth_intro_title_line_one",
  "growth_intro_title_line_two",
  "growth_intro_body",
  "growth_no_api_key",
  "growth_demo_title",
  "growth_demo_body",
  "growth_run_bundled_demo",
  "growth_start_with_product",
  "growth_import_local_folder",
  "growth_import_product",
  "growth_read_evidence",
  "growth_candidate_earns_place",
  "growth_connection_live",
  "growth_connection_connecting",
];
const measurementKeys = [
  "growth_measure_title",
  "growth_measure_intro",
  "growth_measure_source_boundary",
  "growth_measure_loading_sources",
  "growth_measure_sources_unavailable",
  "growth_measure_available_locally",
  "growth_measure_planned",
  "growth_measure_no_network",
  "growth_measure_opt_in_network",
  "growth_measure_telemetry_csv",
  "growth_measure_baseline_variant",
  "growth_measure_reading",
  "growth_measure_summarize_locally",
  "growth_measure_loaded_locally",
  "growth_measure_descriptive_summary",
  "growth_measure_observations",
  "growth_measure_date_unavailable",
  "growth_measure_metric",
  "growth_measure_distribution",
  "growth_measure_variant",
  "growth_measure_mean",
  "growth_measure_sample_size",
  "growth_measure_mean_interval",
  "growth_measure_change",
  "growth_measure_difference_interval",
  "growth_measure_analysis_summary",
  "growth_measure_analysis_body",
  "growth_measure_limit",
];

test("GrowthLab onboarding shell uses every localized growth message", async () => {
  const source = await readFile(new URL("src/growth/GrowthDashboard.tsx", uiRoot), "utf8");
  const measurementSource = await readFile(new URL("src/growth/MeasurementPanel.tsx", uiRoot), "utf8");
  const settings = JSON.parse(await readFile(new URL("project.inlang/settings.json", uiRoot), "utf8"));
  for (const key of growthKeys) {
    assert.match(source, new RegExp(key), `${key} should be rendered by GrowthDashboard`);
  }
  for (const key of measurementKeys) {
    assert.match(measurementSource, new RegExp(key), `${key} should be rendered by MeasurementPanel`);
  }
  for (const locale of settings.locales) {
    const catalog = JSON.parse(await readFile(new URL(`messages/${locale}.json`, uiRoot), "utf8"));
    for (const key of [...growthKeys, ...measurementKeys]) {
      assert.equal(typeof catalog[key], "string", `${locale} should define ${key}`);
      assert.ok(catalog[key].trim(), `${locale} should not leave ${key} empty`);
    }
  }
});
