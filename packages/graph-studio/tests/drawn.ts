/** The state the chrome tests draw over, built by hand. No React here, so plain unit tests can use it. */
import { metaOf } from "../src/source/meta.ts";
import { type RunSummary, type StudioState, initialState } from "../src/state/model.ts";
import { DEFAULT_SETTINGS } from "../src/state/settings.ts";
import { node } from "./support.ts";

const NODES = [
  node("a", { label: "Alpha", group: "red", weight: 0.5 }),
  node("b", { label: "Beta", group: "red", weight: 1 }),
  node("c", { label: "Gamma", group: "blue", weight: 2 }),
];
const ENDS = { source: Uint32Array.of(0, 1), target: Uint32Array.of(1, 2) };

export const DIGEST = "0123456789abcdef0123456789abcdef";

const RUN: RunSummary = {
  // The layout the settings asked for, so the drawn state is the coherent one: a drawing
  // under the studio's own default. Name the default, not the id it happens to hold today,
  // or every panel reset plans a relayout this fixture has no motor to answer.
  layoutId: DEFAULT_SETTINGS.layout, postId: null, postError: null, digest: DIGEST, byteLength: 64,
  nodeKind: "Point", edgeKind: "Line", dim: 0, layoutMs: 12.5, postMs: 0, notes: [],
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
