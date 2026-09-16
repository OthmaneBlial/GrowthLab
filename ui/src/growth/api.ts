import { isCurrentScope, workspaceScope } from "../queries/client";

export type Provenance = "MEASURED" | "OBSERVED" | "ESTIMATED" | "SIMULATED" | "UNTESTED";
export interface Confidence { label: "low" | "medium" | "high"; rationale: string }
export interface GrowthConfig {
  version: number;
  product: { name: string; audience: string; description: string };
  goal: { primary: string };
  permissions: { mode: "analyze_only" | "draft" | "implementation"; allowed_paths: string[]; denied_paths: string[] };
  validation: { commands: string[]; timeout_seconds: number };
  metrics: { primary: string; guardrails: string[] };
  agents: { parallelism: number };
  static_preview?: { root: string; entry: string };
}
export interface Workspace { projectId: string; config: GrowthConfig; sourceSnapshotCommit: string; createdAt: number }
export interface Evidence {
  id: string; title: string; source: string; retrievedAt: number; observation: string;
  publisher: string | null; evidenceType: string; supportsClaims: string[]; challengesClaims: string[];
  provenance: Provenance; confidence: Confidence; limitations: string;
}
export interface Hypothesis {
  id: string; title: string; goal: string; audience: string; channel: string; role: string;
  hypothesis: string; mechanism: string; baselineDefinition: string; primaryMetric: string;
  guardrailMetrics: string[]; successThreshold: string | null; evidence: Evidence[];
  risks: string[]; provenance: Provenance; confidence: Confidence;
}
export interface Battle {
  id: string; projectId: string; status: "ready" | "running" | "completed" | "failed" | "cancelled";
  createdAt: number; endedAt: number | null; cancelRequested: boolean; contractDigest: string;
  contract: {
    version: number; goal: string; config: GrowthConfig; sourceSnapshotCommit: string; sourceSnapshotDigest: string;
    hypotheses: Hypothesis[]; evaluation: string; limitations: string[];
  };
}
export interface Variant { id: string; battleId: string; hypothesisId: string; branchName: string; worktree: string }
export interface Check {
  runId: string; command: string; sourceCommit: string; sourceDigest: string; status: string;
  exitCode: number | null; terminationReason: string | null; logTruncated: boolean;
  startedAt: number; endedAt: number; provenance: Provenance; limitation: string;
  confinement?: { backend: string; policyDigest: string; network: string; writable: string; limitation: string };
}
export interface Run {
  id: string; variantId: string; battleId: string; candidateCommit: string | null; status: string; error: string | null;
  provenance: Provenance; outcomeProvenance: Provenance; confidence: Confidence;
  startedAt: number; endedAt: number;
  implementation: { summary: string; files: { path: string; contents: string | null }[]; risks: string[] } | null;
  validations: Check[]; activeValidation?: { runId: string; commandIndex: number; confinement?: Check["confinement"] };
  agent: { harness: string; requestedModel: string | null; mode: string; cost: number | null; tokens: number | null };
  staticPreview?: StaticPreviewRecord;
}
export interface StaticPreviewRecord {
  producer: string; status: "ready" | "unavailable"; sourceCommit: string;
  documentDigest: string | null; sources: { path: string; digest: string; size: number }[];
  screenshots?: { path: string; width: number; height: number; digest: string; size: number }[];
  renderChecks?: { viewport: "desktop" | "phone"; width: number; height: number; viewportMatches: boolean;
    bodyTextChars: number; horizontalOverflow: boolean; domContentLoadedMs: number | null;
    loadMs: number | null; firstContentfulPaintMs: number | null; provenance: Provenance; limitation: string }[];
  blockedResources: number; limitation: string;
}
export interface StaticPreviewData {
  html: string; record: StaticPreviewRecord; archiveDigest: string; sealed: boolean;
}
export interface Selection { id: string; variantId: string; action: "select" | "apply" | "export"; status: "pending" | "done" | "failed"; error: string | null }
export interface BattleStatus {
  battle: Battle; variants: Variant[];
  runs: { run: Run; archiveDigest: string }[];
  attempts: { run: Run; checkpointDigest: string | null }[];
  selections: Selection[];
  controller: { running: boolean; error: string | null } | null;
}
export interface RubricDimension { key: string; label: string; score: number; maxScore: number; status: "strong" | "partial" | "missing"; evidence: string[] }
export interface SeoRubric { id: string; label: string; score: number; maxScore: number; provenance: Provenance; dimensions: RubricDimension[]; recommendations: string[]; calculation: string; limitations: string[] }
export interface PageQualityRubric { id: string; label: string; score: number; maxScore: number; provenance: Provenance; dimensions: RubricDimension[]; recommendations: string[]; calculation: string; limitations: string[] }
export interface RenderRubric { id: string; label: string; score: number; maxScore: number; provenance: Provenance; dimensions: RubricDimension[]; recommendations: string[]; calculation: string; limitations: string[] }
export type PerformanceRubric = RenderRubric;
export interface RemoteSeoAudit {
  url: string; retrievedAt: number; httpStatus: number; contentType: string | null;
  robots: { url: string; status: number; allowed: boolean; matchedRule: string | null; retrievedAt: number; limitation: string };
  scope: string; provenance: Provenance; rubric: SeoRubric; quality?: PageQualityRubric; limitations: string[];
}
export interface PublicRepositoryAudit {
  url: string; apiUrl: string; owner: string; repository: string; fullName: string; htmlUrl: string;
  description: string | null; defaultBranch: string | null; license: string | null;
  archived: boolean; fork: boolean; retrievedAt: number; scope: string; provenance: Provenance; limitations: string[];
}
export interface Comparison {
  battleId: string; label: string; evaluator: string; calculation: string; limitations: string[];
  recommendedCandidates: string[];
  rows: { variantId: string; hypothesisId?: string; title: string; status: string; checks: Check[];
    passedCommands: number; requiredCommands: number; eligible: boolean; archiveDigest: string | null;
    implementationProvenance: Provenance; checkProvenance: Provenance; outcomeProvenance: Provenance; confidence: Confidence; rubric?: SeoRubric; quality?: PageQualityRubric; render?: RenderRubric; performance?: PerformanceRubric }[];
}
export interface ArtifactList {
  runId: string; archiveDigest: string; sealed: boolean;
  artifacts: { name: string; size: number; digest: string }[];
}
export interface Artifact { name: string; text: string; digest: string; archiveDigest: string; sealed: boolean }
export interface ApplyPreview { variantId: string; sourceCommit: string; candidateCommit: string; patchDigest: string; changedFiles: string[]; operation: string }
export interface Capabilities { isolationAvailable: boolean; platform: string; harnesses: { id: string; toolsDisabledProposals: boolean }[]; limitation: string }
export interface GrowthPlaybook {
  id: string; title: string; role: string; focus: string; summary: string;
  questions: string[]; outputs: string[]; guardrails: string[];
}
export interface PlaybookRun {
  id: string; projectId: string; role: string; title: string;
  responses: { question: string; answer: string }[]; outputs: string[];
  guardrails: string[]; nextStep: string; provenance: Provenance;
  confidence: Confidence; createdAt: number;
}
export type Execution = { mode: "replay"; plan: unknown } | { mode: "native"; harness: string; model: string | null; agentTimeoutSeconds: number };

const prefix = "/api/growth";
const id = encodeURIComponent;
async function response(path: string, method = "GET", body?: unknown, signal?: AbortSignal) {
  const scope = workspaceScope();
  const result = await fetch(prefix + path, {
    method, signal, headers: body === undefined ? {} : { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!isCurrentScope(scope)) throw new DOMException("Workspace changed", "AbortError");
  if (!result.ok) {
    const text = await result.text();
    let message = `HTTP ${result.status}`;
    try { const data: unknown = JSON.parse(text); if (data && typeof data === "object" && "error" in data && typeof data.error === "string") message = data.error; } catch { /* JSON schema rejections have a plain-text body. */ }
    throw new Error(message);
  }
  return result;
}
async function request<T>(path: string, method = "GET", body?: unknown, signal?: AbortSignal): Promise<T> {
  const scope = workspaceScope();
  const result: T = await (await response(path, method, body, signal)).json();
  if (!isCurrentScope(scope)) throw new DOMException("Workspace changed", "AbortError");
  return result;
}
export const growth = {
  demo: () => request<{projectId: string; battleId: string; accepted: boolean}>("/demo", "POST", {}),
  workspaces: (signal?: AbortSignal) => request<Workspace[]>("/workspaces", "GET", undefined, signal),
  import: (path: string, initializeGit = false) => request<Workspace>("/workspaces", "POST", { path, initializeGit }),
  capabilities: (signal?: AbortSignal) => request<Capabilities>("/capabilities", "GET", undefined, signal),
  playbooks: (signal?: AbortSignal) => request<GrowthPlaybook[]>("/playbooks", "GET", undefined, signal),
  playbookRuns: (projectId: string, signal?: AbortSignal) => request<PlaybookRun[]>(`/workspaces/${id(projectId)}/playbooks`, "GET", undefined, signal),
  runPlaybook: (projectId: string, role: string, answers: string[] = []) => request<PlaybookRun>(`/workspaces/${id(projectId)}/playbooks/${id(role)}`, "POST", { answers }),
  hypotheses: (projectId: string, signal?: AbortSignal) => request<Hypothesis[]>(`/workspaces/${id(projectId)}/hypotheses`, "GET", undefined, signal),
  createHypotheses: (projectId: string) => request<Hypothesis[]>(`/workspaces/${id(projectId)}/hypotheses`, "POST"),
  urlAudit: (url: string) => request<RemoteSeoAudit>("/url-audit", "POST", { url }),
  repositoryAudit: (url: string) => request<PublicRepositoryAudit>("/repository-audit", "POST", { url }),
  repositoryImport: (input: { url: string; path: string; name?: string; audience?: string; goal?: string; description?: string; metric?: string; mode?: "analyze_only" | "draft" | "implementation"; allowedPaths?: string[]; deniedPaths?: string[]; commands?: string[]; shallow?: boolean }) => request<Workspace>("/repository-import", "POST", input),
  createBrief: (brief: { name: string; audience: string; goal: string; description: string; metric: string }) => request<Workspace>("/briefs", "POST", brief),
  battles: (signal?: AbortSignal) => request<Battle[]>("/battles", "GET", undefined, signal),
  prepare: (projectId: string, goal: string) => request<{battle: Battle; variants: Variant[]}>("/battles", "POST", { projectId, goal }),
  status: (battleId: string, signal?: AbortSignal) => request<BattleStatus>(`/battles/${id(battleId)}`, "GET", undefined, signal),
  run: (battleId: string, execution: Execution) => request<{accepted: boolean}>(`/battles/${id(battleId)}/run`, "POST", execution),
  cancel: (battleId: string) => request(`/battles/${id(battleId)}/cancel`, "POST"),
  recover: (battleId: string) => request<{status: string; warnings: string[]}>(`/battles/${id(battleId)}/recover`, "POST"),
  compare: (battleId: string, signal?: AbortSignal) => request<Comparison>(`/battles/${id(battleId)}/compare`, "GET", undefined, signal),
  artifacts: (variantId: string, signal?: AbortSignal) => request<ArtifactList>(`/variants/${id(variantId)}/artifacts`, "GET", undefined, signal),
  artifact: (variantId: string, name: string, signal?: AbortSignal) => request<Artifact>(`/variants/${id(variantId)}/artifact?name=${id(name)}`, "GET", undefined, signal),
  staticPreview: (variantId: string, signal?: AbortSignal) => request<StaticPreviewData>(`/variants/${id(variantId)}/static-preview`, "GET", undefined, signal),
  select: (variantId: string) => request<Selection>(`/variants/${id(variantId)}/select`, "POST"),
  preview: (variantId: string) => request<ApplyPreview>(`/variants/${id(variantId)}/apply-preview`),
  apply: (variantId: string) => request<Selection>(`/variants/${id(variantId)}/apply`, "POST", { confirmed: true }),
  export: (variantId: string, path: string) => request<Selection>(`/variants/${id(variantId)}/export`, "POST", { path }),
  report: async (battleId: string, format: "html" | "markdown") => {
    const scope = workspaceScope();
    const blob = await (await response(`/battles/${id(battleId)}/report?format=${format}`)).blob();
    if (!isCurrentScope(scope)) throw new DOMException("Workspace changed", "AbortError");
    return blob;
  },
};
