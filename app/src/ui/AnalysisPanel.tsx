/**
 * The analysis panel: which registered analysis to run over the graph, what it
 * coloured the nodes with, and the scalars it handed back.
 *
 * The list comes from `Motor#analyses()` — the module's own registry, in registry
 * order (C1) — so an analysis registered after this file was written appears here
 * with no change to the studio, and no analysis id is written down in `app/`.
 *
 * The colouring is chosen by the face's OWN `kind`, never by its id: an `f64`
 * face is a magnitude and is ramped across its own value domain, a `u32` face is
 * a labelling and is coloured by group. The legend says which, so the picture is
 * never unlabelled — and the three optional members (`converged`, `modularity`,
 * `max`) are shown exactly when the analysis handed one back, because each is the
 * escape hatch its own analysis names (docs/contract/wasm-abi.md "ANALYSIS").
 */

import { analysisLegend, coversGraph, describeAnalysis } from "../core/analysis.ts";
import type { AnalysisResult } from "../../../crates/graph-sdk-js/src/index.ts";

export interface AnalysisPanelProps {
  readonly analyses: readonly string[];
  readonly analysisId: string | null;
  readonly result: AnalysisResult | null;
  /** Nodes on screen: a face over a different graph is refused here, not drawn. */
  readonly nodeCount: number;
  readonly available: boolean;
  readonly onPick: (id: string) => void;
  readonly onRun: (id: string) => void;
  readonly onClear: () => void;
}

function selected(props: AnalysisPanelProps): string {
  return props.analysisId ?? props.result?.id ?? props.analyses[0] ?? "";
}

export function AnalysisPanel(props: AnalysisPanelProps): React.JSX.Element {
  const value = selected(props);
  const result = props.result;
  const applied = result !== null && coversGraph(result, props.nodeCount);
  return (
    <section className="panel-card">
      <h2>Analysis</h2>
      <label className="field">
        <span>analysis</span>
        <select
          value={value}
          disabled={props.analyses.length === 0}
          onChange={(event) => props.onPick(event.target.value)}
        >
          {props.analyses.length === 0 && <option value="">(no analyses — motor unavailable)</option>}
          {props.analyses.map((id) => (
            <option key={id} value={id}>
              {id}
            </option>
          ))}
        </select>
      </label>
      <div className="row">
        <button
          type="button"
          disabled={!props.available || value === ""}
          onClick={() => props.onRun(value)}
        >
          Run
        </button>
        <button
          type="button"
          className="secondary"
          disabled={result === null}
          onClick={props.onClear}
        >
          Clear
        </button>
      </div>
      {result === null ? (
        <p className="hint">nothing applied — the nodes wear their ingest colours</p>
      ) : (
        <>
          <dl className="rows">
            {describeAnalysis(result).map((row) => (
              <div key={row.label}>
                <dt>{row.label}</dt>
                <dd>{row.value}</dd>
              </div>
            ))}
          </dl>
          {applied ? (
            <ul className="legend">
              {analysisLegend(result).map((entry) => (
                <li key={entry.label}>
                  <span className="swatch" style={{ background: entry.fill }} />
                  {entry.label}
                </li>
              ))}
            </ul>
          ) : (
            <p className="hint">
              this face covers {result.nodeCount} node(s) and the graph has {props.nodeCount} — not drawn
            </p>
          )}
        </>
      )}
      <p className="hint">
        {props.analyses.length} analysis(es) registered by the module · an f64 face ramps across its own
        range, a u32 face colours by group
      </p>
    </section>
  );
}
