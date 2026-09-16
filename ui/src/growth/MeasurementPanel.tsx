import { useState, type FormEvent } from "react";
import { useQuery } from "@tanstack/react-query";
import { workspaceKey } from "../queries/client";
import { growth } from "./api";
import { parseMeasurementCsv, type LocalMeasurementReport } from "./measurement";

function number(value: number) {
  return new Intl.NumberFormat(undefined, { maximumFractionDigits: 4 }).format(value);
}

function interval(value: { lower: number; upper: number } | null) {
  return value ? `[${number(value.lower)}, ${number(value.upper)}]` : "—";
}

export function MeasurementPanel() {
  const [csv, setCsv] = useState("");
  const [fileName, setFileName] = useState("");
  const [baseline, setBaseline] = useState("baseline");
  const [report, setReport] = useState<LocalMeasurementReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [reading, setReading] = useState(false);
  const sources = useQuery({ queryKey: workspaceKey("growth", "measurement-sources"), queryFn: ({ signal }) => growth.measurementSources(signal) });

  async function readFile(file: File | undefined) {
    if (!file) return;
    if (file.size > 8 * 1024 * 1024) { setError("Measurement CSV must be at most 8 MiB."); setCsv(""); setFileName(""); return; }
    setReading(true); setError(null); setReport(null);
    try { setCsv(await file.text()); setFileName(file.name); }
    catch { setError("Could not read the local CSV file."); setCsv(""); setFileName(""); }
    finally { setReading(false); }
  }

  function summarize(event: FormEvent) {
    event.preventDefault();
    if (!csv) return;
    try { setReport(parseMeasurementCsv(csv, baseline)); setError(null); }
    catch (cause) { setReport(null); setError(cause instanceof Error ? cause.message : String(cause)); }
  }

  return <section className="growth-measurement growth-panel" aria-labelledby="growth-measurement-title">
    <span className="growth-section-number">03 / MEASURE WHAT YOU OWN</span>
    <h2 id="growth-measurement-title">Bring a local outcome export.</h2>
    <p>Read your own CSV in this browser. GrowthLab compares variants within each metric and optional distribution or channel, locally, and sends no rows to a provider.</p>
    <details className="growth-measurement-analysis" open>
      <summary>Measurement source boundary</summary>
      {sources.isPending ? <p className="growth-muted">Loading source availability…</p> : sources.error ? <p className="growth-muted">Source availability could not be loaded; local CSV remains available in this browser.</p> : <ul>{sources.data?.map((source) => <li key={source.id}><strong>{source.label}</strong> · {source.status === "available" ? "available locally" : "planned"} · {source.network === "none" ? "no network" : "explicit opt-in network"}<br /><span className="growth-muted">{source.limitation}</span></li>)}</ul>}
    </details>
    <form onSubmit={summarize}>
      <label className="growth-file-label" htmlFor="growth-measurement-file">Telemetry CSV<input id="growth-measurement-file" type="file" accept=".csv,text/csv" onChange={(event) => { void readFile(event.target.files?.[0]); }} /></label>
      <label htmlFor="growth-measurement-baseline">Baseline variant</label>
      <input id="growth-measurement-baseline" value={baseline} onChange={(event) => setBaseline(event.target.value)} maxLength={256} required />
      <button className="growth-button" type="submit" disabled={reading || !csv}>{reading ? "Reading…" : "Summarize locally"} <span aria-hidden="true">→</span></button>
    </form>
    {fileName && <p className="growth-measurement-file">Loaded locally: <code>{fileName}</code></p>}
    {error && <p className="growth-error" role="alert">{error}</p>}
    {report && <div className="growth-measurement-result" aria-live="polite">
      <div className="growth-line"><strong>Descriptive summary</strong><span className="growth-mark growth-mark-observed">{report.provenance}</span></div>
      <p className="growth-muted">{report.rowsIncluded} observations · baseline <code>{report.baselineVariant}</code>{report.dateRange ? ` · ${report.dateRange.from} → ${report.dateRange.to}` : " · date range unavailable"}</p>
      <div className="growth-measurement-table-wrap"><table className="growth-measurement-table"><thead><tr><th>Metric</th><th>Distribution</th><th>Variant</th><th>Mean</th><th>n</th><th>Mean 95%</th><th>Change</th><th>Δ 95%</th></tr></thead><tbody>{report.groups.map((group) => <tr key={`${group.metric}-${group.distribution}-${group.variant}`}><td>{group.metric}</td><td>{group.distribution}</td><td>{group.variant}</td><td>{number(group.mean)}</td><td>{group.sampleSize}</td><td>{interval(group.meanInterval95)}</td><td>{group.comparison?.relativeChangePercent == null ? "—" : `${number(group.comparison.relativeChangePercent)}%`}</td><td>{interval(group.comparison?.differenceInterval95 ?? null)}</td></tr>)}</tbody></table></div>
      <details className="growth-measurement-analysis"><summary>How the exploratory intervals work</summary><p className="growth-muted">95% normal approximation over independent observations. A mean interval needs at least two rows in a group; a difference interval needs at least two rows in both the variant and baseline. These intervals are descriptive and do not establish significance, causality or a winner.</p></details>
      {report.warnings.map((warning) => <p className="growth-muted" key={warning}>{warning}</p>)}
      <p className="growth-measurement-limit">Measured from your file; no causality, significance or growth lift is inferred.</p>
    </div>}
  </section>;
}
