/**
 * The force knobs kept in the settings: a set reaches the motor and the store, a new graph's
 * session is given the saved knobs once its own force run lands (never on the run left over
 * from the graph before), and the drawn radius is converted to layout units at the zoom.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { Camera } from "../../graph-render/src/camera.ts";
import type { Frame } from "../../graph-render/src/frame.ts";
import { EMPTY_FRAME } from "../../graph-render/src/scene.ts";
import type { ViewEvents } from "../../graph-render/src/view.ts";
import { type ForceLink, NO_FORCE_LINK } from "../src/actions/forces.ts";
import { DEFAULT_KNOBS, type ForceKnobs } from "../src/motor/live.ts";
import { drawnRadius, keepForces } from "../src/state/keepForces.ts";
import type { StudioState } from "../src/state/model.ts";
import { withSettings } from "../src/state/settings.ts";
import { type Store, createStore } from "../src/state/store.ts";
import type { ViewFace } from "../src/studio/pipeline.ts";
import { DRAWN } from "./drawn.ts";

type Handlers = { [Name in keyof ViewEvents]: Set<(payload: ViewEvents[Name]) => void> };

const DISCS: Frame = { ...EMPTY_FRAME, nodeKind: "Circle", nodeCount: 3, r: Float32Array.of(2, 5, 3) };
const SPREAD: ForceKnobs = { ...DEFAULT_KNOBS, collideRadius: 9, charge: -270 };

interface Rig {
  readonly store: Store<StudioState>;
  readonly sent: ForceKnobs[];
  readonly heats: (number | undefined)[];
  readonly zoom: (scale: number) => void;
  readonly stop: () => void;
  readonly link: ForceLink;
}

function fakeView(frame: Frame, handlers: Handlers): Pick<ViewFace, "on" | "frame"> {
  return {
    frame: () => frame,
    on: (name, handler) => {
      handlers[name].add(handler);
      return () => void handlers[name].delete(handler);
    },
  };
}

function rig(state: StudioState = DRAWN, frame: Frame = DISCS): Rig {
  const store = createStore(state);
  const sent: ForceKnobs[] = [];
  const heats: (number | undefined)[] = [];
  const handlers: Handlers = { hover: new Set(), select: new Set(), selection: new Set(), camera: new Set(), context: new Set(), frame: new Set() };
  const set = (knobs: ForceKnobs, heat?: number): void => {
    sent.push(knobs);
    heats.push(heat);
  };
  const inner = { ...NO_FORCE_LINK, disabled: () => null, set };
  const kept = keepForces(store, inner, fakeView(frame, handlers));
  const zoom = (scale: number): void => {
    const camera: Camera = { x: 0, y: 0, scale };
    for (const handler of handlers.camera) handler(camera);
  };
  return { store, sent, heats, zoom, stop: kept.stop, link: kept.link };
}

function withForces(state: StudioState, forces: ForceKnobs): StudioState {
  return { ...state, settings: withSettings(state.settings, { forces }) };
}

/** A new graph loaded: the run on screen is still the old graph's until the new one lands. */
function loaded(state: StudioState, name: string): StudioState {
  return { ...state, graph: { name, nodeCount: 3, edgeCount: 2, notes: [], buildMs: 1 } };
}

function ranOn(state: StudioState, layoutId: string): StudioState {
  const run = state.run ?? DRAWN.run;
  if (run === null) throw new Error("the drawn fixture has a run");
  return { ...state, run: { ...run, layoutId } };
}

/** A load, then its run, as two publishes: the pipeline never writes both in one. */
function nextGraph(store: Store<StudioState>, layoutId: string): void {
  store.update((state) => loaded(state, "next"));
  store.update((state) => ranOn(state, layoutId));
}

test("a set reaches the motor and is written into the settings, so it is saved", () => {
  const { store, sent, link } = rig();
  link.set(SPREAD);
  assert.deepEqual(sent, [SPREAD]);
  assert.deepEqual(store.get().settings.forces, SPREAD);
  assert.deepEqual(link.knobs(), SPREAD, "the link reads the knobs back from the store");
  assert.equal(link.knobs(DRAWN).collideRadius, DEFAULT_KNOBS.collideRadius, "or from the state it is given");
});

test("a new graph gets the saved knobs once its own force run lands, not on the run before", () => {
  const { store, sent } = rig(withForces(DRAWN, SPREAD));
  store.update((state) => loaded(state, "next"));
  assert.deepEqual(sent, [], "the run on screen still belongs to the graph before");
  store.update((state) => ranOn(state, "layout.force.barnes_hut"));
  assert.deepEqual(sent, [SPREAD]);
  store.update((state) => ranOn(state, "layout.forceatlas2.barnes_hut"));
  assert.deepEqual(sent, [SPREAD], "the same session already has them");
});

test("default knobs and a layout that does not settle live push nothing", () => {
  const plain = rig();
  nextGraph(plain.store, "layout.force.barnes_hut");
  assert.deepEqual(plain.sent, [], "a fresh session already holds the defaults");
  const frozen = rig(withForces(DRAWN, SPREAD));
  nextGraph(frozen.store, "layout.grid");
  assert.deepEqual(frozen.sent, []);
});

test("knobs changed outside the link (an import, a recalled source) reach the session on screen", () => {
  const { store, sent } = rig();
  nextGraph(store, "layout.force.barnes_hut");
  store.update((state) => withForces(state, SPREAD));
  assert.deepEqual(sent, [SPREAD]);
});

test("stop unsubscribes from the store and the camera", () => {
  const { store, sent, zoom, stop, link } = rig(withForces(DRAWN, SPREAD));
  zoom(0.1);
  stop();
  zoom(10);
  nextGraph(store, "layout.force.barnes_hut");
  assert.deepEqual(sent, []);
  assert.equal(link.drawn?.(), 12.5, "the scale held is the last one seen before the stop");
});

test("the drawn radius is the largest disc in layout units, floored at the painter's minimum on screen", () => {
  assert.equal(drawnRadius(DRAWN, DISCS, 1), 5);
  assert.equal(drawnRadius(DRAWN, DISCS, 0.1), 12.5, "1.25 px at a tenth of a pixel per unit");
  const boxes: Frame = { ...EMPTY_FRAME, nodeKind: "Box", nodeCount: 2, w: Float32Array.of(4, 10), h: Float32Array.of(12, 2) };
  assert.equal(drawnRadius(DRAWN, boxes, 1), 6, "half the larger side of a box");
});

test("the drawn radius of points is the style's, and nothing drawn is null", () => {
  const points: Frame = { ...EMPTY_FRAME, nodeCount: 3 };
  const styled = drawnRadius(DRAWN, points, 1);
  assert.ok(styled !== null && styled > 0, `a styled radius, got ${String(styled)}`);
  assert.equal(drawnRadius({ ...DRAWN, meta: null }, DISCS, 1), null);
  assert.equal(drawnRadius(DRAWN, EMPTY_FRAME, 1), null);
  assert.equal(drawnRadius(DRAWN, DISCS, 0), null);
});

test("the link's drawn radius follows the camera", () => {
  const { zoom, link } = rig();
  const { drawn } = link;
  if (drawn === undefined) throw new Error("the kept link can see the drawing");
  assert.equal(drawn(), 5, "at the starting scale of 1");
  zoom(0.05);
  assert.equal(drawn(), 25);
});

test("a set's heat reaches the motor; a knob set without one asks for none", () => {
  const { link, heats, stop } = rig();
  link.set(SPREAD, 1);
  link.set(DEFAULT_KNOBS);
  assert.deepEqual(heats, [1, undefined]);
  stop();
});
