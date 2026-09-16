import type { BattleStatus, Run } from "./api";

export type InspectionTab = "evidence" | "artifacts" | "preview";
const inspectionTabs: InspectionTab[] = ["evidence", "artifacts", "preview"];
export function inspectionTab(current: InspectionTab, key: string): InspectionTab | null {
  if (key === "Home") return inspectionTabs[0];
  if (key === "End") return inspectionTabs.at(-1)!;
  const direction = key === "ArrowLeft" ? -1 : key === "ArrowRight" ? 1 : 0;
  return direction ? inspectionTabs[(inspectionTabs.indexOf(current) + direction + inspectionTabs.length) % inspectionTabs.length] : null;
}

export function validGrowthId(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value);
}

/** Live checkpoints are inspection state; only terminal seals supply eligibility. */
export function variantRun(status: BattleStatus | undefined, variantId: string): { run: Run; sealed: boolean; digest: string | null } | null {
  const seal = status?.runs.find((item) => item.run.variantId === variantId);
  if (seal) return { run: seal.run, sealed: true, digest: seal.archiveDigest };
  const attempt = status?.attempts.find((item) => item.run.variantId === variantId);
  return attempt ? { run: attempt.run, sealed: false, digest: attempt.checkpointDigest } : null;
}

export function selectedVariant(status: BattleStatus | undefined): string | null {
  return status?.selections.filter((item) => item.action === "select" && item.status === "done").at(-1)?.variantId ?? null;
}

export function runPhase(run: Run | undefined): string {
  if (!run) return "Prepared";
  if (run.status !== "running") return run.status.charAt(0).toUpperCase() + run.status.slice(1);
  if (run.activeValidation) return `Checking command ${run.activeValidation.commandIndex + 1}`;
  return run.candidateCommit ? "Capturing validation" : "Preparing proposal";
}
