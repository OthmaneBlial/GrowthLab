import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { workspaceKey } from "../queries/client";
import { Spinner } from "../components/ui";
import { m } from "../paraglide/messages.js";
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
  const screenshot = record?.screenshots?.find((item) => item.path.endsWith(`screenshot-${viewport}.png`));
  const renderCheck = record?.renderChecks?.find((item) => item.viewport === viewport);
  useEffect(() => {
    if (!ready || !stage.current) return;
    const observer = new ResizeObserver(([entry]) => setAvailableWidth(entry.contentRect.width));
    observer.observe(stage.current);
    return () => observer.disconnect();
  }, [ready]);
  if (!trusted) return <p role="alert">{m.growth_static_preview_evidence_failed()}</p>;
  if (!record) return <p className="growth-muted">{configured
    ? m.growth_static_preview_no_archive()
    : m.growth_static_preview_not_configured()}</p>;
  if (record.status === "unavailable") return <p className="growth-muted">{record.limitation}</p>;
  if (preview.error) return <p role="alert">{preview.error.message}</p>;
  if (preview.isPending || !ready) return <Spinner />;
  const size = viewports[viewport];
  const scale = Math.min(1, availableWidth / size.width);
  return <div className="growth-static-preview">
    <div className="growth-preview-controls">
      <div role="group" aria-label={m.growth_static_preview_viewport_group()}>
        <button className="growth-button growth-button-secondary min-h-11" aria-pressed={viewport === "desktop"} onClick={() => setViewport("desktop")}>{m.growth_static_preview_desktop()}</button>
        <button className="growth-button growth-button-secondary min-h-11" aria-pressed={viewport === "phone"} onClick={() => setViewport("phone")}>{m.growth_static_preview_phone()}</button>
      </div>
      <span className="growth-muted">{data.sealed ? m.growth_static_preview_sealed_source() : m.growth_static_preview_checkpoint_source()}</span>
    </div>
    <p className="growth-preview-note">{screenshot
      ? m.growth_static_preview_note_captured({ viewport: viewport === "desktop" ? "desktop" : "phone" })
      : m.growth_static_preview_note_missing({ viewport: viewport === "desktop" ? "desktop" : "phone" })}</p>
    <div ref={stage} className="growth-preview-stage" style={{ height: Math.max(1, size.height * scale) }}>
      <iframe title={m.growth_static_preview_iframe_title()} srcDoc={data.html} sandbox="" inert tabIndex={-1} referrerPolicy="no-referrer"
        style={{ width: size.width, height: size.height, transform: `translateX(-50%) scale(${scale})` }} />
    </div>
    {screenshot && <figure className="growth-preview-capture">
      <img src={`/api/growth/variants/${encodeURIComponent(variantId)}/static-preview/${viewport}`} alt={m.growth_static_preview_alt({ viewport })} loading="lazy" />
      <figcaption>{m.growth_static_preview_captured_png({ width: String(screenshot.width), height: String(screenshot.height), digest: screenshot.digest })}</figcaption>
    </figure>}
    {renderCheck && <details className="growth-local-metadata">
      <summary>{m.growth_static_preview_observed_render_check({ provenance: renderCheck.provenance })}</summary>
      <dl><div className="growth-digest"><dt>{m.growth_static_preview_viewport()}</dt><dd>{renderCheck.width} × {renderCheck.height} · {renderCheck.viewportMatches ? m.growth_static_preview_matched() : m.growth_static_preview_mismatched()}</dd></div>
        <div className="growth-digest"><dt>{m.growth_static_preview_horizontal_overflow()}</dt><dd>{renderCheck.horizontalOverflow ? m.growth_static_preview_detected() : m.growth_static_preview_none_detected()}</dd></div>
        <div className="growth-digest"><dt>{m.growth_static_preview_visible_copy()}</dt><dd>{m.growth_static_preview_characters({ count: renderCheck.bodyTextChars.toLocaleString() })}</dd></div>
        <div className="growth-digest"><dt>{m.growth_static_preview_dom_load()}</dt><dd>{renderCheck.domContentLoadedMs ?? "—"} ms / {renderCheck.loadMs ?? "—"} ms</dd></div>
        <div className="growth-digest"><dt>{m.growth_static_preview_first_contentful_paint()}</dt><dd>{renderCheck.firstContentfulPaintMs == null ? "—" : `${renderCheck.firstContentfulPaintMs} ms`}</dd></div>
      </dl>
      <p>{renderCheck.limitation}</p>
    </details>}
    <details className="growth-local-metadata"><summary>{m.growth_static_preview_source_limitations()}</summary>
      <p>{record.limitation}</p><p>{m.growth_static_preview_source_summary({ files: String(record.sources.length), resources: String(record.blockedResources), producer: record.producer })}</p>
      <dl><div className="growth-digest"><dt>{m.growth_static_preview_candidate_commit()}</dt><dd>{record.sourceCommit}</dd></div>
        <div className="growth-digest"><dt>{m.growth_static_preview_document_sha()}</dt><dd>{record.documentDigest}</dd></div>
        <div className="growth-digest"><dt>{data.sealed ? m.growth_static_preview_archive_sha() : m.growth_static_preview_checkpoint_sha()}</dt><dd>{data.archiveDigest}</dd></div></dl>
    </details>
  </div>;
}
