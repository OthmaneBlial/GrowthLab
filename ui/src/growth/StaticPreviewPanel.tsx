import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { workspaceKey } from "../queries/client";
import { Spinner } from "../components/ui";
import { growth, type StaticPreviewRecord } from "./api";

const viewports = { desktop: { width: 1280, height: 900 }, phone: { width: 390, height: 844 } };

export function StaticPreviewPanel({ variantId, record, digest, configured, trusted }: {
  variantId: string; record?: StaticPreviewRecord; digest: string | null;
  configured: boolean; trusted: boolean;
}) {
  const [viewport, setViewport] = useState<keyof typeof viewports>(() => window.matchMedia("(max-width:520px)").matches ? "phone" : "desktop");
  const stage = useRef<HTMLDivElement>(null);
  const [availableWidth, setAvailableWidth] = useState(0);
  const preview = useQuery({
    queryKey: workspaceKey("growth", "static-preview", variantId, digest),
    queryFn: ({ signal }) => growth.staticPreview(variantId, signal),
    enabled: trusted && !!digest && record?.status === "ready",
  });
  const data = preview.data;
  const ready = trusted && data?.archiveDigest === digest && data.record.documentDigest === record?.documentDigest;
  useEffect(() => {
    if (!ready || !stage.current) return;
    const observer = new ResizeObserver(([entry]) => setAvailableWidth(entry.contentRect.width));
    observer.observe(stage.current);
    return () => observer.disconnect();
  }, [ready]);
  if (!trusted) return <p role="alert">Evidence verification failed. The preview is unavailable until the archive can be verified.</p>;
  if (!record) return <p className="growth-muted">{configured
    ? "This attempt has no archived static preview yet. Earlier run archives remain unchanged."
    : "No static preview was configured for this battle. Add static_preview to growthlab.yaml before preparing a new battle."}</p>;
  if (record.status === "unavailable") return <p className="growth-muted">{record.limitation}</p>;
  if (preview.error) return <p role="alert">{preview.error.message}</p>;
  if (preview.isPending || !ready) return <Spinner />;
  const size = viewports[viewport];
  const scale = Math.min(1, availableWidth / size.width);
  return <div className="growth-static-preview">
    <div className="growth-preview-controls">
      <div role="group" aria-label="Static preview viewport">
        <button className="growth-button growth-button-secondary min-h-11" aria-pressed={viewport === "desktop"} onClick={() => setViewport("desktop")}>Desktop · 1280 × 900</button>
        <button className="growth-button growth-button-secondary min-h-11" aria-pressed={viewport === "phone"} onClick={() => setViewport("phone")}>Phone · 390 × 844</button>
      </div>
      <span className="growth-muted">{data.sealed ? "Sealed static source" : "Verified source checkpoint"}</span>
    </div>
    <p className="growth-preview-note">Live browser preview of archived static source. One viewport; no saved screenshot or visual quality score yet. Scripts, forms and navigation are disabled.</p>
    <div ref={stage} className="growth-preview-stage" style={{ height: Math.max(1, size.height * scale) }}>
      <iframe title="Archived candidate static page" srcDoc={data.html} sandbox="" inert tabIndex={-1} referrerPolicy="no-referrer"
        style={{ width: size.width, height: size.height, transform: `translateX(-50%) scale(${scale})` }} />
    </div>
    <details className="growth-local-metadata"><summary>Preview source and limitations</summary>
      <p>{record.limitation}</p><p>{record.sources.length} archived source files · {record.blockedResources} omitted resource references · {record.producer}</p>
      <dl><div className="growth-digest"><dt>Candidate commit</dt><dd>{record.sourceCommit}</dd></div>
        <div className="growth-digest"><dt>Preview document SHA-256</dt><dd>{record.documentDigest}</dd></div>
        <div className="growth-digest"><dt>{data.sealed ? "Archive SHA-256" : "Checkpoint SHA-256"}</dt><dd>{data.archiveDigest}</dd></div></dl>
    </details>
  </div>;
}
