import { useQuery } from "@tanstack/react-query";
import { workspaceKey } from "../queries/client";
import { setLocale, useLocale } from "../locale";
import { locales, type Locale } from "../paraglide/runtime.js";
import { m } from "../paraglide/messages.js";
import { useThemePreference, type ThemePreference } from "../theme";
import { growth } from "./api";

const LOCALE_LABELS: Record<Locale, string> = {
  en: "English",
  "zh-CN": "简体中文",
  fa: "فارسی",
  ar: "العربية",
  es: "Español",
  hi: "हिन्दी",
};

export function GrowthSettingsPanel() {
  const locale = useLocale();
  const [theme, setTheme] = useThemePreference();
  const sources = useQuery({ queryKey: workspaceKey("growth", "measurement-sources"), queryFn: ({ signal }) => growth.measurementSources(signal) });

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
  </section>;
}
