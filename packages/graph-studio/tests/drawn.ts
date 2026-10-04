/** The state the chrome tests draw over, built by hand. No React here, so plain unit tests can use it. */
import type { LayoutParamSpec } from "../src/motor/protocol.ts";
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

/**
 * The layout the drawn state is under: one the motor really publishes three parameters for, so
 * the Layout settings panel has a schema to draw from and a drawn state is a coherent one.
 */
export const DRAWN_LAYOUT = "layout.force.spring";

/** What the motor publishes for {@link DRAWN_LAYOUT}, as the worker hands it over. */
export const DRAWN_SCHEMA: readonly LayoutParamSpec[] = [
  { name: "iterations", kind: "int", min: 1, max: 10_000, default: 50, step: 1, doc: "maximum gathers of the pass" },
  { name: "threshold", kind: "float", min: 0, max: 1, default: 0.0001, step: 0.000001, doc: "early exit below this" },
  { name: "scale", kind: "float", min: 0.000001, max: 1_000_000, default: 5, step: 0.5, doc: "final extent of the drawing" },
];

const RUN: RunSummary = {
  // The layout the settings asked for, so the drawn state is the coherent one: a drawing under
  // the layout whose schema this fixture holds.
  layoutId: DRAWN_LAYOUT, postId: null, postError: null, digest: DIGEST, byteLength: 64,
  nodeKind: "Point", edgeKind: "Line", dim: 0, layoutMs: 12.5, postMs: 0, notes: [],
};

export const CATALOG = {
  layouts: [DRAWN_LAYOUT, "layout.forceatlas2", "layout.grid"],
  posts: ["post.style.bezier"],
  analyses: ["analysis.depth.bfs"],
};

export const META = metaOf(NODES, ["a", "b", "c"], ENDS);

/**
 * What the drawn state is run at, and what the last run was run at: one value the user moved,
 * so the panel has a value of its own and the defaults are somewhere to go back to. The two
 * agree, or every panel reset on this fixture plans a relayout nothing can answer.
 */
const HELD = { iterations: 120 } as const;

export const DRAWN: StudioState = {
  ...initialState(),
  catalog: CATALOG,
  graph: { name: "three", nodeCount: 3, edgeCount: 2, notes: [], buildMs: 1 },
  meta: META,
  run: RUN,
  runParams: JSON.stringify(HELD),
  schemas: { [DRAWN_LAYOUT]: DRAWN_SCHEMA },
  settings: { ...DEFAULT_SETTINGS, layout: DRAWN_LAYOUT, params: { [DRAWN_LAYOUT]: HELD } },
};
