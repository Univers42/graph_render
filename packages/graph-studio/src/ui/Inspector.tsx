/** The selected node: what the drawing knows about it, and what it is joined to. */
import { memo, type ReactElement } from "react";

import type { View } from "../../../graph-render/src/view.ts";
import type { AnalysisReport } from "../motor/protocol.ts";
import type { GraphMeta } from "../source/meta.ts";
import type { Studio } from "../studio/studio.ts";
import { shortName, sig3 } from "./names.ts";

/** More neighbours than this and the panel is a list of the graph, not of one node. */
const SHOWN = 12;

export interface InspectorProps {
  readonly studio: Studio;
  /** The four slices the panel draws; the rest of the state is not its news. */
  readonly meta: GraphMeta | null;
  readonly selected: number;
  readonly analysis: AnalysisReport | null;
  readonly selection: readonly number[];
  /**
   * WHY it is here and unread: every control in this panel is an action now, so nothing here
   * touches the drawing itself — but `Shell` is not this change's to edit, and a prop it does
   * not pass is a type error. It goes when the shell's call site can be.
   */
  readonly view: Pick<View, "focus" | "select">;
}

/**
 * WHY the id and not the index: an action takes a name and resolves it (`nodeNamed`,
 * `actions/view.ts`), so a click is checked and refused in one place and the line it logs is
 * the line a reader can type. A dense index means nothing outside this panel.
 */
function focusOn(studio: Studio, meta: GraphMeta, node: number): void {
  const id = meta.ids[node];
  if (id === undefined) return;
  void studio.dispatch("view.focus", { node: id });
}

function Row(props: { readonly name: string; readonly value: string }): ReactElement {
  const { name, value } = props;
  return (
    <div className="gs-kv">
      <span className="gs-kv-key">{name}</span>
      <span className="gs-kv-value">{value}</span>
    </div>
  );
}

function Neighbours(props: {
  readonly studio: Studio;
  readonly meta: GraphMeta;
  readonly at: number;
}): ReactElement | null {
  const { studio, meta, at } = props;
  const found = studio.neighbours(at);
  if (found.length === 0) return null;
  const rest = found.length - Math.min(found.length, SHOWN);
  return (
    <div className="gs-neighbours" role="group" aria-label="Neighbours">
      {found.slice(0, SHOWN).map((node) => (
        <button
          key={node}
          type="button"
          className="gs-btn"
          aria-label={`Centre ${meta.labels[node] ?? ""}`}
          onClick={() => focusOn(studio, meta, node)}
        >
          {meta.labels[node] ?? ""}
        </button>
      ))}
      {rest > 0 && <span className="gs-more">{`+${rest} more`}</span>}
    </div>
  );
}

/** Several nodes selected: how many, and the first of them to jump to. */
function Selection(props: { readonly studio: Studio; readonly meta: GraphMeta; readonly nodes: readonly number[] }): ReactElement | null {
  const { studio, meta, nodes } = props;
  if (nodes.length < 2) return null;
  return (
    <div className="gs-neighbours" role="group" aria-label="Selection">
      <span className="gs-more">{`${nodes.length} selected`}</span>
      {nodes.slice(0, SHOWN).map((node) => (
        <button key={node} type="button" className="gs-btn" aria-label={`Centre ${meta.labels[node] ?? ""}`} onClick={() => focusOn(studio, meta, node)}>
          {meta.labels[node] ?? ""}
        </button>
      ))}
      {nodes.length > SHOWN && <span className="gs-more">{`+${nodes.length - SHOWN} more`}</span>}
    </div>
  );
}

/** Memoised: the panel draws one node, and re-renders when the node or the graph changes. */
export const Inspector = memo(function Inspector(props: InspectorProps): ReactElement | null {
  const { studio, meta, selected, analysis, selection } = props;
  if (selected < 0 || meta === null) return null;
  const label = meta.labels[selected] ?? "";
  const measured = analysis !== null && analysis.values.length === meta.nodeCount
    ? analysis.values[selected] : undefined;
  return (
    <div className="gs-panel gs-inspector" aria-label="The selected node">
      <div className="gs-head">
        <h2 className="gs-head-name gs-title">{label}</h2>
        <button type="button" className="gs-btn" aria-label="Close the inspector" onClick={() => void studio.dispatch("view.unselect")}>
          ×
        </button>
      </div>
      <Selection studio={studio} meta={meta} nodes={selection} />
      <Row name="Id" value={meta.ids[selected] ?? ""} />
      <Row name="Kind" value={meta.kinds[selected] ?? ""} />
      <Row name="Group" value={meta.groups[meta.group[selected] ?? 0] ?? ""} />
      <Row name="Degree" value={String(meta.degree[selected] ?? 0)} />
      <Row name="Weight" value={sig3(meta.weight[selected] ?? 0)} />
      {measured !== undefined && <Row name={shortName(analysis?.id ?? "")} value={sig3(measured)} />}
      <Neighbours studio={studio} meta={meta} at={selected} />
    </div>
  );
});
