/** What each colour on the canvas stands for, and how many nodes wear it. */
import { memo, type ReactElement } from "react";

import { parseQuery } from "../console/parse.ts";
import type { Query } from "../console/parse.ts";
import { matchesQuery, rowOf } from "../console/queryMatch.ts";
import { type AnalysisValues, type LegendEntry, legendOf } from "../look/styleOf.ts";
import type { AnalysisReport } from "../motor/protocol.ts";
import type { GraphMeta } from "../source/meta.ts";
import type { Appearance, Filter, Group, Settings } from "../state/settings.ts";

export interface LegendProps {
  /** The graph the counts are read off: every count below is rebuilt when this changes. */
  readonly meta: GraphMeta | null;
  readonly settings: Settings;
  readonly analysis: AnalysisReport | null;
}

/** Everything the panel draws, read off one graph and one settings document. */
export interface LegendView {
  /** One count per named group, in the order the groups are in. */
  readonly counts: readonly number[];
  /** One row per colour the drawing falls back on, in palette order. */
  readonly rows: readonly LegendEntry[];
}

/** The analysis the drawing is coloured by is the one the state holds, or none. */
function analysisOf(analysis: AnalysisReport | null): AnalysisValues | null {
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
function parsedOf(group: Group): Query | null {
  try {
    return parseQuery(group.query);
  } catch {
    return null;
  }
}

/** One walk of the nodes for every group: each query is parsed once, each row is built once. */
function countAll(meta: GraphMeta, groups: readonly Group[]): readonly number[] {
  const parsed = groups.map(parsedOf);
  const counts = new Array<number>(groups.length).fill(0);
  if (!parsed.some((query) => query !== null)) return counts;
  for (let node = 0; node < meta.nodeCount; node += 1) {
    const row = rowOf(meta, node);
    for (const [group, query] of parsed.entries()) {
      if (query !== null && matchesQuery(query, row)) counts[group] = (counts[group] ?? 0) + 1;
    }
  }
  return counts;
}

/** What the cache was built from: identities, so a miss is a comparison and not a hash. */
interface CacheKey {
  readonly groups: readonly Group[];
  readonly appearance: Appearance;
  readonly filter: Filter;
  readonly analysis: AnalysisReport | null;
  readonly view: LegendView;
}

/**
 * Caveat: one view per graph object, and only for the settings it was built from. A graph
 * equal in every field but new is walked again (correct, slower), and two settings
 * documents alternating over one graph rebuild the view on each switch (correct, slower).
 * Nothing here can go stale; a miss is a rebuild.
 */
const CACHE = new WeakMap<GraphMeta, CacheKey>();

/** Whether the held view was built from this settings document and this analysis. */
function fresh(held: CacheKey | undefined, settings: Settings, analysis: AnalysisReport | null): held is CacheKey {
  return held !== undefined
    && held.groups === settings.groups
    && held.appearance === settings.appearance
    && held.filter === settings.filter
    && held.analysis === analysis;
}

/**
 * The panel's whole reading of the graph, computed once per (meta, settings.groups) instead
 * of once per render: the node walk and the colour rows are the two costs this panel pays.
 */
export function legendViewOf(props: LegendProps): LegendView | null {
  const { meta, settings, analysis } = props;
  if (meta === null) return null;
  const held = CACHE.get(meta);
  if (fresh(held, settings, analysis)) return held.view;
  const view: LegendView = {
    counts: countAll(meta, settings.groups),
    rows: legendOf({ meta, appearance: settings.appearance, filter: settings.filter, groups: [], analysis: analysisOf(analysis) }),
  };
  CACHE.set(meta, {
    groups: settings.groups, appearance: settings.appearance, filter: settings.filter, analysis, view,
  });
  return view;
}

function groupOf(groups: readonly Group[], counts: readonly number[]): ReactElement[] {
  return groups.map((group, at) => (
    <div className="gs-legend-row" role="listitem" key={`group:${group.name}`}>
      <span className="gs-swatch" style={{ background: group.colour }} />
      <span className="gs-label">{group.name}</span>
      <span className="gs-count">{counts[at] ?? 0}</span>
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

/** Memoised: the legend reads three slices, and a store change to any other one is not its news. */
export const Legend = memo(function Legend(props: LegendProps): ReactElement | null {
  const view = legendViewOf(props);
  if (view === null) return null;
  // Ponytail: the rows below the groups describe the colouring the groups fall back on, so a
  // node a group paints wears the group's swatch above and is still counted in a row below.
  const groups = props.settings.groups;
  if (view.rows.length === 0 && groups.length === 0) return null;
  // The named groups come first and in the order the studio was given them: the swatch
  // carries the colour literally, so a probe reads back exactly what was asked for.
  return (
    <div className="gs-panel gs-legend" role="list" aria-label="Legend">
      {groupOf(groups, view.counts)}
      {view.rows.map(colourOf)}
    </div>
  );
});