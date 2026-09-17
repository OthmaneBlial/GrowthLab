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
  "growth_settings_nav",
  "growth_battle_variant_inspection",
  "growth_battle_hypothesis_evidence",
  "growth_battle_diff_files_logs",
  "growth_battle_static_preview",
  "growth_battle_your_decision",
  "growth_battle_ready_delivery",
  "growth_battle_eligible_review",
  "growth_battle_no_eligible",
  "growth_battle_selection_review",
  "growth_battle_candidate_selected",
  "growth_battle_select_candidate",
  "growth_battle_take_evidence",
  "growth_battle_report_title",
  "growth_battle_report_body",
  "growth_battle_download_html",
  "growth_battle_download_markdown",
  "growth_battle_inspect_diff",
  "growth_contract_summary",
  "growth_contract_configured_checks",
  "growth_contract_allowed_paths",
  "growth_contract_denied_paths",
  "growth_contract_none",
  "growth_contract_protected_paths",
  "growth_contract_validation",
  "growth_contract_timeout",
  "growth_contract_parallel_variants",
  "growth_contract_baseline_commit",
  "growth_contract_sha",
  "growth_digest_not_recorded",
  "growth_evidence_mechanism",
  "growth_evidence_outcome_contract",
  "growth_evidence_success_threshold",
  "growth_evidence_not_supplied",
  "growth_evidence_skeptic_notes",
  "growth_rubric_suggested_steps",
  "growth_rubric_no_gaps",
  "growth_rubric_no_structural_gaps",
];
const staticPreviewKeys = [
  "growth_static_preview_evidence_failed",
  "growth_static_preview_no_archive",
  "growth_static_preview_not_configured",
  "growth_static_preview_viewport_group",
  "growth_static_preview_desktop",
  "growth_static_preview_phone",
  "growth_static_preview_sealed_source",
  "growth_static_preview_checkpoint_source",
  "growth_static_preview_note_captured",
  "growth_static_preview_note_missing",
  "growth_static_preview_iframe_title",
  "growth_static_preview_captured_png",
  "growth_static_preview_observed_render_check",
  "growth_static_preview_viewport",
  "growth_static_preview_matched",
  "growth_static_preview_mismatched",
  "growth_static_preview_horizontal_overflow",
  "growth_static_preview_detected",
  "growth_static_preview_none_detected",
  "growth_static_preview_visible_copy",
  "growth_static_preview_characters",
  "growth_static_preview_dom_load",
  "growth_static_preview_first_contentful_paint",
  "growth_static_preview_source_limitations",
  "growth_static_preview_candidate_commit",
  "growth_static_preview_document_sha",
  "growth_static_preview_archive_sha",
  "growth_static_preview_checkpoint_sha",
  "growth_static_preview_source_summary",
  "growth_static_preview_alt",
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
const settingsKeys = [
  "growth_settings_title",
  "growth_settings_intro",
  "growth_settings_language",
  "growth_settings_theme",
  "growth_settings_system",
  "growth_settings_light",
  "growth_settings_dark",
  "growth_settings_privacy_title",
  "growth_settings_privacy_body",
  "growth_settings_integrations_title",
  "growth_settings_integrations_body",
  "growth_settings_local_source_count",
  "growth_settings_planned_source_count",
  "growth_settings_contract_title",
  "growth_settings_contract_intro",
  "growth_settings_read_only",
  "growth_settings_contract_empty",
  "growth_settings_product",
  "growth_settings_description",
  "growth_settings_audience",
  "growth_settings_goal",
  "growth_settings_permission_mode",
  "growth_settings_allowed_paths",
  "growth_settings_denied_paths",
  "growth_settings_validation_commands",
  "growth_settings_primary_metric",
  "growth_settings_guardrails",
  "growth_settings_parallelism",
  "growth_settings_source_snapshot",
  "growth_settings_none",
  "growth_settings_section",
  "growth_settings_mode_analyze_only",
  "growth_settings_mode_draft",
  "growth_settings_mode_implementation",
  "growth_settings_edit",
  "growth_settings_edit_notice",
  "growth_settings_save",
  "growth_settings_saving",
  "growth_settings_cancel",
  "growth_settings_saved",
  "growth_settings_policy_loading",
  "growth_settings_policy_hypotheses",
  "growth_settings_policy_battle",
  "growth_settings_policy_available",
];

test("GrowthLab onboarding shell uses every localized growth message", async () => {
  const source = await readFile(new URL("src/growth/GrowthDashboard.tsx", uiRoot), "utf8");
  const staticPreviewSource = await readFile(new URL("src/growth/StaticPreviewPanel.tsx", uiRoot), "utf8");
  const measurementSource = await readFile(new URL("src/growth/MeasurementPanel.tsx", uiRoot), "utf8");
  const settingsSource = await readFile(new URL("src/growth/GrowthSettingsPanel.tsx", uiRoot), "utf8");
  const settings = JSON.parse(await readFile(new URL("project.inlang/settings.json", uiRoot), "utf8"));
  for (const key of growthKeys) {
    assert.match(source, new RegExp(key), `${key} should be rendered by GrowthDashboard`);
  }
  for (const key of staticPreviewKeys) {
    assert.match(staticPreviewSource, new RegExp(key), `${key} should be rendered by StaticPreviewPanel`);
  }
  for (const key of measurementKeys) {
    assert.match(measurementSource, new RegExp(key), `${key} should be rendered by MeasurementPanel`);
  }
  for (const key of settingsKeys) {
    assert.match(settingsSource, new RegExp(key), `${key} should be rendered by GrowthSettingsPanel`);
  }
  for (const locale of settings.locales) {
    const catalog = JSON.parse(await readFile(new URL(`messages/${locale}.json`, uiRoot), "utf8"));
    for (const key of [...growthKeys, ...staticPreviewKeys, ...measurementKeys, ...settingsKeys]) {
      assert.equal(typeof catalog[key], "string", `${locale} should define ${key}`);
      assert.ok(catalog[key].trim(), `${locale} should not leave ${key} empty`);
    }
  }
});
