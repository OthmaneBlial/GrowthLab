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
function roleLabel(role: string) { return role.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase()); }
function Digest({ label, value }: { label: string; value?: string | null }) {
  return <div className="growth-digest"><dt>{label}</dt><dd>{value ?? "Not recorded"}</dd></div>;
}
function HypothesisEvidence({ hypothesis }: { hypothesis: Hypothesis }) {
  return <div className="growth-evidence">
    <div><span className="growth-eyebrow">Proposed mechanism</span><p>{hypothesis.mechanism}</p></div>
    <div><span className="growth-eyebrow">Outcome contract</span><p>{hypothesis.primaryMetric}</p><p className="growth-muted">{hypothesis.baselineDefinition}</p><p className="growth-muted">Success threshold: {hypothesis.successThreshold ?? "Not supplied"}</p></div>
    {hypothesis.evidence.map((evidence) => <article key={evidence.id}>
      <div className="growth-line"><strong>{evidence.title}</strong><Mark value={evidence.provenance} /></div>
      <p>{evidence.observation}</p><code>{evidence.source}</code>
      <p className="growth-muted">{evidence.publisher} · {new Date(evidence.retrievedAt).toLocaleDateString()}</p>
      <p className="growth-muted">{evidence.limitations}</p>
    </article>)}
    <div><span className="growth-eyebrow">Skeptic's notes</span><ul>{hypothesis.risks.map((risk) => <li key={risk}>{risk}</li>)}</ul></div>
  </div>;
}
function QualityRubricDetails({ rubric }: { rubric: PageQualityRubric | RenderRubric | PerformanceRubric | AccessibilityRubric }) {
  return <details className="growth-seo-rubric growth-quality-rubric"><summary><span>{rubric.label}</span><strong>{rubric.score} / {rubric.maxScore} · {rubric.provenance}</strong></summary><p className="growth-muted">{rubric.calculation}</p><div className="growth-seo-dimensions">{rubric.dimensions.map((dimension) => <article className={`growth-seo-dimension growth-seo-dimension-${dimension.status}`} key={dimension.key}><div className="growth-line"><strong>{dimension.label}</strong><span>{dimension.score} / {dimension.maxScore}</span></div>{dimension.evidence.map((evidence) => <p key={evidence}>{evidence}</p>)}</article>)}</div><div className="growth-seo-recommendations"><strong>Suggested next steps</strong><ul>{rubric.recommendations.length ? rubric.recommendations.map((recommendation) => <li key={recommendation}>{recommendation}</li>) : <li>No gaps were found by this local review.</li>}</ul></div><ul className="growth-seo-limitations">{rubric.limitations.map((limitation) => <li key={limitation}>{limitation}</li>)}</ul></details>;
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
    <div className="growth-hypothesis-root"><span className="growth-eyebrow">Current goal</span><strong>{goal}</strong><small>{audience}</small></div>
    <ol className="growth-hypothesis-branches" aria-label="Growth hypothesis branches">
      {[...groups.entries()].map(([role, items]) => <li className="growth-hypothesis-branch" key={role}>
        <details open><summary><span>{roleLabel(role)}</span><small>{items.length} hypothesis{items.length === 1 ? "" : "es"}</small></summary>
          <div className="growth-hypothesis-children">{items.map((item) => <article key={item.id} className="growth-hypothesis-node"><div className="growth-line"><span className="growth-playbook-role">{roleLabel(item.role)}</span><Mark value={item.provenance} /></div><h3 id={`growth-hypothesis-${item.id}`}>{item.title}</h3><p>{item.hypothesis}</p><p className="growth-muted">Mechanism: {item.mechanism}</p><small>{item.channel} · metric {item.primaryMetric}</small><div className="growth-hypothesis-actions"><button type="button" className="growth-button growth-button-secondary" onClick={() => onChoose(item)}>Use this branch <span aria-hidden="true">↓</span></button><button type="button" className="growth-button growth-button-secondary" onClick={() => { setEditingId(item.id); setDraft(hypothesisDraft(item)); }}>Edit branch</button><code title={item.id}>ID {item.id.slice(0, 8)}</code></div>{editingId === item.id && draft && <form className="growth-hypothesis-editor" onSubmit={(event) => void save(event, item.id)}><div className="growth-editor-heading"><strong>Edit hypothesis</strong><span className="growth-muted">Identity, role and evidence stay fixed.</span></div><label htmlFor={`growth-edit-title-${item.id}`}>Branch title<input id={`growth-edit-title-${item.id}`} value={draft.title} onChange={(event) => setDraft({ ...draft, title: event.target.value })} maxLength={4096} required /></label><label htmlFor={`growth-edit-hypothesis-${item.id}`}>Hypothesis<textarea id={`growth-edit-hypothesis-${item.id}`} value={draft.hypothesis} onChange={(event) => setDraft({ ...draft, hypothesis: event.target.value })} maxLength={4096} rows={3} required /></label><label htmlFor={`growth-edit-mechanism-${item.id}`}>Mechanism<textarea id={`growth-edit-mechanism-${item.id}`} value={draft.mechanism} onChange={(event) => setDraft({ ...draft, mechanism: event.target.value })} maxLength={4096} rows={2} required /></label><label htmlFor={`growth-edit-baseline-${item.id}`}>Baseline definition<input id={`growth-edit-baseline-${item.id}`} value={draft.baselineDefinition} onChange={(event) => setDraft({ ...draft, baselineDefinition: event.target.value })} maxLength={4096} required /></label><label htmlFor={`growth-edit-metric-${item.id}`}>Primary metric<input id={`growth-edit-metric-${item.id}`} value={draft.primaryMetric} onChange={(event) => setDraft({ ...draft, primaryMetric: event.target.value })} maxLength={4096} required /></label><label htmlFor={`growth-edit-threshold-${item.id}`}>Success threshold (optional)<input id={`growth-edit-threshold-${item.id}`} value={draft.successThreshold ?? ""} onChange={(event) => setDraft({ ...draft, successThreshold: event.target.value || null })} maxLength={4096} /></label><label htmlFor={`growth-edit-guardrails-${item.id}`}>Guardrail metrics (one per line)<textarea id={`growth-edit-guardrails-${item.id}`} value={draft.guardrailMetrics.join("\n")} onChange={(event) => setDraft({ ...draft, guardrailMetrics: event.target.value.split("\n").map((value) => value.trim()).filter(Boolean) })} rows={2} /></label><label htmlFor={`growth-edit-risks-${item.id}`}>Risks (one per line)<textarea id={`growth-edit-risks-${item.id}`} value={draft.risks.join("\n")} onChange={(event) => setDraft({ ...draft, risks: event.target.value.split("\n").map((value) => value.trim()).filter(Boolean) })} rows={2} /></label><div className="growth-hypothesis-actions"><button type="submit" className="growth-button" disabled={editingId !== item.id}>Save branch</button><button type="button" className="growth-button growth-button-secondary" onClick={() => { setEditingId(null); setDraft(null); }}>Cancel</button></div></form>}</article>)}</div>
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
    const result = await perform("Importing product", () => growth.import(importPath.trim(), initializeGit));
    if (result && mounted.current && activeView.current === viewKey) void navigate({ to: "/growth/workspaces/$projectId", params: { projectId: result.projectId } });
  }
  async function auditWebsite(event: React.FormEvent) {
    event.preventDefault();
    const result = await perform("Reviewing public page", () => growth.urlAudit(websiteUrl.trim()));
    if (result && mounted.current && activeView.current === viewKey) setWebsiteAudit(result);
  }
  async function auditRepository(event: React.FormEvent) {
    event.preventDefault();
    const result = await perform("Reviewing public repository", () => growth.repositoryAudit(repositoryUrl.trim()));
    if (result && mounted.current && activeView.current === viewKey) setRepositoryAudit(result);
  }
  async function importRepository(event: React.FormEvent) {
    event.preventDefault();
    const result = await perform("Cloning public repository locally", () => growth.repositoryImport({
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
    const result = await perform("Creating local product brief", () => growth.createBrief({
      name: briefName.trim(), audience: briefAudience.trim(), goal: briefGoal.trim(),
      description: briefDescription.trim(), metric: briefMetric.trim(),
    }));
    if (result && mounted.current && activeView.current === viewKey) void navigate({ to: "/growth/workspaces/$projectId", params: { projectId: result.projectId } });
  }
  async function runPlaybook(role: string) {
    if (!workspace) return;
    await perform(`Running ${role} playbook`, () => growth.runPlaybook(workspace.projectId, role));
  }
  async function createHypotheses() {
    if (!workspace) return;
    await perform("Creating the first experiment map", () => growth.createHypotheses(workspace.projectId));
  }
  async function updateHypothesis(id: string, draft: HypothesisUpdate): Promise<boolean> {
    if (!workspace) return false;
    const result = await perform("Saving hypothesis", () => growth.updateHypothesis(workspace.projectId, id, draft));
    return !!result;
  }
  async function launchDemo() {
    const result = await perform("Preparing the fictional replay battle", () => growth.demo());
    if (result && mounted.current && activeView.current === viewKey) void navigate({ to: "/growth/$battleId", params: { battleId: result.battleId } });
  }
  async function prepare(event: React.FormEvent) {
    event.preventDefault(); if (!workspace) return;
    const result = await perform("Preparing isolated variants", () => growth.prepare(workspace.projectId, goal));
    if (result && mounted.current && activeView.current === viewKey) void navigate({ to: "/growth/$battleId", params: { battleId: result.battle.id } });
  }
  async function execute(event: React.FormEvent) {
    event.preventDefault(); if (!battle) return;
    await perform("Submitting battle", async () => {
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
    await perform("Preparing private-context-free report", async () => {
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
      <Link to="/" className="growth-brand" aria-label="GrowthLab home"><svg width="27" height="27" viewBox="0 0 27 27" aria-hidden="true"><path d="M5 4v10m0-5h10v6m-10-1h17v7" fill="none" stroke="currentColor" strokeWidth="1.6" /><circle cx="5" cy="4" r="2.4" /><circle cx="15" cy="15" r="2.4" /><circle cx="22" cy="21" r="2.4" /></svg>GrowthLab<span className="growth-alpha">SOURCE ALPHA</span></Link>
      <div className="growth-header-actions"><span className="growth-local"><i aria-hidden="true" />{m.growth_local_workspace()}</span><GrowthLanguagePicker /><button className="growth-icon-button" onClick={() => setTheme(theme === "dark" ? "light" : "dark")} aria-label="Toggle color theme">{theme === "dark" ? "☀" : "◐"}</button></div>
    </header>
    <div className="growth-body">
      <nav className="growth-sidebar" aria-label="Product workspaces">
        <Link to="/" className={activeProject ? "growth-nav-home" : "growth-nav-home active"}>↗ {m.growth_product_workspaces()}</Link>
        {!battleId && !projectId && <a href="#growth-settings" className="growth-nav-home">⚙ {m.growth_settings_nav()}</a>}
        <span className="growth-eyebrow">{growthYourLab()}</span>
        {workspaces.data?.map((item) => <Link key={item.projectId} to="/growth/workspaces/$projectId" params={{ projectId: item.projectId }} className={item.projectId === activeProject ? "growth-workspace-link active" : "growth-workspace-link"}><span className="growth-workspace-monogram">{item.config.product.name.slice(0, 1)}</span><span>{item.config.product.name}<small>{item.config.permissions.mode.replaceAll("_", " ")}</small></span></Link>)}
        {workspaces.data?.length === 0 && <p className="growth-muted">{m.growth_no_products()}</p>}
        <div className="growth-sidebar-foot"><p>One context.<br />Competing hypotheses.<br />Inspectable evidence.</p><details><summary>About this alpha</summary><p>Built from OpenResearch's open-source foundation. MIT licensed; no alphaXiv endorsement.</p><p>Native execution, broader evaluators, the bundled visual demo and release packaging are still in progress.</p></details></div>
      </nav>
      <main className="growth-main">
        <div className="growth-page-top"><span className="growth-eyebrow">{battle ? "Growth Battle" : workspace ? "Product workspace" : m.growth_experimentation_laboratory()}</span><span className="growth-connection">{streamConnected ? m.growth_connection_live() : m.growth_connection_connecting()}</span></div>
        {(error || queryError) && <div className="growth-error" role="alert"><span>{error ?? queryError?.message}</span><button onClick={() => { setError(null); void refresh(); }}>Retry reads</button></div>}
        {notice && <div className="growth-notice" role="status">{notice}</div>}
        {busy && <div className="growth-notice" role="status"><Spinner />{busy}…</div>}
        {workspaces.isPending ? <div className="growth-empty"><Spinner /><p>Opening your local lab…</p></div> : !activeProject ? <>
          <section className="growth-intro"><h1>{m.growth_intro_title_line_one()}<br />{m.growth_intro_title_line_two()}</h1><p>{m.growth_intro_body()}</p></section>
          <section className="growth-demo-launch" aria-labelledby="growth-demo-title">
            <div><span className="growth-eyebrow">{m.growth_no_api_key()}</span><h2 id="growth-demo-title">{m.growth_demo_title()}</h2><p>{m.growth_demo_body()}</p>{capabilities.data?.isolationAvailable === false && <p className="growth-error">OS validation isolation is unavailable on this host. Demo execution is refused.</p>}</div>
            <button className="growth-button min-h-11" disabled={!!busy || !capabilities.data?.isolationAvailable} onClick={() => void launchDemo()}>{m.growth_run_bundled_demo()} <span aria-hidden="true">↗</span></button>
          </section>
          <div className="growth-home-grid"><section className="growth-panel"><span className="growth-section-number">01 / {m.growth_start_with_product()}</span><h2>{m.growth_import_local_folder()}</h2><p>Use a reviewed <code>growthlab.yaml</code>. Import records the brief and baseline without publishing your product.</p><form onSubmit={importProduct}><label htmlFor="growth-import">Repository or folder root</label><input id="growth-import" value={importPath} onChange={(event) => setImportPath(event.target.value)} placeholder="/path/to/your/product" required /><label className="growth-checkbox"><input type="checkbox" checked={initializeGit} onChange={(event) => setInitializeGit(event.target.checked)} />Initialize a local Git snapshot if this folder is not already a repository.</label><button className="growth-button" disabled={!!busy || !importPath.trim()}>{m.growth_import_product()} <span aria-hidden="true">↗</span></button></form><details className="growth-help"><summary>Prepare the configuration</summary><p>Run <code>growthlab init --help</code> in your product, choose permissions and validation commands, then review <code>growthlab.yaml</code>.</p><p>With the option enabled, GrowthLab stages the folder, rejects protected paths such as credentials and <code>.env</code>, creates one local commit and configures no remote. Existing repositories are never auto-committed.</p></details><details className="growth-help"><summary>Start from a manual brief</summary><p>Describe the product and the outcome you want to improve. GrowthLab creates a private local analysis workspace with no provider request and no publication.</p><form onSubmit={createManualBrief}><label htmlFor="growth-brief-name">Product name</label><input id="growth-brief-name" value={briefName} onChange={(event) => setBriefName(event.target.value)} maxLength={512} required /><label htmlFor="growth-brief-audience">Audience</label><input id="growth-brief-audience" value={briefAudience} onChange={(event) => setBriefAudience(event.target.value)} maxLength={2048} required /><label htmlFor="growth-brief-goal">Outcome to improve</label><input id="growth-brief-goal" value={briefGoal} onChange={(event) => setBriefGoal(event.target.value)} maxLength={4096} required /><label htmlFor="growth-brief-description">What does it do? (optional)</label><textarea id="growth-brief-description" value={briefDescription} onChange={(event) => setBriefDescription(event.target.value)} maxLength={8192} rows={3} /><label htmlFor="growth-brief-metric">Primary metric</label><input id="growth-brief-metric" value={briefMetric} onChange={(event) => setBriefMetric(event.target.value)} maxLength={256} required /><button className="growth-button" disabled={!!busy}>Create local brief <span aria-hidden="true">↗</span></button></form></details></section><section className="growth-panel growth-principles"><span className="growth-section-number">02 / {m.growth_read_evidence()}</span><h2>{m.growth_candidate_earns_place()}</h2><p>File changes run in isolated worktrees. Validation runs on recorded candidate snapshots. Every finished attempt is sealed for inspection.</p><div><Mark value="SIMULATED" /><p>Declared replay proposals; real edits still execute.</p></div><div><Mark value="OBSERVED" /><p>Actual configured checks and their exit codes.</p></div><div><Mark value="UNTESTED" /><p>Growth outcomes until real telemetry is supplied.</p></div></section></div>
          <MeasurementPanel />
          <GrowthSettingsPanel />
          <section className="growth-url-audit growth-panel" aria-labelledby="growth-url-audit-title"><div className="growth-url-audit-intro"><span className="growth-section-number">03 / CHECK A PUBLIC PAGE</span><h2 id="growth-url-audit-title">Review one page before you write more.</h2><p>Paste a public HTTPS page to get an inspectable structural SEO review. GrowthLab checks <code>robots.txt</code>, makes one read-only request and never claims rankings or conversions.</p></div><form onSubmit={auditWebsite} className="growth-url-audit-form"><label htmlFor="growth-website-url">Public page URL</label><input id="growth-website-url" type="url" value={websiteUrl} onChange={(event) => setWebsiteUrl(event.target.value)} placeholder="https://example.com/use-case" required /><button className="growth-button" disabled={!!busy || !websiteUrl.trim()}>Review page <span aria-hidden="true">↗</span></button></form>{websiteAudit && <div className="growth-url-audit-result" role="status"><div className="growth-url-audit-score"><span className="growth-eyebrow">Estimated page hygiene</span><strong>{websiteAudit.rubric.score}<small> / {websiteAudit.rubric.maxScore}</small></strong><Mark value={websiteAudit.provenance} /></div><div><p><strong>{websiteAudit.url}</strong> · HTTP {websiteAudit.httpStatus} · robots: {websiteAudit.robots.allowed ? "allowed" : "blocked"}</p><p className="growth-muted">Retrieved {new Date(websiteAudit.retrievedAt).toLocaleString()} · one page request · no credentials or redirects</p></div><div className="growth-url-audit-dimensions">{websiteAudit.rubric.dimensions.map((dimension) => <div key={dimension.key}><span>{dimension.label}</span><strong>{dimension.score}/{dimension.maxScore}</strong></div>)}</div><details><summary>How to read this review</summary><p>{websiteAudit.rubric.calculation}</p><ul>{websiteAudit.rubric.limitations.map((limitation) => <li key={limitation}>{limitation}</li>)}</ul></details>{websiteAudit.quality && <QualityRubricDetails rubric={websiteAudit.quality} />}{websiteAudit.accessibility && <QualityRubricDetails rubric={websiteAudit.accessibility} />}</div>}</section>
          <section className="growth-repository-audit growth-panel" aria-labelledby="growth-repository-audit-title"><div><span className="growth-section-number">04 / START FROM A PUBLIC PRODUCT</span><h2 id="growth-repository-audit-title">Review a public repository.</h2><p>Check the public description, branch and license before deciding what product context to bring locally. GrowthLab reads one GitHub metadata response; it never clones or runs source code until you explicitly choose a new local destination.</p></div><form onSubmit={auditRepository} className="growth-url-audit-form"><label htmlFor="growth-repository-url">Public repository URL</label><input id="growth-repository-url" type="url" value={repositoryUrl} onChange={(event) => setRepositoryUrl(event.target.value)} placeholder="https://github.com/owner/product" required /><button className="growth-button" disabled={!!busy || !repositoryUrl.trim()}>Review repository <span aria-hidden="true">↗</span></button></form>{repositoryAudit && <div className="growth-repository-audit-result" role="status"><div className="growth-line"><strong>{repositoryAudit.fullName}</strong><Mark value={repositoryAudit.provenance} /></div><p>{repositoryAudit.description ?? "No public description reported."}</p><p className="growth-muted">Default branch: {repositoryAudit.defaultBranch ?? "Not reported"} · License: {repositoryAudit.license ?? "Not reported"}{repositoryAudit.archived ? " · archived" : ""}{repositoryAudit.fork ? " · fork" : ""}</p><p className="growth-muted">One metadata response · source files were not fetched · no workspace was created.</p><details><summary>Limits</summary><ul>{repositoryAudit.limitations.map((limitation) => <li key={limitation}>{limitation}</li>)}</ul></details><details className="growth-repository-import"><summary>Import this repository into a new local folder</summary><p className="growth-muted">The destination must not already exist. If the repository has no <code>growthlab.yaml</code>, provide the three brief fields below; the contract is committed only to your local clone.</p><form onSubmit={importRepository}><label htmlFor="growth-repository-path">New local folder</label><input id="growth-repository-path" value={repositoryImportPath} onChange={(event) => setRepositoryImportPath(event.target.value)} placeholder="./my-product" required /><label htmlFor="growth-repository-name">Product name (only for a checkout without growthlab.yaml)</label><input id="growth-repository-name" value={repositoryImportName} onChange={(event) => setRepositoryImportName(event.target.value)} maxLength={512} /><label htmlFor="growth-repository-audience">Audience</label><input id="growth-repository-audience" value={repositoryImportAudience} onChange={(event) => setRepositoryImportAudience(event.target.value)} maxLength={2048} /><label htmlFor="growth-repository-goal">Outcome to improve</label><input id="growth-repository-goal" value={repositoryImportGoal} onChange={(event) => setRepositoryImportGoal(event.target.value)} maxLength={4096} /><label htmlFor="growth-repository-mode">Permission mode for a new contract</label><select id="growth-repository-mode" value={repositoryImportMode} onChange={(event) => setRepositoryImportMode(event.target.value as typeof repositoryImportMode)}><option value="analyze_only">Analysis only</option><option value="draft">Draft</option></select><button className="growth-button" disabled={!!busy || !repositoryImportPath.trim()}>Clone and import locally <span aria-hidden="true">↗</span></button></form></details></div>}</section>
          <section className="growth-playbooks" aria-labelledby="growth-playbooks-title"><div className="growth-playbooks-intro"><span className="growth-section-number">05 / CHOOSE A GROWTH ANGLE</span><h2 id="growth-playbooks-title">Start with a focused playbook.</h2><p>Use one clear role at a time: ask better questions, collect evidence, and keep the next experiment within your product's boundaries.</p></div><div className="growth-playbook-grid">{(playbooks.data ?? []).map((playbook: GrowthPlaybook) => { const latest = playbookRuns.data?.find((run: PlaybookRun) => run.role === playbook.role); return <article className="growth-playbook-card" key={playbook.id}><div className="growth-line"><span className="growth-playbook-role">{roleLabel(playbook.role)}</span><span className="growth-playbook-template">{workspace ? "Executable" : "Template"}</span></div><h3>{playbook.title}</h3><p className="growth-playbook-focus">{playbook.focus}</p><p>{playbook.summary}</p><details><summary>Questions to answer</summary><ul>{playbook.questions.map((question) => <li key={question}>{question}</li>)}</ul></details><details><summary>Outputs and guardrails</summary><strong>Outputs</strong><ul>{playbook.outputs.map((output) => <li key={output}>{output}</li>)}</ul><strong>Guardrails</strong><ul>{playbook.guardrails.map((guardrail) => <li key={guardrail}>{guardrail}</li>)}</ul></details>{workspace && <button className="growth-button growth-button-secondary" disabled={!!busy} onClick={() => void runPlaybook(playbook.id)}>Run locally <span aria-hidden="true">→</span></button>}{latest && <details className="growth-playbook-latest"><summary>Latest local run · {latest.provenance}</summary><ul>{latest.responses.map((response) => <li key={response.question}><strong>{response.question}</strong><br />{response.answer}</li>)}</ul><p className="growth-muted">{latest.nextStep}</p></details>}<p className="growth-muted">{workspace ? "Saved locally as an UNTESTED role contract; no provider or outcome claim." : "A reusable template; no provider request or outcome claim."}</p></article>; })}</div></section>
          {!!workspaces.data?.length && <section className="growth-recent"><h2>Your product workspaces</h2>{workspaces.data.map((item) => <Link className="growth-recent-row" key={item.projectId} to="/growth/workspaces/$projectId" params={{ projectId: item.projectId }}><span>{item.config.product.name}<small>{item.config.product.audience}</small></span><span>{item.config.permissions.mode.replaceAll("_", " ")} <span aria-hidden="true">↗</span></span></Link>)}</section>}
          {!!battles.data?.length && <section className="growth-recent"><h2>Recent battles</h2>{battles.data.map((item) => <Link className="growth-recent-row" key={item.id} to="/growth/$battleId" params={{ battleId: item.id }}><span>{item.contract.goal}<small>{item.contract.config.product.name}</small></span><span>{item.status} <span aria-hidden="true">↗</span></span></Link>)}</section>}
        </> : !config ? <div className="growth-empty"><h1>{status.isPending ? "Opening battle…" : "Workspace unavailable"}</h1><p>Check the local API error or return to product workspaces.</p><Link to="/">Product workspaces</Link></div> : <>
          <div className="growth-workspace-heading"><div><h1>{battle ? battle.contract.goal : config.product.name}</h1><p>{config.product.audience} <span aria-hidden="true">/</span> {battle ? config.product.name : "Landing-page experimentation"}</p></div>{battle && <span className={`growth-status growth-status-${battle.status}`}>{battle.cancelRequested && running ? "Cancelling" : battle.status}</span>}</div>
          <details className="growth-contract"><summary>Permissions & frozen context <span>{config.permissions.mode.replaceAll("_", " ")} · {config.validation.commands.length} configured checks</span></summary><div className="growth-contract-grid"><div><h3>Allowed implementation paths</h3><p>{config.permissions.allowed_paths.join(", ") || "None"}</p><h3>Denied paths</h3><p>{config.permissions.denied_paths.join(", ") || "Built-in protected paths still apply"}</p></div><div><h3>Validation contract</h3>{config.validation.commands.map((command) => <code className="growth-command" key={command}>{command}</code>)}<p>{config.validation.timeout_seconds}s timeout · {config.agents.parallelism} competing variants in parallel</p></div><dl><Digest label="Baseline commit" value={battle?.contract.sourceSnapshotCommit ?? workspace?.sourceSnapshotCommit} /><Digest label="Contract SHA-256" value={battle?.contractDigest} /></dl></div></details>
          {!battle && workspace && <><section className="growth-experiment-map growth-panel" aria-labelledby="growth-experiment-map-title"><div className="growth-line"><div><span className="growth-section-number">NEXT / EXPERIMENT MAP</span><h2 id="growth-experiment-map-title">Keep the growth question visible.</h2></div>{!hypotheses.isPending && !(hypotheses.data?.length) && <button className="growth-button growth-button-secondary" disabled={!!busy} onClick={() => void createHypotheses()}>Create three hypotheses <span aria-hidden="true">→</span></button>}</div><p className="growth-muted">Each branch starts from the same committed product facts. The cards are untested until you run a battle and bring evidence.</p>{hypotheses.isPending ? <p className="growth-muted">Loading the experiment map…</p> : hypotheses.data?.length ? <HypothesisTree hypotheses={hypotheses.data} goal={workspace.config.goal.primary} audience={workspace.config.product.audience} onChoose={(hypothesis) => { setGoal(hypothesis.hypothesis); requestAnimationFrame(() => document.getElementById("growth-goal")?.scrollIntoView({ behavior: "smooth", block: "center" })); }} onSave={updateHypothesis} /> : <p className="growth-muted">No hypotheses yet. Create the starter map, then choose one branch for a focused battle.</p>}</section><section className="growth-goal"><span className="growth-section-number">NEXT / COMPOSE A BATTLE</span><h2>One goal. Three approaches.</h2><p>Create three hypothesis templates and isolated worktrees from your imported baseline. Templates use committed product facts; they are untested.</p><form onSubmit={prepare}><label htmlFor="growth-goal">What outcome matters now?</label><textarea id="growth-goal" value={goal} onChange={(event) => setGoal(event.target.value)} rows={2} maxLength={4096} required /><button className="growth-button" disabled={!!busy || !goal.trim() || config.permissions.mode !== "implementation"}>Prepare landing-page battle <span aria-hidden="true">→</span></button></form>{config.permissions.mode !== "implementation" && <p className="growth-muted">Worktree battles require implementation permission. Review and commit a suitable configuration, then import a new baseline. Analysis-only and draft playbooks remain in progress.</p>}</section><section className="growth-recent"><h2>This product's battles</h2>{battles.data?.filter((item) => item.projectId === workspace.projectId).map((item) => <Link className="growth-recent-row" key={item.id} to="/growth/$battleId" params={{ battleId: item.id }}><span>{item.contract.goal}<small>{new Date(item.createdAt).toLocaleString()}</small></span><span>{item.status} ↗</span></Link>)}{!battles.data?.some((item) => item.projectId === workspace.projectId) && <p className="growth-muted">Prepare your first battle to see the shared contract and competing variants.</p>}</section></>}
          {battle && <>
            <div className="growth-battle-actions"><p>{running ? "Inspect captured progress while the controller works." : "Review the evidence before selecting a candidate."}</p><div>{running && <button className="growth-button growth-button-secondary" disabled={!!busy} onClick={() => void perform("Requesting cancellation", () => growth.cancel(battle.id))}>Cancel battle</button>}{battle.status !== "ready" && !status.data?.controller?.running && <button className="growth-button growth-button-secondary" disabled={!!busy} onClick={() => void perform("Inspecting recovery", async () => {const result = await growth.recover(battle.id); if (mounted.current && activeView.current === viewKey) setNotice(`Recovery: ${result.status}. ${result.warnings.join(" ")}`); return result;})}>Inspect recovery</button>}</div></div>
            {status.data?.controller?.error && <p className="growth-error" role="alert">{status.data.controller.error}</p>}
            {battle.status === "ready" && !status.data?.controller?.running && <section className="growth-execution"><div><span className="growth-section-number">RUN / AUTHORIZE EXECUTION</span><h2>Choose the proposal source</h2><p>Every strategy follows this frozen contract. Changes stay in isolated worktrees; configured checks run under OS restrictions.</p>{capabilities.data?.isolationAvailable === false && <p className="growth-error">Validation isolation is unavailable on this host. Execution is refused.</p>}</div><form onSubmit={execute}><label htmlFor="growth-execution-mode">Execution mode</label><select id="growth-execution-mode" value={executionMode} onChange={(event) => setExecutionMode(event.target.value as typeof executionMode)}><option value="replay">Declared replay · SIMULATED proposals</option><option value="native">Native agent · provider authorization required</option></select>{executionMode === "replay" ? <><label htmlFor="growth-replay">Replay JSON plan (schema v1, three implementations)</label><textarea className="growth-json" id="growth-replay" value={replay} onChange={(event) => setReplay(event.target.value)} rows={5} spellCheck={false} required /><label className="growth-file-label">Load a local replay file<input type="file" accept=".json,application/json" onChange={(event) => {const file = event.target.files?.[0]; if (!file) return; if (file.size > 1024 * 1024) {setError("Replay file must be at most 1 MiB."); return;} void file.text().then((text) => {if (mounted.current && activeView.current === viewKey && isCurrentScope(scope)) setReplay(text);}).catch(() => {if (mounted.current && activeView.current === viewKey && isCurrentScope(scope)) setError("Could not read replay file.");});}} /></label><p className="growth-muted">The replay declares proposals only. File edits, commits, validation and archives execute in Rust. No outcome data is simulated into a measured result.</p></> : <><label htmlFor="growth-harness">Proposal harness</label><select id="growth-harness" value={harness} onChange={(event) => setHarness(event.target.value)} required><option value="">Select an adapter</option>{capabilities.data?.harnesses.filter((item) => item.toolsDisabledProposals).map((item) => <option key={item.id} value={item.id}>{item.id}</option>)}</select><label htmlFor="growth-model">Requested model (optional)</label><input id="growth-model" value={model} onChange={(event) => setModel(event.target.value)} maxLength={256} /><label className="growth-checkbox"><input type="checkbox" checked={providerConsent} onChange={(event) => setProviderConsent(event.target.checked)} />Authorize this harness to send the allowed product files, goal and frozen contract to its provider for tools-disabled proposals.</label><p className="growth-muted">Adapter capability does not verify installation or authentication. Native execution remains unverified in this alpha; unavailable CLIs produce honest failed attempts.</p></>}<button className="growth-button" disabled={!!busy || !capabilities.data?.isolationAvailable || (executionMode === "native" && (!harness || !providerConsent))}>Run three variants <span aria-hidden="true">→</span></button></form></section>}
            <section className="growth-competitors" aria-label="Competing variants">{variants.map((item, index) => {
              const hypothesis = portfolio.find((hypothesis) => hypothesis.id === item.hypothesisId)!;
              const captured = variantRun(status.data, item.id);
              const row = comparison.data?.rows.find((row) => row.variantId === item.id);
              return <article className={`growth-competitor ${variant?.id === item.id ? "is-focused" : ""}`} key={item.id}>
                <div className="growth-line"><span className="growth-variant-number">{String(index + 1).padStart(2, "0")}</span><span className={`growth-status ${row?.eligible ? "growth-status-completed" : ""}`}>{selected === item.id ? "Selected candidate" : row?.eligible ? "Eligible candidate" : runPhase(captured?.run)}</span></div>
                <button className="growth-variant-title" onClick={() => {setFocused(item.id); setTab("evidence");}} aria-pressed={variant?.id === item.id}><h2>{hypothesis.title}</h2><span aria-hidden="true">↗</span></button><p>{hypothesis.hypothesis}</p>{row?.hypothesisId && <p className="growth-muted growth-lineage">Hypothesis ID: <code>{row.hypothesisId}</code></p>}
                <div className="growth-check-fraction"><span>Configured checks</span><strong>{captured?.run.validations.length ? `${captured.run.validations.filter((check) => check.status === "done" && check.exitCode === 0).length} / ${config.validation.commands.length}` : "—"}</strong></div>
                <details className="growth-checks"><summary>Inspect checks & calculation</summary><p>{comparison.data?.calculation ?? "Passed configured commands / required commands. Eligibility additionally requires a successful sealed run, the exact frozen command list and one immutable candidate commit. This is not a growth score."}</p>{captured?.run.validations.map((check) => <div className="growth-check" key={check.runId}><code>{check.command}</code><div className="growth-line"><strong>{check.status} · exit {check.exitCode ?? "unavailable"}</strong><Mark value={check.provenance} /></div><p>{check.terminationReason}</p><p className="growth-muted">{check.limitation}</p><dl><Digest label="Source SHA-256" value={check.sourceDigest} /><Digest label="Isolation" value={check.confinement?.backend} /><Digest label="Policy SHA-256" value={check.confinement?.policyDigest} /></dl></div>)}{!captured?.run.validations.length && <p>No completed checks captured yet.</p>}{captured?.run.activeValidation && <p className="growth-notice">Command {captured.run.activeValidation.commandIndex + 1} is registered. Its captured log appears in the artifact viewer as the checkpoint advances.</p>}</details>
                {row?.rubric && <details className="growth-seo-rubric"><summary><span>{row.rubric.label}</span><strong>{row.rubric.score} / {row.rubric.maxScore} · {row.rubric.provenance}</strong></summary><p className="growth-muted">{row.rubric.calculation}</p><div className="growth-seo-dimensions">{row.rubric.dimensions.map((dimension) => <article className={`growth-seo-dimension growth-seo-dimension-${dimension.status}`} key={dimension.key}><div className="growth-line"><strong>{dimension.label}</strong><span>{dimension.score} / {dimension.maxScore}</span></div>{dimension.evidence.map((evidence) => <p key={evidence}>{evidence}</p>)}</article>)}</div><div className="growth-seo-recommendations"><strong>Suggested next steps</strong><ul>{row.rubric.recommendations.length ? row.rubric.recommendations.map((recommendation) => <li key={recommendation}>{recommendation}</li>) : <li>No structural gaps were found by this local rubric.</li>}</ul></div><ul className="growth-seo-limitations">{row.rubric.limitations.map((limitation) => <li key={limitation}>{limitation}</li>)}</ul></details>}
                {row?.quality && <QualityRubricDetails rubric={row.quality} />}
                {row?.accessibility && <QualityRubricDetails rubric={row.accessibility} />}
                {row?.render && <QualityRubricDetails rubric={row.render} />}
                {row?.performance && <QualityRubricDetails rubric={row.performance} />}
                {captured?.run.error && <p className="growth-check-error">{captured.run.error}</p>}
                <div className="growth-provenance"><div><span>Proposal</span><Mark value={captured?.run.provenance ?? "UNTESTED"} /></div><div><span>Growth outcome</span><Mark value={captured?.run.outcomeProvenance ?? "UNTESTED"} /></div><div><span>Confidence</span><strong>{row?.confidence.label ?? captured?.run.confidence.label ?? hypothesis.confidence.label}</strong></div></div>
                <p className="growth-muted growth-confidence">{row?.confidence.rationale ?? hypothesis.confidence.rationale}</p><button className="growth-text-button" onClick={() => {setFocused(item.id); setTab("artifacts");}}>Inspect diff & captured logs →</button>
              </article>;
            })}</section>
            {variant && hypothesis && <section className="growth-inspector"><div className="growth-inspector-top"><div><span className="growth-eyebrow">Variant inspection</span><h2>{hypothesis.title}</h2></div><span className="growth-muted">{capture?.sealed ? "Sealed archive" : capture?.digest ? "Live checkpoint" : "Prepared hypothesis"}</span></div><div className="growth-tab-bar" role="tablist" aria-label="Variant inspection"><button className="min-h-11" ref={evidenceTab} onKeyDown={tabKey} tabIndex={tab === "evidence" ? 0 : -1} role="tab" aria-selected={tab === "evidence"} aria-controls="growth-evidence-panel" id="growth-evidence-tab" onClick={() => setTab("evidence")}>Hypothesis & evidence</button><button className="min-h-11" ref={artifactsTab} onKeyDown={tabKey} tabIndex={tab === "artifacts" ? 0 : -1} role="tab" aria-selected={tab === "artifacts"} aria-controls="growth-artifact-panel" id="growth-artifact-tab" onClick={() => setTab("artifacts")}>Diff, files & logs</button><button className="min-h-11" ref={previewTab} onKeyDown={tabKey} tabIndex={tab === "preview" ? 0 : -1} role="tab" aria-selected={tab === "preview"} aria-controls="growth-preview-panel" id="growth-preview-tab" onClick={() => setTab("preview")}>Static preview</button></div>
              {tab === "evidence" ? <div role="tabpanel" id="growth-evidence-panel" aria-labelledby="growth-evidence-tab"><HypothesisEvidence hypothesis={hypothesis} /></div> : tab === "preview" ? <div role="tabpanel" id="growth-preview-panel" aria-labelledby="growth-preview-tab"><StaticPreviewPanel key={variant.id} variantId={variant.id} record={capture?.run.staticPreview} digest={capture?.digest ?? null} configured={!!config?.static_preview} trusted={evidenceTrusted} /></div> : <div role="tabpanel" id="growth-artifact-panel" aria-labelledby="growth-artifact-tab">{!capture?.digest ? <p className="growth-muted">Artifacts appear after the first recorded checkpoint. No execution has been fabricated.</p> : artifacts.isPending ? <Spinner /> : artifacts.error ? <p role="alert">{artifacts.error.message}</p> : !artifacts.data?.artifacts.length ? <p className="growth-muted">This checkpoint has no viewer artifacts yet.</p> : <><label htmlFor="growth-artifact-name">Captured artifact</label><select id="growth-artifact-name" value={activeArtifact?.name ?? ""} onChange={(event) => setArtifactName(event.target.value)}>{artifacts.data.artifacts.map((item) => <option key={item.name} value={item.name}>{item.name} · {item.size.toLocaleString()} bytes</option>)}</select>{artifact.isPending ? <Spinner /> : artifact.error ? <p role="alert">{artifact.error.message}</p> : <><pre className="growth-artifact">{artifact.data?.text}</pre><dl><Digest label="Artifact SHA-256" value={artifact.data?.digest} /><Digest label={artifact.data?.sealed ? "Archive SHA-256" : "Checkpoint SHA-256"} value={artifact.data?.archiveDigest} /></dl></>}</>}</div>}
              <details className="growth-local-metadata"><summary>Local reproducibility metadata</summary><dl><Digest label="Variant ID" value={variant.id} /><Digest label="Branch" value={variant.branchName} /><Digest label="Local worktree" value={variant.worktree} /><Digest label="Candidate commit" value={capture?.run.candidateCommit} /><Digest label="Agent harness" value={capture?.run.agent.harness} /><Digest label="Requested model" value={capture?.run.agent.requestedModel} /><Digest label="Cost / tokens" value={capture?.run.agent.cost == null && capture?.run.agent.tokens == null ? "Not supplied by the adapter" : `${capture?.run.agent.cost ?? "Unknown"} / ${capture?.run.agent.tokens ?? "Unknown"}`} /></dl></details>
              <div className="growth-delivery"><div><span className="growth-eyebrow">Your decision</span><h3>{eligible ? selected === variant.id ? "Ready for explicit delivery" : "Eligible for user review" : "No eligible sealed candidate yet"}</h3><p>Selection is a review decision. Passing commands does not establish conversion lift.</p></div><button className="growth-button" disabled={!!busy || !eligible || selected === variant.id} onClick={() => void perform("Recording selected candidate", () => growth.select(variant.id))}>{selected === variant.id ? "Candidate selected" : "Select candidate"}</button></div>
              {selected === variant.id && <div className="growth-delivery-controls"><form onSubmit={(event) => {event.preventDefault(); void perform("Exporting selected patch", async () => {const result = await growth.export(variant.id, exportPath.trim()); if (mounted.current && activeView.current === viewKey) setNotice("Selected patch exported. The product working tree was preserved."); return result;});}}><label htmlFor="growth-export">New patch path outside product/worktrees</label><input id="growth-export" value={exportPath} onChange={(event) => setExportPath(event.target.value)} placeholder="/path/to/new-selected.patch" required /><button className="growth-button growth-button-secondary" disabled={!!busy || !evidenceTrusted || !exportPath.trim()}>Export patch</button></form><div><p>Preview the exact change before modifying your product working tree.</p><button className="growth-button growth-button-secondary" disabled={!!busy || !evidenceTrusted} onClick={() => void perform("Checking selected apply", async () => {const result = await growth.preview(variant.id); if (mounted.current && activeView.current === viewKey) {setPreview(result); setApplyConsent(false);} return result;})}>Review selected apply</button></div></div>}
              {preview && <section className="growth-apply-review" aria-label="Apply review"><h3>Apply this selected patch?</h3><p>{preview.operation}</p><ul>{preview.changedFiles.map((path) => <li key={path}><code>{path}</code></li>)}</ul><dl><Digest label="Product baseline" value={preview.sourceCommit} /><Digest label="Candidate commit" value={preview.candidateCommit} /><Digest label="Patch SHA-256" value={preview.patchDigest} /></dl><label className="growth-checkbox"><input type="checkbox" checked={applyConsent} onChange={(event) => setApplyConsent(event.target.checked)} />Apply only this selected variant to my product working tree. I will review and commit the changes myself.</label><div className="growth-line"><button className="growth-button" disabled={!!busy || !evidenceTrusted || !applyConsent} onClick={() => void perform("Applying selected working-tree change", async () => {const result = await growth.apply(preview.variantId); if (mounted.current && activeView.current === viewKey) {setPreview(null); setNotice("Selected changes applied to the working tree. Product HEAD, index and remotes were preserved.");} return result;})}>Apply selected changes</button><button className="growth-text-button" disabled={!!busy} onClick={() => setPreview(null)}>Close preview</button></div></section>}
            </section>}
            {!!status.data?.runs.length && <section className="growth-share"><div><span className="growth-eyebrow">Take the evidence with you</span><h2>A self-contained battle report.</h2><p>Includes checks, digests and provenance. Private product names, paths, prompts and raw logs are withheld by default.</p></div><div><button className="growth-button growth-button-secondary" disabled={!!busy} onClick={() => void downloadReport("html")}>Download HTML</button><button className="growth-button growth-button-secondary" disabled={!!busy} onClick={() => void downloadReport("markdown")}>Download Markdown</button></div></section>}
          </>}
        </>}
        <footer className="growth-footer"><span>Built with GrowthLab · local-first, open source</span><span>Command checks ≠ measured growth. Runtime files are trusted inputs; resource quotas are not provided.</span></footer>
      </main>
    </div>
  </div>;
}
