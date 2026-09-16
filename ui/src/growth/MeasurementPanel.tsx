import { useState, type FormEvent } from "react";
import { useQuery } from "@tanstack/react-query";
import { workspaceKey } from "../queries/client";
import { m } from "../paraglide/messages.js";
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
    <h2 id="growth-measurement-title">{m.growth_measure_title()}</h2>
    <p>{m.growth_measure_intro()}</p>
    <details className="growth-measurement-analysis" open>
      <summary>{m.growth_measure_source_boundary()}</summary>
      {sources.isPending ? <p className="growth-muted">{m.growth_measure_loading_sources()}</p> : sources.error ? <p className="growth-muted">{m.growth_measure_sources_unavailable()}</p> : <ul>{sources.data?.map((source) => <li key={source.id}><strong>{source.label}</strong> · {source.status === "available" ? m.growth_measure_available_locally() : m.growth_measure_planned()} · {source.network === "none" ? m.growth_measure_no_network() : m.growth_measure_opt_in_network()}<br /><span className="growth-muted">{source.limitation}</span></li>)}</ul>}
    </details>
    <form onSubmit={summarize}>
      <label className="growth-file-label" htmlFor="growth-measurement-file">{m.growth_measure_telemetry_csv()}<input id="growth-measurement-file" type="file" accept=".csv,text/csv" onChange={(event) => { void readFile(event.target.files?.[0]); }} /></label>
      <label htmlFor="growth-measurement-baseline">{m.growth_measure_baseline_variant()}</label>
      <input id="growth-measurement-baseline" value={baseline} onChange={(event) => setBaseline(event.target.value)} maxLength={256} required />
      <button className="growth-button" type="submit" disabled={reading || !csv}>{reading ? m.growth_measure_reading() : m.growth_measure_summarize_locally()} <span aria-hidden="true">→</span></button>
    </form>
    {fileName && <p className="growth-measurement-file">{m.growth_measure_loaded_locally()} <code>{fileName}</code></p>}
    {error && <p className="growth-error" role="alert">{error}</p>}
    {report && <div className="growth-measurement-result" aria-live="polite">
      <div className="growth-line"><strong>{m.growth_measure_descriptive_summary()}</strong><span className="growth-mark growth-mark-observed">{report.provenance}</span></div>
      <p className="growth-muted">{report.rowsIncluded} {m.growth_measure_observations()} · baseline <code>{report.baselineVariant}</code>{report.dateRange ? ` · ${report.dateRange.from} → ${report.dateRange.to}` : ` · ${m.growth_measure_date_unavailable()}`}</p>
      <div className="growth-measurement-table-wrap"><table className="growth-measurement-table"><thead><tr><th>{m.growth_measure_metric()}</th><th>{m.growth_measure_distribution()}</th><th>{m.growth_measure_variant()}</th><th>{m.growth_measure_mean()}</th><th>{m.growth_measure_sample_size()}</th><th>{m.growth_measure_mean_interval()}</th><th>{m.growth_measure_change()}</th><th>{m.growth_measure_difference_interval()}</th></tr></thead><tbody>{report.groups.map((group) => <tr key={`${group.metric}-${group.distribution}-${group.variant}`}><td>{group.metric}</td><td>{group.distribution}</td><td>{group.variant}</td><td>{number(group.mean)}</td><td>{group.sampleSize}</td><td>{interval(group.meanInterval95)}</td><td>{group.comparison?.relativeChangePercent == null ? "—" : `${number(group.comparison.relativeChangePercent)}%`}</td><td>{interval(group.comparison?.differenceInterval95 ?? null)}</td></tr>)}</tbody></table></div>
      <details className="growth-measurement-analysis"><summary>{m.growth_measure_analysis_summary()}</summary><p className="growth-muted">{m.growth_measure_analysis_body()}</p></details>
      {report.warnings.map((warning) => <p className="growth-muted" key={warning}>{warning}</p>)}
      <p className="growth-measurement-limit">{m.growth_measure_limit()}</p>
    </div>}
  </section>;
}
