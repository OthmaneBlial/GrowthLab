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

test("GrowthLab onboarding shell uses every localized growth message", async () => {
  const source = await readFile(new URL("src/growth/GrowthDashboard.tsx", uiRoot), "utf8");
  const settings = JSON.parse(await readFile(new URL("project.inlang/settings.json", uiRoot), "utf8"));
  for (const key of growthKeys) {
    assert.match(source, new RegExp(key), `${key} should be rendered by GrowthDashboard`);
  }
  for (const locale of settings.locales) {
    const catalog = JSON.parse(await readFile(new URL(`messages/${locale}.json`, uiRoot), "utf8"));
    for (const key of growthKeys) {
      assert.equal(typeof catalog[key], "string", `${locale} should define ${key}`);
      assert.ok(catalog[key].trim(), `${locale} should not leave ${key} empty`);
    }
  }
});
