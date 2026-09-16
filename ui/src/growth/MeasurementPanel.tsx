import { useState, type FormEvent } from "react";
import { parseMeasurementCsv, type LocalMeasurementReport } from "./measurement";

function number(value: number) {
  return new Intl.NumberFormat(undefined, { maximumFractionDigits: 4 }).format(value);
}

export function MeasurementPanel() {
  const [csv, setCsv] = useState("");
  const [fileName, setFileName] = useState("");
  const [baseline, setBaseline] = useState("baseline");
  const [report, setReport] = useState<LocalMeasurementReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [reading, setReading] = useState(false);

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
    <p>Read your own CSV in this browser. GrowthLab calculates descriptive comparisons locally and sends no rows to a provider.</p>
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
      <div className="growth-measurement-table-wrap"><table className="growth-measurement-table"><thead><tr><th>Metric</th><th>Variant</th><th>Mean</th><th>n</th><th>Change</th></tr></thead><tbody>{report.groups.map((group) => <tr key={`${group.metric}-${group.variant}`}><td>{group.metric}</td><td>{group.variant}</td><td>{number(group.mean)}</td><td>{group.sampleSize}</td><td>{group.comparison?.relativeChangePercent == null ? "—" : `${number(group.comparison.relativeChangePercent)}%`}</td></tr>)}</tbody></table></div>
      {report.warnings.map((warning) => <p className="growth-muted" key={warning}>{warning}</p>)}
      <p className="growth-measurement-limit">Measured from your file; no causality, significance or growth lift is inferred.</p>
    </div>}
  </section>;
}
