/**
 * The graph area: one panel, or two side by side in compare mode. React owns the
 * DOM around the canvases and nothing inside them — `GraphView` is imperative and
 * draws from the wasm run's geometry, so a re-render never repaints a graph.
 *
 * It also owns the two things that need the live view rather than the last run:
 * the hover tooltip (which needs the cursor position, not the geometry) and the
 * PNG export (which composites the aurora field under the graph).
 */

import { useEffect, useRef, useState } from "react";

import { GraphView } from "../render/graphView.ts";
import type { Point } from "../core/hitTest.ts";
import type { RunReport } from "../motor/session.ts";
import type { EdgeKind } from "../../../src/core/types.ts";
import type { NodeStyle } from "../render/palette.ts";

export interface GraphPanelProps {
  readonly run: RunReport | null;
  readonly second: RunReport | null;
  readonly styles: readonly NodeStyle[];
  readonly edgeKinds: readonly EdgeKind[];
  readonly nodeCount: number;
  readonly edgeCount: number;
}

interface Tooltip {
  readonly text: string;
  readonly at: Point;
}

interface PaneProps {
  readonly title: string;
  readonly report: RunReport | null;
  readonly styles: readonly NodeStyle[];
  readonly edgeKinds: readonly EdgeKind[];
  readonly fileName: string;
}

function download(name: string, url: string): void {
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
}

/** The tooltip line for the node under the cursor: its label, its kind, and the
 *  dense index the click handlers speak in. */
function tooltipText(styles: readonly NodeStyle[], index: number): string {
  const style = styles[index];
  return style === undefined ? `#${index}` : `${style.label} · ${style.kind} · #${index}`;
}

function paneTitle(report: RunReport | null): string {
  if (report === null) return "no run yet";
  return `${report.layoutId} · ${report.nodeKind}/${report.edgeKind} · ${report.nodeCount} nodes`;
}

/** Create the view once per pane and feed it every run. The callbacks are made
 *  once too, so they read the current run and styles through a ref rather than
 *  closing over the first render's values. */
type PaneView = {
  hostRef: React.RefObject<HTMLDivElement | null>;
  viewRef: React.RefObject<GraphView | null>;
  mounted: boolean;
};

/** Create the view once per pane. The callbacks are made once too, so they read
 *  the current run and styles through a ref rather than closing over the first
 *  render's values. */
function useCreateView(
  props: PaneProps,
  setTooltip: (tooltip: Tooltip | null) => void,
): PaneView {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const viewRef = useRef<GraphView | null>(null);
  const latest = useRef({ report: props.report, styles: props.styles });
  latest.current = { report: props.report, styles: props.styles };
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    const host = hostRef.current;
    if (host === null) return;
    const view = new GraphView(host, {
      onHover: (index, at) => {
        const current = latest.current;
        setTooltip(index < 0 || at === null || current.report === null ? null : {
          text: tooltipText(current.styles, index),
          at,
        });
      },
      onSelect: () => {},
    });
    view.setReducedMotion(globalThis.matchMedia?.("(prefers-reduced-motion: reduce)").matches === true);
    viewRef.current = view;
    setMounted(true);
    return () => {
      view.destroy();
      viewRef.current = null;
    };
  }, [setTooltip]);

  return { hostRef, viewRef, mounted };
}

/** Feed the view every run. */
function useRunData(props: PaneProps, viewRef: React.RefObject<GraphView | null>): void {
  useEffect(() => {
    const view = viewRef.current;
    if (view === null || props.report === null) return;
    view.setData({
      list: props.report.list,
      styles: props.styles,
      edgeKinds: props.edgeKinds,
      // Animate from what is on screen only when it is the SAME graph: a new
      // document must not morph into the next one.
      animate: view.hasData() && view.nodeCount() === props.report.list.nodes.length,
    });
  }, [props.edgeKinds, props.report, props.styles, viewRef]);
}

/** One canvas panel bound to one run. */
function Pane(props: PaneProps): React.JSX.Element {
  const [tooltip, setTooltip] = useState<Tooltip | null>(null);
  const { hostRef, viewRef, mounted } = useCreateView(props, setTooltip);
  useRunData(props, viewRef);
  return (
    <div className="pane">
      <div className="pane__head">
        <span className="pane__title">{`${props.title} — ${paneTitle(props.report)}`}</span>
        <button
          type="button"
          disabled={!mounted || props.report === null}
          onClick={() => {
            const view = viewRef.current;
            if (view !== null) download(props.fileName, view.exportPng());
          }}
        >
          Export PNG
        </button>
      </div>
      <div className="panel" ref={hostRef}>
        {tooltip !== null && (
          <div className="tooltip" style={{ left: `${tooltip.at.x}px`, top: `${tooltip.at.y}px` }}>
            {tooltip.text}
          </div>
        )}
      </div>
    </div>
  );
}

export function GraphPanel(props: GraphPanelProps): React.JSX.Element {
  const shared = { styles: props.styles, edgeKinds: props.edgeKinds };
  return (
    <div className={props.second === null ? "graph" : "graph graph--compare"}>
      <Pane {...shared} title="primary" report={props.run} fileName="graph-motor-studio.png" />
      {props.second !== null && (
        <Pane {...shared} title="second" report={props.second} fileName="graph-motor-studio-compare.png" />
      )}
      <footer className="graph__foot">
        {props.nodeCount} nodes · {props.edgeCount} edges in the ingest — drag to pan, wheel to zoom,
        double-click to fit, click a node to highlight its neighbours
      </footer>
    </div>
  );
}
