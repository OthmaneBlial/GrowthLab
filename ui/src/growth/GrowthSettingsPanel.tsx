import { useQuery } from "@tanstack/react-query";
import { workspaceKey } from "../queries/client";
import { setLocale, useLocale } from "../locale";
import { locales, type Locale } from "../paraglide/runtime.js";
import { m } from "../paraglide/messages.js";
import { useThemePreference, type ThemePreference } from "../theme";
import { growth, type Workspace } from "./api";

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

function displayMode(mode: Workspace["config"]["permissions"]["mode"]) {
  return mode.replaceAll("_", " ");
}

export function GrowthSettingsPanel({ workspaces = [], activeWorkspace }: GrowthSettingsPanelProps) {
  const locale = useLocale();
  const [theme, setTheme] = useThemePreference();
  const sources = useQuery({ queryKey: workspaceKey("growth", "measurement-sources"), queryFn: ({ signal }) => growth.measurementSources(signal) });
  const config = activeWorkspace?.config;
  const list = (values: string[]) => values.length ? values.join(", ") : m.growth_settings_none();

  return <section id="growth-settings" className="growth-settings growth-panel" aria-labelledby="growth-settings-title">
    <span className="growth-section-number">SETTINGS / LOCAL CONTROL</span>
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
      <div className="growth-settings-contract-heading"><div><span className="growth-eyebrow">{m.growth_settings_contract_title()}</span><p>{m.growth_settings_contract_intro()}</p></div><span className="growth-settings-readonly">{m.growth_settings_read_only()}</span></div>
      {config ? <div className="growth-settings-contract-grid">
        <dl><div><dt>{m.growth_settings_product()}</dt><dd>{config.product.name}</dd></div><div><dt>{m.growth_settings_audience()}</dt><dd>{config.product.audience}</dd></div><div><dt>{m.growth_settings_goal()}</dt><dd>{config.goal.primary}</dd></div><div><dt>{m.growth_settings_permission_mode()}</dt><dd>{displayMode(config.permissions.mode)}</dd></div></dl>
        <dl><div><dt>{m.growth_settings_allowed_paths()}</dt><dd>{list(config.permissions.allowed_paths)}</dd></div><div><dt>{m.growth_settings_denied_paths()}</dt><dd>{list(config.permissions.denied_paths)}</dd></div><div><dt>{m.growth_settings_validation_commands()}</dt><dd>{list(config.validation.commands)}</dd></div></dl>
        <dl><div><dt>{m.growth_settings_primary_metric()}</dt><dd>{config.metrics.primary}</dd></div><div><dt>{m.growth_settings_guardrails()}</dt><dd>{list(config.metrics.guardrails)}</dd></div><div><dt>{m.growth_settings_parallelism()}</dt><dd>{config.agents.parallelism}</dd></div><div><dt>{m.growth_settings_source_snapshot()}</dt><dd><code>{activeWorkspace.sourceSnapshotCommit}</code></dd></div></dl>
      </div> : <div className="growth-settings-workspaces"><p>{m.growth_settings_contract_empty()}</p>{workspaces.length > 0 && <ul>{workspaces.map((workspace) => <li key={workspace.projectId}><strong>{workspace.config.product.name}</strong><span>{displayMode(workspace.config.permissions.mode)} · {workspace.config.goal.primary}</span></li>)}</ul>}</div>}
    </div>
  </section>;
}
