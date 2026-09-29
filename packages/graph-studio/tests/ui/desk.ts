/** A studio to draw chrome over: a motor that refuses everything, and a state built by hand. */
import type { ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";

import type { ViewEvents, ViewStats } from "../../../graph-render/src/view.ts";
import type { View } from "../../../graph-render/src/view.ts";
import type { MotorClient } from "../../src/motor/client.ts";
import { metaOf } from "../../src/source/meta.ts";
import { type RunSummary, type StudioState, initialState } from "../../src/state/model.ts";
import { desk, type Desk } from "../desk.ts";
import { node } from "../support.ts";

const NODES = [
  node("a", { label: "Alpha", group: "red", weight: 0.5 }),
  node("b", { label: "Beta", group: "red", weight: 1 }),
  node("c", { label: "Gamma", group: "blue", weight: 2 }),
];
const ENDS = { source: Uint32Array.of(0, 1), target: Uint32Array.of(1, 2) };

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

/** Nothing in the chrome asks the motor; these refuse if anything ever does. */
export function refusingClient(): MotorClient {
  const never = (what: string): never => {
    throw new Error(`the test client was asked to ${what}`);
  };
  return {
    catalog: () => never("open"),
    load: () => never("load"),
    layout: () => never("lay out"),
    analysis: () => never("analyse"),
    cancel: () => false,
    busy: () => false,
    close: () => undefined,
  };
}

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
    on: <Name extends keyof ViewEvents>(_name: Name, _handler: (payload: ViewEvents[Name]) => void) => () => undefined,
  };
}

export function markup(element: ReactElement): string {
  return renderToStaticMarkup(element);
}
