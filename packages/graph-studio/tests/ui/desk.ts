/** A studio to draw chrome over: a motor that refuses everything, and a state built by hand. */
import type { ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";

import type { ViewStats } from "../../../graph-render/src/view.ts";
import type { View } from "../../../graph-render/src/view.ts";
import { metaOf } from "../../src/source/meta.ts";
import { type RunSummary, type StudioState, initialState } from "../../src/state/model.ts";
import { desk, refusingClient, type Desk } from "../desk.ts";
import { node } from "../support.ts";

const NODES = [
  node("a", { label: "Alpha", group: "red", weight: 0.5 }),
  node("b", { label: "Beta", group: "red", weight: 1 }),
  node("c", { label: "Gamma", group: "blue", weight: 2 }),
];
const ENDS = { source: Uint32Array.of(0, 1), target: Uint32Array.of(1, 2) };

export { refusingClient };

export const DIGEST = "0123456789abcdef0123456789abcdef";

const RUN: RunSummary = {
  layoutId: "layout.forceatlas2", postId: null, postError: null, digest: DIGEST, byteLength: 64,
  nodeKind: "Point", edgeKind: "Line", layoutMs: 12.5, postMs: 0, notes: [],
};

export const CATALOG = {
  layouts: ["layout.forceatlas2", "layout.grid"],
  posts: ["post.style.bezier"],
  analyses: ["analysis.depth.bfs"],
};

export const META = metaOf(NODES, ["a", "b", "c"], ENDS);

export const DRAWN: StudioState = {
  ...initialState(),
  catalog: CATALOG,
  graph: { name: "three", nodeCount: 3, edgeCount: 2, notes: [], buildMs: 1 },
  meta: META,
  run: RUN,
};

export const IDLE: StudioState = initialState();

export const STATS: ViewStats = {
  backend: "canvas2d", nodes: 3, edges: 2, drawnNodes: 3, drawnEdges: 2, drawnLabels: 2,
  draws: 12, frameMs: 4.2, fps: 60, frames: 42,
};

export function studioWith(state: StudioState = DRAWN): Desk {
  const made = desk(refusingClient());
  made.studio.store.set(state);
  return made;
}

/** The parts of a view the chrome uses, recording nothing: effects do not run here. */
export function fakeView(): Pick<View, "stats" | "on" | "focus" | "select"> {
  return {
    stats: () => STATS,
    focus: () => undefined,
    select: () => undefined,
    on: () => () => undefined,
  };
}

export function markup(element: ReactElement): string {
  return renderToStaticMarkup(element);
}
