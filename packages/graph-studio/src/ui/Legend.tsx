/** What each colour on the canvas stands for, and how many nodes wear it. */
import type { ReactElement } from "react";

import { type AnalysisValues, legendOf } from "../look/styleOf.ts";
import type { StudioState } from "../state/model.ts";

export interface LegendProps {
  readonly state: StudioState;
}

/** The analysis the drawing is coloured by is the one the state holds, or none. */
function analysisOf(state: StudioState): AnalysisValues | null {
  const { analysis } = state;
  if (analysis === null) return null;
  return { id: analysis.id, kind: analysis.kind, values: analysis.values };
}

export function Legend(props: LegendProps): ReactElement | null {
  const { meta, settings } = props.state;
  if (meta === null) return null;
  const rows = legendOf({
    meta, appearance: settings.appearance, filter: settings.filter, analysis: analysisOf(props.state),
  });
  if (rows.length === 0) return null;
  return (
    <div className="gs-panel gs-legend" role="list" aria-label="Legend">
      {rows.map((row) => (
        <div className="gs-legend-row" role="listitem" key={row.label}>
          <span className="gs-swatch" style={{ background: row.colour }} />
          <span className="gs-label">{row.label}</span>
          <span className="gs-count">{row.count}</span>
        </div>
      ))}
    </div>
  );
}
