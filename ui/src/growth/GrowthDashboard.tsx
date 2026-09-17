import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { isCurrentScope, workspaceKey, workspaceScope } from "../queries/client";
import { useThemePreference } from "../theme";
import { setLocale, useLocale } from "../locale";
import { locales, type Locale } from "../paraglide/runtime.js";
import { m } from "../paraglide/messages.js";
import { Spinner } from "../components/ui";
import { growth, type ApplyPreview, type Execution, type Hypothesis, type HypothesisUpdate, type Provenance, type GrowthPlaybook, type PlaybookRun, type PublicRepositoryAudit, type RemoteSeoAudit, type PageQualityRubric, type RenderRubric, type PerformanceRubric, type AccessibilityRubric } from "./api";
import { inspectionTab, runPhase, selectedVariant, variantRun, type InspectionTab } from "./view";
import { StaticPreviewPanel } from "./StaticPreviewPanel";
import { MeasurementPanel } from "./MeasurementPanel";
import { GrowthSettingsPanel } from "./GrowthSettingsPanel";
import "./growth.css";

const GROWTH_LOCALE_LABELS: Record<Locale, string> = {
  en: "English",
  "zh-CN": "简体中文",
  fa: "فارسی",
  ar: "العربية",
  es: "Español",
  hi: "हिन्दी",
};

function GrowthLanguagePicker() {
  const locale = useLocale();
  return <label className="growth-language-picker"><span>{m.growth_language()}</span><select aria-label={m.growth_language()} value={locale} onChange={(event) => { const next = event.currentTarget.value; if (locales.includes(next as Locale)) setLocale(next as Locale); }}>{locales.map((item) => <option key={item} value={item}>{GROWTH_LOCALE_LABELS[item]}</option>)}</select></label>;
}

const growthYourLab = m.growth_your_lab;

function Mark({ value }: { value: Provenance }) { return <span className={`growth-mark growth-mark-${value.toLowerCase()}`}>{value}</span>; }
const GROWTH_ROLE_LABELS: Record<string, () => string> = {
  strategist: m.growth_role_strategist,
  researcher: m.growth_role_researcher,
  positioning: m.growth_role_positioning,
  conversion: m.growth_role_conversion,
  seo: m.growth_role_seo,
  onboarding: m.growth_role_onboarding,
  pricing: m.growth_role_pricing,
  launch: m.growth_role_launch,
  evaluator: m.growth_role_evaluator,
  skeptic: m.growth_role_skeptic,
};
function roleLabel(role: string) { return GROWTH_ROLE_LABELS[role]?.() ?? role.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase()); }
function permissionModeLabel(mode: string) {
  if (mode === "analyze_only") return m.growth_settings_mode_analyze_only();
  if (mode === "draft") return m.growth_settings_mode_draft();
  if (mode === "implementation") return m.growth_settings_mode_implementation();
  return mode.replaceAll("_", " ");
}
function Digest({ label, value }: { label: string; value?: string | null }) {
  return <div className="growth-digest"><dt>{label}</dt><dd>{value ?? m.growth_digest_not_recorded()}</dd></div>;
}
function HypothesisEvidence({ hypothesis }: { hypothesis: Hypothesis }) {
  return <div className="growth-evidence">
    <div><span className="growth-eyebrow">{m.growth_evidence_mechanism()}</span><p>{hypothesis.mechanism}</p></div>
    <div><span className="growth-eyebrow">{m.growth_evidence_outcome_contract()}</span><p>{hypothesis.primaryMetric}</p><p className="growth-muted">{hypothesis.baselineDefinition}</p><p className="growth-muted">{m.growth_evidence_success_threshold({ value: hypothesis.successThreshold ?? m.growth_evidence_not_supplied() })}</p></div>
    {hypothesis.evidence.map((evidence) => <article key={evidence.id}>
      <div className="growth-line"><strong>{evidence.title}</strong><Mark value={evidence.provenance} /></div>
      <p>{evidence.observation}</p><code>{evidence.source}</code>
      <p className="growth-muted">{evidence.publisher} · {new Date(evidence.retrievedAt).toLocaleDateString()}</p>
      <p className="growth-muted">{evidence.limitations}</p>
    </article>)}
    <div><span className="growth-eyebrow">{m.growth_evidence_skeptic_notes()}</span><ul>{hypothesis.risks.map((risk) => <li key={risk}>{risk}</li>)}</ul></div>
  </div>;
}
function QualityRubricDetails({ rubric }: { rubric: PageQualityRubric | RenderRubric | PerformanceRubric | AccessibilityRubric }) {
  return <details className="growth-seo-rubric growth-quality-rubric"><summary><span>{rubric.label}</span><strong>{rubric.score} / {rubric.maxScore} · {rubric.provenance}</strong></summary><p className="growth-muted">{rubric.calculation}</p><div className="growth-seo-dimensions">{rubric.dimensions.map((dimension) => <article className={`growth-seo-dimension growth-seo-dimension-${dimension.status}`} key={dimension.key}><div className="growth-line"><strong>{dimension.label}</strong><span>{dimension.score} / {dimension.maxScore}</span></div>{dimension.evidence.map((evidence) => <p key={evidence}>{evidence}</p>)}</article>)}</div><div className="growth-seo-recommendations"><strong>{m.growth_rubric_suggested_steps()}</strong><ul>{rubric.recommendations.length ? rubric.recommendations.map((recommendation) => <li key={recommendation}>{recommendation}</li>) : <li>{m.growth_rubric_no_gaps()}</li>}</ul></div><ul className="growth-seo-limitations">{rubric.limitations.map((limitation) => <li key={limitation}>{limitation}</li>)}</ul></details>;
}

function hypothesisDraft(hypothesis: Hypothesis): HypothesisUpdate {
  return {
    title: hypothesis.title,
    hypothesis: hypothesis.hypothesis,
    mechanism: hypothesis.mechanism,
    baselineDefinition: hypothesis.baselineDefinition,
    primaryMetric: hypothesis.primaryMetric,
    guardrailMetrics: hypothesis.guardrailMetrics,
    successThreshold: hypothesis.successThreshold,
    risks: hypothesis.risks,
  };
}

function HypothesisTree({ hypotheses, goal, audience, onChoose, onSave }: { hypotheses: Hypothesis[]; goal: string; audience: string; onChoose: (hypothesis: Hypothesis) => void; onSave: (id: string, draft: HypothesisUpdate) => Promise<boolean> }) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState<HypothesisUpdate | null>(null);
  const groups = new Map<string, Hypothesis[]>();
  for (const hypothesis of hypotheses) {
    const group = groups.get(hypothesis.role) ?? [];
    group.push(hypothesis);
    groups.set(hypothesis.role, group);
  }
  async function save(event: React.FormEvent, id: string) {
    event.preventDefault();
    if (!draft) return;
    if (await onSave(id, draft)) {
      setEditingId(null);
      setDraft(null);
    }
  }
  return <div className="growth-hypothesis-tree">
    <div className="growth-hypothesis-root"><span className="growth-eyebrow">{m.growth_current_goal()}</span><strong>{goal}</strong><small>{audience}</small></div>
    <ol className="growth-hypothesis-branches" aria-label={m.growth_hypothesis_branches()}>
      {[...groups.entries()].map(([role, items]) => <li className="growth-hypothesis-branch" key={role}>
        <details open><summary><span>{roleLabel(role)}</span><small>{items.length} {items.length === 1 ? m.growth_hypothesis_singular() : m.growth_hypothesis_plural()}</small></summary>
          <div className="growth-hypothesis-children">{items.map((item) => <article key={item.id} className="growth-hypothesis-node"><div className="growth-line"><span className="growth-playbook-role">{roleLabel(item.role)}</span><Mark value={item.provenance} /></div><h3 id={`growth-hypothesis-${item.id}`}>{item.title}</h3><p>{item.hypothesis}</p><p className="growth-muted">{m.growth_mechanism_label({ value: item.mechanism })}</p><small>{m.growth_metric_label({ channel: item.channel, metric: item.primaryMetric })}</small><div className="growth-hypothesis-actions"><button type="button" className="growth-button growth-button-secondary" onClick={() => onChoose(item)}>{m.growth_use_branch()} <span aria-hidden="true">↓</span></button><button type="button" className="growth-button growth-button-secondary" onClick={() => { setEditingId(item.id); setDraft(hypothesisDraft(item)); }}>{m.growth_edit_branch()}</button><code title={item.id}>ID {item.id.slice(0, 8)}</code></div>{editingId === item.id && draft && <form className="growth-hypothesis-editor" onSubmit={(event) => void save(event, item.id)}><div className="growth-editor-heading"><strong>{m.growth_edit_hypothesis()}</strong><span className="growth-muted">{m.growth_identity_evidence_fixed()}</span></div><label htmlFor={`growth-edit-title-${item.id}`}>{m.growth_branch_title()}<input id={`growth-edit-title-${item.id}`} value={draft.title} onChange={(event) => setDraft({ ...draft, title: event.target.value })} maxLength={4096} required /></label><label htmlFor={`growth-edit-hypothesis-${item.id}`}>{m.growth_label_hypothesis()}<textarea id={`growth-edit-hypothesis-${item.id}`} value={draft.hypothesis} onChange={(event) => setDraft({ ...draft, hypothesis: event.target.value })} maxLength={4096} rows={3} required /></label><label htmlFor={`growth-edit-mechanism-${item.id}`}>{m.growth_label_mechanism()}<textarea id={`growth-edit-mechanism-${item.id}`} value={draft.mechanism} onChange={(event) => setDraft({ ...draft, mechanism: event.target.value })} maxLength={4096} rows={2} required /></label><label htmlFor={`growth-edit-baseline-${item.id}`}>{m.growth_label_baseline_definition()}<input id={`growth-edit-baseline-${item.id}`} value={draft.baselineDefinition} onChange={(event) => setDraft({ ...draft, baselineDefinition: event.target.value })} maxLength={4096} required /></label><label htmlFor={`growth-edit-metric-${item.id}`}>{m.growth_label_primary_metric()}<input id={`growth-edit-metric-${item.id}`} value={draft.primaryMetric} onChange={(event) => setDraft({ ...draft, primaryMetric: event.target.value })} maxLength={4096} required /></label><label htmlFor={`growth-edit-threshold-${item.id}`}>{m.growth_label_success_threshold_optional()}<input id={`growth-edit-threshold-${item.id}`} value={draft.successThreshold ?? ""} onChange={(event) => setDraft({ ...draft, successThreshold: event.target.value || null })} maxLength={4096} /></label><label htmlFor={`growth-edit-guardrails-${item.id}`}>{m.growth_label_guardrail_metrics()}<textarea id={`growth-edit-guardrails-${item.id}`} value={draft.guardrailMetrics.join("\n")} onChange={(event) => setDraft({ ...draft, guardrailMetrics: event.target.value.split("\n").map((value) => value.trim()).filter(Boolean) })} rows={2} /></label><label htmlFor={`growth-edit-risks-${item.id}`}>{m.growth_label_risks()}<textarea id={`growth-edit-risks-${item.id}`} value={draft.risks.join("\n")} onChange={(event) => setDraft({ ...draft, risks: event.target.value.split("\n").map((value) => value.trim()).filter(Boolean) })} rows={2} /></label><div className="growth-hypothesis-actions"><button type="submit" className="growth-button" disabled={editingId !== item.id}>{m.growth_save_branch()}</button><button type="button" className="growth-button growth-button-secondary" onClick={() => { setEditingId(null); setDraft(null); }}>{m.growth_cancel()}</button></div></form>}</article>)}</div>
        </details>
      </li>)}
    </ol>
  </div>;
}

export function GrowthDashboard({ battleId, projectId }: { battleId?: string; projectId?: string }) {
  const client = useQueryClient();
  const navigate = useNavigate();
  const [theme, setTheme] = useThemePreference();
  useLocale();
  const mounted = useRef(true);
  const viewKey = `${battleId ?? ""}/${projectId ?? ""}`;
  const activeView = useRef(viewKey);
  activeView.current = viewKey;
  const evidenceTab = useRef<HTMLButtonElement>(null);
  const artifactsTab = useRef<HTMLButtonElement>(null);
  const previewTab = useRef<HTMLButtonElement>(null);
  const scope = workspaceScope();
  const [streamConnected, setStreamConnected] = useState(false);
  const [importPath, setImportPath] = useState("");
  const [initializeGit, setInitializeGit] = useState(false);
  const [websiteUrl, setWebsiteUrl] = useState("");
  const [websiteAudit, setWebsiteAudit] = useState<RemoteSeoAudit | null>(null);
  const [repositoryUrl, setRepositoryUrl] = useState("");
  const [repositoryAudit, setRepositoryAudit] = useState<PublicRepositoryAudit | null>(null);
  const [repositoryImportPath, setRepositoryImportPath] = useState("");
  const [repositoryImportName, setRepositoryImportName] = useState("");
  const [repositoryImportAudience, setRepositoryImportAudience] = useState("");
  const [repositoryImportGoal, setRepositoryImportGoal] = useState("");
  const [repositoryImportMode, setRepositoryImportMode] = useState<"analyze_only" | "draft" | "implementation">("analyze_only");
  const [briefName, setBriefName] = useState("");
  const [briefAudience, setBriefAudience] = useState("");
  const [briefGoal, setBriefGoal] = useState("");
  const [briefDescription, setBriefDescription] = useState("");
  const [briefMetric, setBriefMetric] = useState("qualified_signup");
  const [goal, setGoal] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [focused, setFocused] = useState<string | null>(null);
  const [tab, setTab] = useState<InspectionTab>("evidence");
  const [artifactName, setArtifactName] = useState("implementation.diff");
  const [executionMode, setExecutionMode] = useState<"replay" | "native">("replay");
  const [replay, setReplay] = useState("");
  const [harness, setHarness] = useState("");
  const [model, setModel] = useState("");
  const [providerConsent, setProviderConsent] = useState(false);
  const [exportPath, setExportPath] = useState("");
  const [preview, setPreview] = useState<ApplyPreview | null>(null);
  const [applyConsent, setApplyConsent] = useState(false);
  const workspaces = useQuery({ queryKey: workspaceKey("growth", "workspaces"), queryFn: ({ signal }) => growth.workspaces(signal) });
  const battles = useQuery({ queryKey: workspaceKey("growth", "battles"), queryFn: ({ signal }) => growth.battles(signal) });
  const capabilities = useQuery({ queryKey: workspaceKey("growth", "capabilities"), queryFn: ({ signal }) => growth.capabilities(signal) });
  const playbooks = useQuery({ queryKey: workspaceKey("growth", "playbooks"), queryFn: ({ signal }) => growth.playbooks(signal) });
  const status = useQuery({
    queryKey: workspaceKey("growth", "status", battleId), queryFn: ({ signal }) => growth.status(battleId!, signal),
    enabled: !!battleId, refetchInterval: 1500,
  });
  const battle = status.data?.battle;
  const activeProject = battle?.projectId ?? projectId;
  const playbookRuns = useQuery({ queryKey: workspaceKey("growth", "playbook-runs", activeProject), queryFn: ({ signal }) => growth.playbookRuns(activeProject!, signal), enabled: !!activeProject });
  const hypotheses = useQuery({ queryKey: workspaceKey("growth", "hypotheses", activeProject), queryFn: ({ signal }) => growth.hypotheses(activeProject!, signal), enabled: !!activeProject && !battle });
  const workspace = workspaces.data?.find((item) => item.projectId === activeProject);
  const config = battle?.contract.config ?? workspace?.config;
  const portfolio = battle?.contract.hypotheses ?? [];
  const variants = status.data?.variants ?? [];
  const variant = variants.find((item) => item.id === focused) ?? variants[0];
  const hypothesis = portfolio.find((item) => item.id === variant?.hypothesisId);
  const capture = variant ? variantRun(status.data, variant.id) : null;
  const selected = selectedVariant(status.data);
  const comparison = useQuery({
    queryKey: workspaceKey("growth", "comparison", battleId), queryFn: ({ signal }) => growth.compare(battleId!, signal),
    enabled: !!battleId && !!status.data?.runs.length,
  });
  const artifacts = useQuery({
    queryKey: workspaceKey("growth", "artifacts", variant?.id), queryFn: ({ signal }) => growth.artifacts(variant!.id, signal),
    enabled: !!variant && !!capture?.digest && tab === "artifacts",
  });
  const activeArtifact = artifacts.data?.artifacts.find((item) => item.name === artifactName) ?? artifacts.data?.artifacts[0];
  const artifact = useQuery({
    queryKey: workspaceKey("growth", "artifact", variant?.id, activeArtifact?.name),
    queryFn: ({ signal }) => growth.artifact(variant!.id, activeArtifact!.name, signal),
    enabled: !!variant && !!activeArtifact && tab === "artifacts",
  });
  const evidenceTrusted = !!status.data && !status.error && !comparison.error;
  const eligible = evidenceTrusted && (comparison.data?.rows.find((item) => item.variantId === variant?.id)?.eligible ?? false);
  const running = status.data?.controller?.running || battle?.status === "running";
  const queryError = workspaces.error ?? battles.error ?? capabilities.error ?? playbooks.error ?? hypotheses.error ?? status.error ?? comparison.error;
  const refresh = () => client.invalidateQueries({ queryKey: workspaceKey("growth") });

  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  useEffect(() => { setBusy(null); setError(null); setNotice(null); setPreview(null); setApplyConsent(false); }, [viewKey]);
  useEffect(() => { document.title = battle ? `${battle.contract.goal} · GrowthLab` : "GrowthLab"; }, [battle]);
  useEffect(() => { setGoal(workspace?.config.goal.primary ?? ""); }, [workspace?.projectId]); // Keep an edited goal during refresh.
  useEffect(() => {
    const events = new EventSource("/api/growth/events");
    const refreshSnapshot = () => { if (isCurrentScope(scope)) void client.invalidateQueries({ queryKey: [...scope, "growth"] }); };
    events.addEventListener("growth.updated", refreshSnapshot);
    events.addEventListener("resync.required", refreshSnapshot);
    events.onopen = () => setStreamConnected(true);
    events.onerror = () => setStreamConnected(false);
    return () => events.close();
  }, [client, scope[1]]);
  useEffect(() => { setPreview(null); setApplyConsent(false); }, [variant?.id]);

  function tabKey(event: KeyboardEvent<HTMLButtonElement>) {
    const next = inspectionTab(tab, event.key);
    if (!next) return;
    event.preventDefault();
    setTab(next);
    ({ evidence: evidenceTab, artifacts: artifactsTab, preview: previewTab })[next].current?.focus();
  }

  async function perform<T>(label: string, action: () => Promise<T>): Promise<T | null> {
    if (busy) return null;
    setBusy(label); setError(null); setNotice(null);
    try { const result = await action(); if (mounted.current && activeView.current === viewKey) await refresh(); return result; }
    catch (cause) { if (mounted.current && activeView.current === viewKey) setError(cause instanceof Error ? cause.message : String(cause)); return null; }
    finally { if (mounted.current && activeView.current === viewKey) setBusy(null); }
  }
  async function importProduct(event: React.FormEvent) {
    event.preventDefault();
    const result = await perform(m.growth_busy_import_product(), () => growth.import(importPath.trim(), initializeGit));
    if (result && mounted.current && activeView.current === viewKey) void navigate({ to: "/growth/workspaces/$projectId", params: { projectId: result.projectId } });
  }
  async function auditWebsite(event: React.FormEvent) {
    event.preventDefault();
    const result = await perform(m.growth_busy_review_public_page(), () => growth.urlAudit(websiteUrl.trim()));
    if (result && mounted.current && activeView.current === viewKey) setWebsiteAudit(result);
  }
  async function auditRepository(event: React.FormEvent) {
    event.preventDefault();
    const result = await perform(m.growth_busy_review_public_repository(), () => growth.repositoryAudit(repositoryUrl.trim()));
    if (result && mounted.current && activeView.current === viewKey) setRepositoryAudit(result);
  }
  async function importRepository(event: React.FormEvent) {
    event.preventDefault();
    const result = await perform(m.growth_busy_clone_repository(), () => growth.repositoryImport({
      url: repositoryUrl.trim(), path: repositoryImportPath.trim(),
      ...(repositoryImportName.trim() ? { name: repositoryImportName.trim() } : {}),
      ...(repositoryImportAudience.trim() ? { audience: repositoryImportAudience.trim() } : {}),
      ...(repositoryImportGoal.trim() ? { goal: repositoryImportGoal.trim() } : {}),
      mode: repositoryImportMode,
    }));
    if (result && mounted.current && activeView.current === viewKey) void navigate({ to: "/growth/workspaces/$projectId", params: { projectId: result.projectId } });
  }
  async function createManualBrief(event: React.FormEvent) {
    event.preventDefault();
    const result = await perform(m.growth_busy_create_brief(), () => growth.createBrief({
      name: briefName.trim(), audience: briefAudience.trim(), goal: briefGoal.trim(),
      description: briefDescription.trim(), metric: briefMetric.trim(),
    }));
    if (result && mounted.current && activeView.current === viewKey) void navigate({ to: "/growth/workspaces/$projectId", params: { projectId: result.projectId } });
  }
  async function runPlaybook(role: string) {
    if (!workspace) return;
    await perform(m.growth_busy_run_playbook({ role }), () => growth.runPlaybook(workspace.projectId, role));
  }
  async function createHypotheses() {
    if (!workspace) return;
    await perform(m.growth_busy_create_map(), () => growth.createHypotheses(workspace.projectId));
  }
  async function updateHypothesis(id: string, draft: HypothesisUpdate): Promise<boolean> {
    if (!workspace) return false;
    const result = await perform(m.growth_busy_save_hypothesis(), () => growth.updateHypothesis(workspace.projectId, id, draft));
    return !!result;
  }
  async function launchDemo() {
    const result = await perform(m.growth_busy_prepare_demo(), () => growth.demo());
    if (result && mounted.current && activeView.current === viewKey) void navigate({ to: "/growth/$battleId", params: { battleId: result.battleId } });
  }
  async function prepare(event: React.FormEvent) {
    event.preventDefault(); if (!workspace) return;
    const result = await perform(m.growth_busy_prepare_variants(), () => growth.prepare(workspace.projectId, goal));
    if (result && mounted.current && activeView.current === viewKey) void navigate({ to: "/growth/$battleId", params: { battleId: result.battle.id } });
  }
  async function execute(event: React.FormEvent) {
    event.preventDefault(); if (!battle) return;
    await perform(m.growth_busy_submit_battle(), async () => {
      let execution: Execution;
      if (executionMode === "replay") {
        if (new TextEncoder().encode(replay).length > 1024 * 1024) throw new Error("Replay JSON must be at most 1 MiB.");
        let plan: unknown;
        try { plan = JSON.parse(replay); } catch { throw new Error("Enter a valid replay JSON plan."); }
        execution = { mode: "replay", plan };
      } else {
        if (!providerConsent) throw new Error("Authorize the provider request before submitting.");
        execution = { mode: "native", harness, model: model.trim() || null, agentTimeoutSeconds: 120 };
      }
      return growth.run(battle.id, execution);
    });
  }
  async function downloadReport(format: "html" | "markdown") {
    if (!battle) return;
    await perform(m.growth_busy_report(), async () => {
      const blob = await growth.report(battle.id, format);
      if (!mounted.current || activeView.current !== viewKey) return;
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url; link.download = `GrowthLab-battle.${format === "html" ? "html" : "md"}`;
      link.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
    });
  }

  return <div className="growth-app">
    <header className="growth-header">
      <Link to="/" className="growth-brand" aria-label={m.growth_home_aria_label()}><svg width="27" height="27" viewBox="0 0 27 27" aria-hidden="true"><path d="M5 4v10m0-5h10v6m-10-1h17v7" fill="none" stroke="currentColor" strokeWidth="1.6" /><circle cx="5" cy="4" r="2.4" /><circle cx="15" cy="15" r="2.4" /><circle cx="22" cy="21" r="2.4" /></svg>GrowthLab<span className="growth-alpha">{m.growth_source_alpha()}</span></Link>
      <div className="growth-header-actions"><span className="growth-local"><i aria-hidden="true" />{m.growth_local_workspace()}</span><GrowthLanguagePicker /><button className="growth-icon-button" onClick={() => setTheme(theme === "dark" ? "light" : "dark")} aria-label={m.growth_toggle_color_theme()}>{theme === "dark" ? "☀" : "◐"}</button></div>
    </header>
    <div className="growth-body">
      <nav className="growth-sidebar" aria-label={m.growth_product_workspaces()}>
        <Link to="/" className={activeProject ? "growth-nav-home" : "growth-nav-home active"}>↗ {m.growth_product_workspaces()}</Link>
        {!battleId && !projectId && <a href="#growth-settings" className="growth-nav-home">⚙ {m.growth_settings_nav()}</a>}
        <span className="growth-eyebrow">{growthYourLab()}</span>
        {workspaces.data?.map((item) => <Link key={item.projectId} to="/growth/workspaces/$projectId" params={{ projectId: item.projectId }} className={item.projectId === activeProject ? "growth-workspace-link active" : "growth-workspace-link"}><span className="growth-workspace-monogram">{item.config.product.name.slice(0, 1)}</span><span>{item.config.product.name}<small>{permissionModeLabel(item.config.permissions.mode)}</small></span></Link>)}
        {workspaces.data?.length === 0 && <p className="growth-muted">{m.growth_no_products()}</p>}
        <div className="growth-sidebar-foot"><p>{m.growth_sidebar_context()}<br />{m.growth_sidebar_hypotheses()}<br />{m.growth_sidebar_evidence()}</p><details><summary>{m.growth_about_alpha()}</summary><p>{m.growth_about_foundation()}</p><p>{m.growth_alpha_pending()}</p></details></div>
      </nav>
      <main className="growth-main">
        <div className="growth-page-top"><span className="growth-eyebrow">{battle ? m.growth_growth_battle() : workspace ? m.growth_product_workspace() : m.growth_experimentation_laboratory()}</span><span className="growth-connection">{streamConnected ? m.growth_connection_live() : m.growth_connection_connecting()}</span></div>
        {(error || queryError) && <div className="growth-error" role="alert"><span>{error ?? queryError?.message}</span><button onClick={() => { setError(null); void refresh(); }}>{m.growth_retry_reads()}</button></div>}
        {notice && <div className="growth-notice" role="status">{notice}</div>}
        {busy && <div className="growth-notice" role="status"><Spinner />{busy}…</div>}
        {workspaces.isPending ? <div className="growth-empty"><Spinner /><p>{m.growth_opening_local_workspace()}</p></div> : !activeProject ? <>
          <section className="growth-intro"><h1>{m.growth_intro_title_line_one()}<br />{m.growth_intro_title_line_two()}</h1><p>{m.growth_intro_body()}</p></section>
          <section className="growth-demo-launch" aria-labelledby="growth-demo-title">
            <div><span className="growth-eyebrow">{m.growth_no_api_key()}</span><h2 id="growth-demo-title">{m.growth_demo_title()}</h2><p>{m.growth_demo_body()}</p>{capabilities.data?.isolationAvailable === false && <p className="growth-error">{m.growth_os_validation_unavailable()}</p>}</div>
            <button className="growth-button min-h-11" disabled={!!busy || !capabilities.data?.isolationAvailable} onClick={() => void launchDemo()}>{m.growth_run_bundled_demo()} <span aria-hidden="true">↗</span></button>
          </section>
          <div className="growth-home-grid"><section className="growth-panel"><span className="growth-section-number">01 / {m.growth_start_with_product()}</span><h2>{m.growth_import_local_folder()}</h2><p>{m.growth_import_description()}</p><form onSubmit={importProduct}><label htmlFor="growth-import">{m.growth_label_repository_root()}</label><input id="growth-import" value={importPath} onChange={(event) => setImportPath(event.target.value)} placeholder="/path/to/your/product" required /><label className="growth-checkbox"><input type="checkbox" checked={initializeGit} onChange={(event) => setInitializeGit(event.target.checked)} />{m.growth_initialize_git_snapshot()}</label><button className="growth-button" disabled={!!busy || !importPath.trim()}>{m.growth_import_product()} <span aria-hidden="true">↗</span></button></form><details className="growth-help"><summary>{m.growth_prepare_configuration()}</summary><p>{m.growth_configuration_help_intro()}</p><p>{m.growth_configuration_help_policy()}</p></details><details className="growth-help"><summary>{m.growth_start_manual_brief()}</summary><p>{m.growth_manual_brief_description()}</p><form onSubmit={createManualBrief}><label htmlFor="growth-brief-name">{m.growth_label_product_name()}</label><input id="growth-brief-name" value={briefName} onChange={(event) => setBriefName(event.target.value)} maxLength={512} required /><label htmlFor="growth-brief-audience">{m.growth_label_audience()}</label><input id="growth-brief-audience" value={briefAudience} onChange={(event) => setBriefAudience(event.target.value)} maxLength={2048} required /><label htmlFor="growth-brief-goal">{m.growth_label_outcome()}</label><input id="growth-brief-goal" value={briefGoal} onChange={(event) => setBriefGoal(event.target.value)} maxLength={4096} required /><label htmlFor="growth-brief-description">{m.growth_label_description_optional()}</label><textarea id="growth-brief-description" value={briefDescription} onChange={(event) => setBriefDescription(event.target.value)} maxLength={8192} rows={3} /><label htmlFor="growth-brief-metric">{m.growth_label_primary_metric()}</label><input id="growth-brief-metric" value={briefMetric} onChange={(event) => setBriefMetric(event.target.value)} maxLength={256} required /><button className="growth-button" disabled={!!busy}>{m.growth_create_local_brief()} <span aria-hidden="true">↗</span></button></form></details></section><section className="growth-panel growth-principles"><span className="growth-section-number">02 / {m.growth_read_evidence()}</span><h2>{m.growth_candidate_earns_place()}</h2><p>{m.growth_principles_body()}</p><div><Mark value="SIMULATED" /><p>{m.growth_status_simulated_detail()}</p></div><div><Mark value="OBSERVED" /><p>{m.growth_status_observed_detail()}</p></div><div><Mark value="UNTESTED" /><p>{m.growth_status_untested_detail()}</p></div></section></div>
          <MeasurementPanel />
          <GrowthSettingsPanel workspaces={workspaces.data ?? []} activeWorkspace={workspace} />
          <section className="growth-url-audit growth-panel" aria-labelledby="growth-url-audit-title"><div className="growth-url-audit-intro"><span className="growth-section-number">{m.growth_page_section()}</span><h2 id="growth-url-audit-title">{m.growth_page_heading()}</h2><p>{m.growth_page_body()}</p></div><form onSubmit={auditWebsite} className="growth-url-audit-form"><label htmlFor="growth-website-url">{m.growth_public_page_url()}</label><input id="growth-website-url" type="url" value={websiteUrl} onChange={(event) => setWebsiteUrl(event.target.value)} placeholder="https://example.com/use-case" required /><button className="growth-button" disabled={!!busy || !websiteUrl.trim()}>{m.growth_review_page()} <span aria-hidden="true">↗</span></button></form>{websiteAudit && <div className="growth-url-audit-result" role="status"><div className="growth-url-audit-score"><span className="growth-eyebrow">{m.growth_estimated_page_hygiene()}</span><strong>{websiteAudit.rubric.score}<small> / {websiteAudit.rubric.maxScore}</small></strong><Mark value={websiteAudit.provenance} /></div><div><p><strong>{websiteAudit.url}</strong> · HTTP {websiteAudit.httpStatus} · robots: {websiteAudit.robots.allowed ? m.growth_robots_allowed() : m.growth_robots_blocked()}</p><p className="growth-muted">{m.growth_retrieved_page_summary({ date: new Date(websiteAudit.retrievedAt).toLocaleString() })}</p></div><div className="growth-url-audit-dimensions">{websiteAudit.rubric.dimensions.map((dimension) => <div key={dimension.key}><span>{dimension.label}</span><strong>{dimension.score}/{dimension.maxScore}</strong></div>)}</div><details><summary>{m.growth_how_to_read_review()}</summary><p>{websiteAudit.rubric.calculation}</p><ul>{websiteAudit.rubric.limitations.map((limitation) => <li key={limitation}>{limitation}</li>)}</ul></details>{websiteAudit.quality && <QualityRubricDetails rubric={websiteAudit.quality} />}{websiteAudit.accessibility && <QualityRubricDetails rubric={websiteAudit.accessibility} />}</div>}</section>
          <section className="growth-repository-audit growth-panel" aria-labelledby="growth-repository-audit-title"><div><span className="growth-section-number">{m.growth_repository_section()}</span><h2 id="growth-repository-audit-title">{m.growth_repository_heading()}</h2><p>{m.growth_repository_body()}</p></div><form onSubmit={auditRepository} className="growth-url-audit-form"><label htmlFor="growth-repository-url">{m.growth_public_repository_url()}</label><input id="growth-repository-url" type="url" value={repositoryUrl} onChange={(event) => setRepositoryUrl(event.target.value)} placeholder="https://github.com/owner/product" required /><button className="growth-button" disabled={!!busy || !repositoryUrl.trim()}>{m.growth_review_repository()} <span aria-hidden="true">↗</span></button></form>{repositoryAudit && <div className="growth-repository-audit-result" role="status"><div className="growth-line"><strong>{repositoryAudit.fullName}</strong><Mark value={repositoryAudit.provenance} /></div><p>{repositoryAudit.description ?? m.growth_no_public_description()}</p><p className="growth-muted">{m.growth_repository_branch({ value: repositoryAudit.defaultBranch ?? m.growth_not_reported() })} · {m.growth_repository_license({ value: repositoryAudit.license ?? m.growth_not_reported() })}{repositoryAudit.archived ? ` · ${m.growth_archived()}` : ""}{repositoryAudit.fork ? ` · ${m.growth_fork()}` : ""}</p><p className="growth-muted">{m.growth_repository_response_summary()}</p><details><summary>{m.growth_limits()}</summary><ul>{repositoryAudit.limitations.map((limitation) => <li key={limitation}>{limitation}</li>)}</ul></details><details className="growth-repository-import"><summary>{m.growth_import_repository_local()}</summary><p className="growth-muted">{m.growth_repository_import_body()}</p><form onSubmit={importRepository}><label htmlFor="growth-repository-path">{m.growth_new_local_folder()}</label><input id="growth-repository-path" value={repositoryImportPath} onChange={(event) => setRepositoryImportPath(event.target.value)} placeholder="./my-product" required /><label htmlFor="growth-repository-name">{m.growth_repository_product_name()}</label><input id="growth-repository-name" value={repositoryImportName} onChange={(event) => setRepositoryImportName(event.target.value)} maxLength={512} /><label htmlFor="growth-repository-audience">{m.growth_label_audience()}</label><input id="growth-repository-audience" value={repositoryImportAudience} onChange={(event) => setRepositoryImportAudience(event.target.value)} maxLength={2048} /><label htmlFor="growth-repository-goal">{m.growth_label_outcome()}</label><input id="growth-repository-goal" value={repositoryImportGoal} onChange={(event) => setRepositoryImportGoal(event.target.value)} maxLength={4096} /><label htmlFor="growth-repository-mode">{m.growth_new_contract_permission_mode()}</label><select id="growth-repository-mode" value={repositoryImportMode} onChange={(event) => setRepositoryImportMode(event.target.value as typeof repositoryImportMode)}><option value="analyze_only">{m.growth_analysis_only()}</option><option value="draft">Draft</option></select><button className="growth-button" disabled={!!busy || !repositoryImportPath.trim()}>{m.growth_clone_import_locally()} <span aria-hidden="true">↗</span></button></form></details></div>}</section>
          <section className="growth-playbooks" aria-labelledby="growth-playbooks-title"><div className="growth-playbooks-intro"><span className="growth-section-number">{m.growth_playbook_section()}</span><h2 id="growth-playbooks-title">{m.growth_playbook_heading()}</h2><p>{m.growth_playbook_body()}</p></div><div className="growth-playbook-grid">{(playbooks.data ?? []).map((playbook: GrowthPlaybook) => { const latest = playbookRuns.data?.find((run: PlaybookRun) => run.role === playbook.role); return <article className="growth-playbook-card" key={playbook.id}><div className="growth-line"><span className="growth-playbook-role">{roleLabel(playbook.role)}</span><span className="growth-playbook-template">{workspace ? m.growth_playbook_executable() : m.growth_playbook_template()}</span></div><h3>{playbook.title}</h3><p className="growth-playbook-focus">{playbook.focus}</p><p>{playbook.summary}</p><details><summary>{m.growth_questions_to_answer()}</summary><ul>{playbook.questions.map((question) => <li key={question}>{question}</li>)}</ul></details><details><summary>{m.growth_outputs_and_guardrails()}</summary><strong>{m.growth_outputs()}</strong><ul>{playbook.outputs.map((output) => <li key={output}>{output}</li>)}</ul><strong>{m.growth_guardrails()}</strong><ul>{playbook.guardrails.map((guardrail) => <li key={guardrail}>{guardrail}</li>)}</ul></details>{workspace && <button className="growth-button growth-button-secondary" disabled={!!busy} onClick={() => void runPlaybook(playbook.id)}>{m.growth_run_locally()} <span aria-hidden="true">→</span></button>}{latest && <details className="growth-playbook-latest"><summary>{m.growth_latest_local_run({ provenance: latest.provenance })}</summary><ul>{latest.responses.map((response) => <li key={response.question}><strong>{response.question}</strong><br />{response.answer}</li>)}</ul><p className="growth-muted">{latest.nextStep}</p></details>}<p className="growth-muted">{workspace ? m.growth_saved_local_role() : m.growth_reusable_template()}</p></article>; })}</div></section>
          {!!workspaces.data?.length && <section className="growth-recent"><h2>{m.growth_your_product_workspaces()}</h2>{workspaces.data.map((item) => <Link className="growth-recent-row" key={item.projectId} to="/growth/workspaces/$projectId" params={{ projectId: item.projectId }}><span>{item.config.product.name}<small>{item.config.product.audience}</small></span><span>{permissionModeLabel(item.config.permissions.mode)} <span aria-hidden="true">↗</span></span></Link>)}</section>}
          {!!battles.data?.length && <section className="growth-recent"><h2>{m.growth_recent_battles()}</h2>{battles.data.map((item) => <Link className="growth-recent-row" key={item.id} to="/growth/$battleId" params={{ battleId: item.id }}><span>{item.contract.goal}<small>{item.contract.config.product.name}</small></span><span>{item.status} <span aria-hidden="true">↗</span></span></Link>)}</section>}
        </> : !config ? <div className="growth-empty"><h1>{status.isPending ? m.growth_opening_battle() : m.growth_workspace_unavailable()}</h1><p>{m.growth_check_api_or_workspaces()}</p><Link to="/">{m.growth_product_workspaces()}</Link></div> : <>
          <div className="growth-workspace-heading"><div><h1>{battle ? battle.contract.goal : config.product.name}</h1><p>{config.product.audience} <span aria-hidden="true">/</span> {battle ? config.product.name : m.growth_landing_page_experimentation()}</p></div>{battle && <span className={`growth-status growth-status-${battle.status}`}>{battle.cancelRequested && running ? m.growth_cancelling() : battle.status}</span>}</div>
          <details className="growth-contract"><summary>{m.growth_contract_summary()} <span>{permissionModeLabel(config.permissions.mode)} · {config.validation.commands.length} {m.growth_contract_configured_checks()}</span></summary><div className="growth-contract-grid"><div><h3>{m.growth_contract_allowed_paths()}</h3><p>{config.permissions.allowed_paths.join(", ") || m.growth_contract_none()}</p><h3>{m.growth_contract_denied_paths()}</h3><p>{config.permissions.denied_paths.join(", ") || m.growth_contract_protected_paths()}</p></div><div><h3>{m.growth_contract_validation()}</h3>{config.validation.commands.map((command) => <code className="growth-command" key={command}>{command}</code>)}<p>{config.validation.timeout_seconds} {m.growth_contract_timeout()} · {config.agents.parallelism} {m.growth_contract_parallel_variants()}</p></div><dl><Digest label={m.growth_contract_baseline_commit()} value={battle?.contract.sourceSnapshotCommit ?? workspace?.sourceSnapshotCommit} /><Digest label={m.growth_contract_sha()} value={battle?.contractDigest} /></dl></div></details>
          {!battle && workspace && <GrowthSettingsPanel workspaces={workspaces.data ?? []} activeWorkspace={workspace} hypothesisCount={hypotheses.data?.length} hasActiveBattle={battles.data?.some((item) => item.projectId === workspace.projectId && (item.status === "ready" || item.status === "running"))} policyReady={!hypotheses.isPending && !battles.isPending} />}
          {!battle && workspace && <><section className="growth-experiment-map growth-panel" aria-labelledby="growth-experiment-map-title"><div className="growth-line"><div><span className="growth-section-number">{m.growth_experiment_map()}</span><h2 id="growth-experiment-map-title">{m.growth_experiment_map_heading()}</h2></div>{!hypotheses.isPending && !(hypotheses.data?.length) && <button className="growth-button growth-button-secondary" disabled={!!busy} onClick={() => void createHypotheses()}>{m.growth_create_three_hypotheses()} <span aria-hidden="true">→</span></button>}</div><p className="growth-muted">Each branch starts from the same committed product facts. The cards are untested until you run a battle and bring evidence.</p>{hypotheses.isPending ? <p className="growth-muted">{m.growth_loading_experiment_map()}</p> : hypotheses.data?.length ? <HypothesisTree hypotheses={hypotheses.data} goal={workspace.config.goal.primary} audience={workspace.config.product.audience} onChoose={(hypothesis) => { setGoal(hypothesis.hypothesis); requestAnimationFrame(() => document.getElementById("growth-goal")?.scrollIntoView({ behavior: "smooth", block: "center" })); }} onSave={updateHypothesis} /> : <p className="growth-muted">{m.growth_no_hypotheses()}</p>}</section><section className="growth-goal"><span className="growth-section-number">{m.growth_compose_battle()}</span><h2>{m.growth_one_goal_three_approaches()}</h2><p>Create three hypothesis templates and isolated worktrees from your imported baseline. Templates use committed product facts; they are untested.</p><form onSubmit={prepare}><label htmlFor="growth-goal">{m.growth_outcome_matters()}</label><textarea id="growth-goal" value={goal} onChange={(event) => setGoal(event.target.value)} rows={2} maxLength={4096} required /><button className="growth-button" disabled={!!busy || !goal.trim() || config.permissions.mode !== "implementation"}>{m.growth_prepare_landing_page_battle()} <span aria-hidden="true">→</span></button></form>{config.permissions.mode !== "implementation" && <p className="growth-muted">Worktree battles require implementation permission. Review and commit a suitable configuration, then import a new baseline. Analysis-only and draft playbooks remain in progress.</p>}</section><section className="growth-recent"><h2>{m.growth_this_products_battles()}</h2>{battles.data?.filter((item) => item.projectId === workspace.projectId).map((item) => <Link className="growth-recent-row" key={item.id} to="/growth/$battleId" params={{ battleId: item.id }}><span>{item.contract.goal}<small>{new Date(item.createdAt).toLocaleString()}</small></span><span>{item.status} ↗</span></Link>)}{!battles.data?.some((item) => item.projectId === workspace.projectId) && <p className="growth-muted">{m.growth_no_battles()}</p>}</section></>}
          {battle && <>
            <div className="growth-battle-actions"><p>{running ? m.growth_inspect_progress() : m.growth_review_evidence()}</p><div>{running && <button className="growth-button growth-button-secondary" disabled={!!busy} onClick={() => void perform(m.growth_busy_cancel(), () => growth.cancel(battle.id))}>{m.growth_cancel_battle()}</button>}{battle.status !== "ready" && !status.data?.controller?.running && <button className="growth-button growth-button-secondary" disabled={!!busy} onClick={() => void perform(m.growth_busy_recovery(), async () => {const result = await growth.recover(battle.id); if (mounted.current && activeView.current === viewKey) setNotice(`Recovery: ${result.status}. ${result.warnings.join(" ")}`); return result;})}>{m.growth_inspect_recovery()}</button>}</div></div>
            {status.data?.controller?.error && <p className="growth-error" role="alert">{status.data.controller.error}</p>}
            {battle.status === "ready" && !status.data?.controller?.running && <section className="growth-execution"><div><span className="growth-section-number">{m.growth_run_authorize_execution()}</span><h2>{m.growth_choose_proposal_source()}</h2><p>{m.growth_execution_contract()}</p>{capabilities.data?.isolationAvailable === false && <p className="growth-error">{m.growth_isolation_unavailable()}</p>}</div><form onSubmit={execute}><label htmlFor="growth-execution-mode">{m.growth_execution_mode()}</label><select id="growth-execution-mode" value={executionMode} onChange={(event) => setExecutionMode(event.target.value as typeof executionMode)}><option value="replay">{m.growth_replay_option()}</option><option value="native">{m.growth_native_option()}</option></select>{executionMode === "replay" ? <><label htmlFor="growth-replay">{m.growth_replay_json_plan()}</label><textarea className="growth-json" id="growth-replay" value={replay} onChange={(event) => setReplay(event.target.value)} rows={5} spellCheck={false} required /><label className="growth-file-label">{m.growth_load_replay_file()}<input type="file" accept=".json,application/json" onChange={(event) => {const file = event.target.files?.[0]; if (!file) return; if (file.size > 1024 * 1024) {setError(m.growth_replay_too_large()); return;} void file.text().then((text) => {if (mounted.current && activeView.current === viewKey && isCurrentScope(scope)) setReplay(text);}).catch(() => {if (mounted.current && activeView.current === viewKey && isCurrentScope(scope)) setError(m.growth_replay_read_failed());});}} /></label><p className="growth-muted">{m.growth_replay_description()}</p></> : <><label htmlFor="growth-harness">{m.growth_native_harness()}</label><select id="growth-harness" value={harness} onChange={(event) => setHarness(event.target.value)} required><option value="">{m.growth_select_adapter()}</option>{capabilities.data?.harnesses.filter((item) => item.toolsDisabledProposals).map((item) => <option key={item.id} value={item.id}>{item.id}</option>)}</select><label htmlFor="growth-model">{m.growth_requested_model()}</label><input id="growth-model" value={model} onChange={(event) => setModel(event.target.value)} maxLength={256} /><label className="growth-checkbox"><input type="checkbox" checked={providerConsent} onChange={(event) => setProviderConsent(event.target.checked)} />{m.growth_provider_consent()}</label><p className="growth-muted">{m.growth_native_unverified()}</p></>}<button className="growth-button" disabled={!!busy || !capabilities.data?.isolationAvailable || (executionMode === "native" && (!harness || !providerConsent))}>{m.growth_run_three_variants()} <span aria-hidden="true">→</span></button></form></section>}
            <section className="growth-competitors" aria-label={m.growth_competing_variants()}>{variants.map((item, index) => {
              const hypothesis = portfolio.find((hypothesis) => hypothesis.id === item.hypothesisId)!;
              const captured = variantRun(status.data, item.id);
              const row = comparison.data?.rows.find((row) => row.variantId === item.id);
              return <article className={`growth-competitor ${variant?.id === item.id ? "is-focused" : ""}`} key={item.id}>
                <div className="growth-line"><span className="growth-variant-number">{String(index + 1).padStart(2, "0")}</span><span className={`growth-status ${row?.eligible ? "growth-status-completed" : ""}`}>{selected === item.id ? m.growth_selected_candidate() : row?.eligible ? m.growth_eligible_candidate() : runPhase(captured?.run)}</span></div>
                <button className="growth-variant-title" onClick={() => {setFocused(item.id); setTab("evidence");}} aria-pressed={variant?.id === item.id}><h2>{hypothesis.title}</h2><span aria-hidden="true">↗</span></button><p>{hypothesis.hypothesis}</p>{row?.hypothesisId && <p className="growth-muted growth-lineage">{m.growth_hypothesis_id()}: <code>{row.hypothesisId}</code></p>}
                <div className="growth-check-fraction"><span>{m.growth_configured_checks()}</span><strong>{captured?.run.validations.length ? `${captured.run.validations.filter((check) => check.status === "done" && check.exitCode === 0).length} / ${config.validation.commands.length}` : "—"}</strong></div>
                <details className="growth-checks"><summary>{m.growth_inspect_checks_calculation()}</summary><p>{comparison.data?.calculation ?? m.growth_check_calculation_fallback()}</p>{captured?.run.validations.map((check) => <div className="growth-check" key={check.runId}><code>{check.command}</code><div className="growth-line"><strong>{check.status} · {m.growth_exit()} {check.exitCode ?? m.growth_unavailable()}</strong><Mark value={check.provenance} /></div><p>{check.terminationReason}</p><p className="growth-muted">{check.limitation}</p><dl><Digest label={m.growth_source_sha()} value={check.sourceDigest} /><Digest label={m.growth_contract_isolation()} value={check.confinement?.backend} /><Digest label={m.growth_contract_policy_sha()} value={check.confinement?.policyDigest} /></dl></div>)}{!captured?.run.validations.length && <p>{m.growth_no_completed_checks()}</p>}{captured?.run.activeValidation && <p className="growth-notice">{m.growth_command_registered({ index: captured.run.activeValidation.commandIndex + 1 })}</p>}</details>
                {row?.rubric && <details className="growth-seo-rubric"><summary><span>{row.rubric.label}</span><strong>{row.rubric.score} / {row.rubric.maxScore} · {row.rubric.provenance}</strong></summary><p className="growth-muted">{row.rubric.calculation}</p><div className="growth-seo-dimensions">{row.rubric.dimensions.map((dimension) => <article className={`growth-seo-dimension growth-seo-dimension-${dimension.status}`} key={dimension.key}><div className="growth-line"><strong>{dimension.label}</strong><span>{dimension.score} / {dimension.maxScore}</span></div>{dimension.evidence.map((evidence) => <p key={evidence}>{evidence}</p>)}</article>)}</div><div className="growth-seo-recommendations"><strong>{m.growth_rubric_suggested_steps()}</strong><ul>{row.rubric.recommendations.length ? row.rubric.recommendations.map((recommendation) => <li key={recommendation}>{recommendation}</li>) : <li>{m.growth_rubric_no_structural_gaps()}</li>}</ul></div><ul className="growth-seo-limitations">{row.rubric.limitations.map((limitation) => <li key={limitation}>{limitation}</li>)}</ul></details>}
                {row?.quality && <QualityRubricDetails rubric={row.quality} />}
                {row?.accessibility && <QualityRubricDetails rubric={row.accessibility} />}
                {row?.render && <QualityRubricDetails rubric={row.render} />}
                {row?.performance && <QualityRubricDetails rubric={row.performance} />}
                {captured?.run.error && <p className="growth-check-error">{captured.run.error}</p>}
                <div className="growth-provenance"><div><span>{m.growth_proposal()}</span><Mark value={captured?.run.provenance ?? "UNTESTED"} /></div><div><span>{m.growth_growth_outcome()}</span><Mark value={captured?.run.outcomeProvenance ?? "UNTESTED"} /></div><div><span>{m.growth_confidence()}</span><strong>{row?.confidence.label ?? captured?.run.confidence.label ?? hypothesis.confidence.label}</strong></div></div>
                <p className="growth-muted growth-confidence">{row?.confidence.rationale ?? hypothesis.confidence.rationale}</p><button className="growth-text-button" onClick={() => {setFocused(item.id); setTab("artifacts");}}>{m.growth_battle_inspect_diff()}</button>
              </article>;
            })}</section>
            {variant && hypothesis && <section className="growth-inspector"><div className="growth-inspector-top"><div><span className="growth-eyebrow">{m.growth_battle_variant_inspection()}</span><h2>{hypothesis.title}</h2></div><span className="growth-muted">{capture?.sealed ? m.growth_sealed_archive() : capture?.digest ? m.growth_live_checkpoint() : m.growth_prepared_hypothesis()}</span></div><div className="growth-tab-bar" role="tablist" aria-label={m.growth_battle_variant_inspection()}><button className="min-h-11" ref={evidenceTab} onKeyDown={tabKey} tabIndex={tab === "evidence" ? 0 : -1} role="tab" aria-selected={tab === "evidence"} aria-controls="growth-evidence-panel" id="growth-evidence-tab" onClick={() => setTab("evidence")}>{m.growth_battle_hypothesis_evidence()}</button><button className="min-h-11" ref={artifactsTab} onKeyDown={tabKey} tabIndex={tab === "artifacts" ? 0 : -1} role="tab" aria-selected={tab === "artifacts"} aria-controls="growth-artifact-panel" id="growth-artifact-tab" onClick={() => setTab("artifacts")}>{m.growth_battle_diff_files_logs()}</button><button className="min-h-11" ref={previewTab} onKeyDown={tabKey} tabIndex={tab === "preview" ? 0 : -1} role="tab" aria-selected={tab === "preview"} aria-controls="growth-preview-panel" id="growth-preview-tab" onClick={() => setTab("preview")}>{m.growth_battle_static_preview()}</button></div>
              {tab === "evidence" ? <div role="tabpanel" id="growth-evidence-panel" aria-labelledby="growth-evidence-tab"><HypothesisEvidence hypothesis={hypothesis} /></div> : tab === "preview" ? <div role="tabpanel" id="growth-preview-panel" aria-labelledby="growth-preview-tab"><StaticPreviewPanel key={variant.id} variantId={variant.id} record={capture?.run.staticPreview} digest={capture?.digest ?? null} configured={!!config?.static_preview} trusted={evidenceTrusted} /></div> : <div role="tabpanel" id="growth-artifact-panel" aria-labelledby="growth-artifact-tab">{!capture?.digest ? <p className="growth-muted">{m.growth_artifacts_before_checkpoint()}</p> : artifacts.isPending ? <Spinner /> : artifacts.error ? <p role="alert">{artifacts.error.message}</p> : !artifacts.data?.artifacts.length ? <p className="growth-muted">{m.growth_no_checkpoint_artifacts()}</p> : <><label htmlFor="growth-artifact-name">{m.growth_captured_artifact()}</label><select id="growth-artifact-name" value={activeArtifact?.name ?? ""} onChange={(event) => setArtifactName(event.target.value)}>{artifacts.data.artifacts.map((item) => <option key={item.name} value={item.name}>{item.name} · {item.size.toLocaleString()} bytes</option>)}</select>{artifact.isPending ? <Spinner /> : artifact.error ? <p role="alert">{artifact.error.message}</p> : <><pre className="growth-artifact">{artifact.data?.text}</pre><dl><Digest label={m.growth_artifact_sha()} value={artifact.data?.digest} /><Digest label={artifact.data?.sealed ? m.growth_archive_sha() : m.growth_checkpoint_sha()} value={artifact.data?.archiveDigest} /></dl></>}</>}</div>}
              <details className="growth-local-metadata"><summary>{m.growth_local_repro_metadata()}</summary><dl><Digest label={m.growth_variant_id()} value={variant.id} /><Digest label={m.growth_branch()} value={variant.branchName} /><Digest label={m.growth_local_worktree()} value={variant.worktree} /><Digest label={m.growth_candidate_commit()} value={capture?.run.candidateCommit} /><Digest label={m.growth_agent_harness()} value={capture?.run.agent.harness} /><Digest label={m.growth_requested_model_label()} value={capture?.run.agent.requestedModel} /><Digest label={m.growth_cost_tokens()} value={capture?.run.agent.cost == null && capture?.run.agent.tokens == null ? m.growth_not_supplied_adapter() : `${capture?.run.agent.cost ?? m.growth_unknown()} / ${capture?.run.agent.tokens ?? m.growth_unknown()}`} /></dl></details>
              <div className="growth-delivery"><div><span className="growth-eyebrow">{m.growth_battle_your_decision()}</span><h3>{eligible ? selected === variant.id ? m.growth_battle_ready_delivery() : m.growth_battle_eligible_review() : m.growth_battle_no_eligible()}</h3><p>{m.growth_battle_selection_review()}</p></div><button className="growth-button" disabled={!!busy || !eligible || selected === variant.id} onClick={() => void perform(m.growth_busy_select(), () => growth.select(variant.id))}>{selected === variant.id ? m.growth_battle_candidate_selected() : m.growth_battle_select_candidate()}</button></div>
              {selected === variant.id && <div className="growth-delivery-controls"><form onSubmit={(event) => {event.preventDefault(); void perform(m.growth_busy_export(), async () => {const result = await growth.export(variant.id, exportPath.trim()); if (mounted.current && activeView.current === viewKey) setNotice(m.growth_export_notice()); return result;});}}><label htmlFor="growth-export">{m.growth_new_patch_path()}</label><input id="growth-export" value={exportPath} onChange={(event) => setExportPath(event.target.value)} placeholder="/path/to/new-selected.patch" required /><button className="growth-button growth-button-secondary" disabled={!!busy || !evidenceTrusted || !exportPath.trim()}>{m.growth_export_patch()}</button></form><div><p>{m.growth_review_exact_change()}</p><button className="growth-button growth-button-secondary" disabled={!!busy || !evidenceTrusted} onClick={() => void perform(m.growth_busy_review_apply(), async () => {const result = await growth.preview(variant.id); if (mounted.current && activeView.current === viewKey) {setPreview(result); setApplyConsent(false);} return result;})}>{m.growth_review_selected_apply()}</button></div></div>}
              {preview && <section className="growth-apply-review" aria-label={m.growth_apply_review()}><h3>{m.growth_apply_selected_patch()}</h3><p>{preview.operation}</p><ul>{preview.changedFiles.map((path) => <li key={path}><code>{path}</code></li>)}</ul><dl><Digest label={m.growth_product_baseline()} value={preview.sourceCommit} /><Digest label={m.growth_candidate_commit()} value={preview.candidateCommit} /><Digest label={m.growth_patch_sha()} value={preview.patchDigest} /></dl><label className="growth-checkbox"><input type="checkbox" checked={applyConsent} onChange={(event) => setApplyConsent(event.target.checked)} />{m.growth_apply_consent()}</label><div className="growth-line"><button className="growth-button" disabled={!!busy || !evidenceTrusted || !applyConsent} onClick={() => void perform(m.growth_busy_apply(), async () => {const result = await growth.apply(preview.variantId); if (mounted.current && activeView.current === viewKey) {setPreview(null); setNotice(m.growth_apply_notice());} return result;})}>{m.growth_apply_selected_changes()}</button><button className="growth-text-button" disabled={!!busy} onClick={() => setPreview(null)}>{m.growth_close_preview()}</button></div></section>}
            </section>}
            {!!status.data?.runs.length && <section className="growth-share"><div><span className="growth-eyebrow">{m.growth_battle_take_evidence()}</span><h2>{m.growth_battle_report_title()}</h2><p>{m.growth_battle_report_body()}</p></div><div><button className="growth-button growth-button-secondary" disabled={!!busy} onClick={() => void downloadReport("html")}>{m.growth_battle_download_html()}</button><button className="growth-button growth-button-secondary" disabled={!!busy} onClick={() => void downloadReport("markdown")}>{m.growth_battle_download_markdown()}</button></div></section>}
          </>}
        </>}
        <footer className="growth-footer"><span>{m.growth_footer_brand()}</span><span>{m.growth_footer_limits()}</span></footer>
      </main>
    </div>
  </div>;
}
