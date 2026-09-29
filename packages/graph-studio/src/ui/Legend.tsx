/** What each colour on the canvas stands for, and how many nodes wear it. */
import type { ReactElement } from "react";

import { parseQuery } from "../console/parse.ts";
import type { Query } from "../console/parse.ts";
import { matchesQuery, rowOf } from "../console/queryMatch.ts";
import { type AnalysisValues, type LegendEntry, legendOf } from "../look/styleOf.ts";
import type { GraphMeta } from "../source/meta.ts";
import type { StudioState } from "../state/model.ts";
import type { Group } from "../state/settings.ts";

export interface LegendProps {
  readonly state: StudioState;
}

/** The analysis the drawing is coloured by is the one the state holds, or none. */
function analysisOf(state: StudioState): AnalysisValues | null {
  const { analysis } = state;
  if (analysis === null) return null;
  return { id: analysis.id, kind: analysis.kind, values: analysis.values };
}

/**
 * Ponytail: a query that does not parse counts zero rather than being dropped from the
 * legend. Failing input: the row is already on screen with its colour, and a legend that
 * hid it would deny the studio is drawing that group at all. Direction it errs: the count
 * reads 0 where the real number is unknown. Escape hatch: the console prints the refusal
 * beside the line that asked for it.
 */
function countOf(meta: GraphMeta, group: Group): number {
  let query: Query;
  try {
    query = parseQuery(group.query);
  } catch {
    return 0;
  }
  let found = 0;
  for (let node = 0; node < meta.nodeCount; node += 1) {
    if (matchesQuery(query, rowOf(meta, node))) found += 1;
  }
  return found;
}

function groupOf(meta: GraphMeta, groups: readonly Group[]): ReactElement[] {
  return groups.map((group) => (
    <div className="gs-legend-row" role="listitem" key={`group:${group.name}`}>
      <span className="gs-swatch" style={{ background: group.colour }} />
      <span className="gs-label">{group.name}</span>
      <span className="gs-count">{countOf(meta, group)}</span>
    </div>
  ));
}

function colourOf(row: LegendEntry): ReactElement {
  return (
    <div className="gs-legend-row" role="listitem" key={row.label}>
      <span className="gs-swatch" style={{ background: row.colour }} />
      <span className="gs-label">{row.label}</span>
      <span className="gs-count">{row.count}</span>
    </div>
  );
}

export function Legend(props: LegendProps): ReactElement | null {
  const { meta, settings } = props.state;
  if (meta === null) return null;
  const rows = legendOf({
    meta, appearance: settings.appearance, filter: settings.filter, groups: [], analysis: analysisOf(props.state),
  });
  // Ponytail: the rows below the groups describe the colouring the groups fall back on, so a
  // node a group paints wears the group's swatch above and is still counted in a row below.
  const groups = settings.groups;
  if (rows.length === 0 && groups.length === 0) return null;
  // The named groups come first and in the order the studio was given them: the swatch
  // carries the colour literally, so a probe reads back exactly what was asked for.
  return (
    <div className="gs-panel gs-legend" role="list" aria-label="Legend">
      {groupOf(meta, groups)}
      {rows.map(colourOf)}
    </div>
  );
}
