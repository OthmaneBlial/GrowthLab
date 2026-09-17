import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { workspaceKey } from "../queries/client";
import { setLocale, useLocale } from "../locale";
import { locales, type Locale } from "../paraglide/runtime.js";
import { m } from "../paraglide/messages.js";
import { useThemePreference, type ThemePreference } from "../theme";
import { growth, type GrowthConfig, type Workspace } from "./api";

const LOCALE_LABELS: Record<Locale, string> = {
  en: "English",
  "zh-CN": "简体中文",
  fa: "فارسی",
  ar: "العربية",
  es: "Español",
  hi: "हिन्दी",
};

type GrowthSettingsPanelProps = {
  workspaces?: Workspace[];
  activeWorkspace?: Workspace;
};

export function GrowthSettingsPanel({ workspaces = [], activeWorkspace }: GrowthSettingsPanelProps) {
  const client = useQueryClient();
  const locale = useLocale();
  const [theme, setTheme] = useThemePreference();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<GrowthConfig | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const sources = useQuery({ queryKey: workspaceKey("growth", "measurement-sources"), queryFn: ({ signal }) => growth.measurementSources(signal) });
  const config = activeWorkspace?.config;
  useEffect(() => {
    if (!editing) setDraft(config ? structuredClone(config) : null);
  }, [config, editing]);
  const list = (values: string[]) => values.length ? values.join(", ") : m.growth_settings_none();
  const modeLabel = (mode: Workspace["config"]["permissions"]["mode"]) => ({
    analyze_only: m.growth_settings_mode_analyze_only(),
    draft: m.growth_settings_mode_draft(),
    implementation: m.growth_settings_mode_implementation(),
  })[mode];

  async function save(event: React.FormEvent) {
    event.preventDefault();
    if (!activeWorkspace || !draft || busy) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await growth.updateWorkspace(activeWorkspace.projectId, draft, activeWorkspace.sourceSnapshotCommit);
      await client.invalidateQueries({ queryKey: workspaceKey("growth", "workspaces") });
      setEditing(false);
      setNotice(m.growth_settings_saved());
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  }

  function lines(value: string) {
    return value.split("\n").map((item) => item.trim()).filter(Boolean);
  }

  return <section id="growth-settings" className="growth-settings growth-panel" aria-labelledby="growth-settings-title">
    <span className="growth-section-number">{m.growth_settings_section()}</span>
    <h2 id="growth-settings-title">{m.growth_settings_title()}</h2>
    <p>{m.growth_settings_intro()}</p>
    <div className="growth-settings-grid">
      <label htmlFor="growth-settings-language">{m.growth_settings_language()}<select id="growth-settings-language" value={locale} onChange={(event) => { const next = event.currentTarget.value; if (locales.includes(next as Locale)) void setLocale(next as Locale); }}>{locales.map((item) => <option key={item} value={item}>{LOCALE_LABELS[item]}</option>)}</select></label>
      <label htmlFor="growth-settings-theme">{m.growth_settings_theme()}<select id="growth-settings-theme" value={theme} onChange={(event) => setTheme(event.currentTarget.value as ThemePreference)}><option value="system">{m.growth_settings_system()}</option><option value="light">{m.growth_settings_light()}</option><option value="dark">{m.growth_settings_dark()}</option></select></label>
    </div>
    <div className="growth-settings-privacy">
      <div><span className="growth-eyebrow">{m.growth_settings_privacy_title()}</span><p>{m.growth_settings_privacy_body()}</p></div>
      <div><span className="growth-eyebrow">{m.growth_settings_integrations_title()}</span><p>{m.growth_settings_integrations_body()}</p>{sources.data && <p className="growth-muted">{sources.data.filter((source) => source.status === "available").length} {m.growth_settings_local_source_count()} · {sources.data.filter((source) => source.status === "planned").length} {m.growth_settings_planned_source_count()}</p>}</div>
    </div>
    <div className="growth-settings-contract">
      <div className="growth-settings-contract-heading"><div><span className="growth-eyebrow">{m.growth_settings_contract_title()}</span><p>{m.growth_settings_contract_intro()}</p></div>{config && !editing && <button type="button" className="growth-button growth-button-secondary" onClick={() => { setDraft(structuredClone(config)); setEditing(true); setError(null); setNotice(null); }}>{m.growth_settings_edit()}</button>}{!editing && <span className="growth-settings-readonly">{m.growth_settings_read_only()}</span>}</div>
      {config && editing && draft ? <form className="growth-settings-editor" onSubmit={(event) => void save(event)}>
        <p className="growth-muted">{m.growth_settings_edit_notice()}</p>
        <div className="growth-settings-editor-grid">
          <label>{m.growth_settings_product()}<input value={draft.product.name} maxLength={4096} required onChange={(event) => setDraft({ ...draft, product: { ...draft.product, name: event.target.value } })} /></label>
          <label>{m.growth_settings_audience()}<input value={draft.product.audience} maxLength={4096} required onChange={(event) => setDraft({ ...draft, product: { ...draft.product, audience: event.target.value } })} /></label>
          <label className="growth-settings-editor-wide">{m.growth_settings_description()}<textarea value={draft.product.description} maxLength={8192} rows={3} onChange={(event) => setDraft({ ...draft, product: { ...draft.product, description: event.target.value } })} /></label>
          <label className="growth-settings-editor-wide">{m.growth_settings_goal()}<textarea value={draft.goal.primary} maxLength={4096} rows={2} required onChange={(event) => setDraft({ ...draft, goal: { primary: event.target.value } })} /></label>
          <label>{m.growth_settings_permission_mode()}<select value={draft.permissions.mode} onChange={(event) => setDraft({ ...draft, permissions: { ...draft.permissions, mode: event.target.value as GrowthConfig["permissions"]["mode"] } })}><option value="analyze_only">{m.growth_settings_mode_analyze_only()}</option><option value="draft">{m.growth_settings_mode_draft()}</option><option value="implementation">{m.growth_settings_mode_implementation()}</option></select></label>
          <label>{m.growth_settings_parallelism()}<input type="number" min={1} max={8} value={draft.agents.parallelism} onChange={(event) => setDraft({ ...draft, agents: { parallelism: Number(event.target.value) } })} /></label>
          <label>{m.growth_settings_allowed_paths()}<textarea value={draft.permissions.allowed_paths.join("\n")} rows={3} onChange={(event) => setDraft({ ...draft, permissions: { ...draft.permissions, allowed_paths: lines(event.target.value) } })} /></label>
          <label>{m.growth_settings_denied_paths()}<textarea value={draft.permissions.denied_paths.join("\n")} rows={3} onChange={(event) => setDraft({ ...draft, permissions: { ...draft.permissions, denied_paths: lines(event.target.value) } })} /></label>
          <label className="growth-settings-editor-wide">{m.growth_settings_validation_commands()}<textarea value={draft.validation.commands.join("\n")} rows={3} onChange={(event) => setDraft({ ...draft, validation: { ...draft.validation, commands: lines(event.target.value) } })} /></label>
          <label>{m.growth_settings_primary_metric()}<input value={draft.metrics.primary} maxLength={4096} required onChange={(event) => setDraft({ ...draft, metrics: { ...draft.metrics, primary: event.target.value } })} /></label>
          <label>{m.growth_settings_guardrails()}<textarea value={draft.metrics.guardrails.join("\n")} rows={3} onChange={(event) => setDraft({ ...draft, metrics: { ...draft.metrics, guardrails: lines(event.target.value) } })} /></label>
          <label>{m.growth_contract_timeout()}<input type="number" min={1} max={3600} value={draft.validation.timeout_seconds} onChange={(event) => setDraft({ ...draft, validation: { ...draft.validation, timeout_seconds: Number(event.target.value) } })} /></label>
        </div>
        {(error || notice) && <p className={error ? "growth-error" : "growth-notice"} role={error ? "alert" : "status"}>{error ?? notice}</p>}
        <div className="growth-line"><button type="submit" className="growth-button" disabled={busy}>{busy ? m.growth_settings_saving() : m.growth_settings_save()}</button><button type="button" className="growth-button growth-button-secondary" disabled={busy} onClick={() => { setEditing(false); setDraft(config ? structuredClone(config) : null); setError(null); }}>{m.growth_settings_cancel()}</button></div>
      </form> : config ? <div className="growth-settings-contract-grid">
        <dl><div><dt>{m.growth_settings_product()}</dt><dd>{config.product.name}</dd></div><div><dt>{m.growth_settings_description()}</dt><dd>{config.product.description || m.growth_settings_none()}</dd></div><div><dt>{m.growth_settings_audience()}</dt><dd>{config.product.audience}</dd></div><div><dt>{m.growth_settings_goal()}</dt><dd>{config.goal.primary}</dd></div><div><dt>{m.growth_settings_permission_mode()}</dt><dd>{modeLabel(config.permissions.mode)}</dd></div></dl>
        <dl><div><dt>{m.growth_settings_allowed_paths()}</dt><dd>{list(config.permissions.allowed_paths)}</dd></div><div><dt>{m.growth_settings_denied_paths()}</dt><dd>{list(config.permissions.denied_paths)}</dd></div><div><dt>{m.growth_settings_validation_commands()}</dt><dd>{list(config.validation.commands)}</dd></div></dl>
        <dl><div><dt>{m.growth_settings_primary_metric()}</dt><dd>{config.metrics.primary}</dd></div><div><dt>{m.growth_settings_guardrails()}</dt><dd>{list(config.metrics.guardrails)}</dd></div><div><dt>{m.growth_settings_parallelism()}</dt><dd>{config.agents.parallelism}</dd></div><div><dt>{m.growth_settings_source_snapshot()}</dt><dd><code>{activeWorkspace.sourceSnapshotCommit}</code></dd></div></dl>
      </div> : <div className="growth-settings-workspaces"><p>{m.growth_settings_contract_empty()}</p>{workspaces.length > 0 && <ul>{workspaces.map((workspace) => <li key={workspace.projectId}><strong>{workspace.config.product.name}</strong><span>{modeLabel(workspace.config.permissions.mode)} · {workspace.config.goal.primary}</span></li>)}</ul>}</div>}
      {config && !editing && (error || notice) && <p className={error ? "growth-error" : "growth-notice"} role={error ? "alert" : "status"}>{error ?? notice}</p>}
    </div>
  </section>;
}
