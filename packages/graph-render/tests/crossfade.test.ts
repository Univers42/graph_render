/**
 * The two ways a change reaches the screen without a cut: `crossFade` (canvas2d/crossfade.ts) lays
 * a copy of the old picture over the new one and fades it out, and a carried `start`
 * (FrameOptions) moves the nodes of another document from where they were drawn. Node has no
 * `OffscreenCanvas` and no animation frames, so both are stood in for and stepped by hand.
 */
import assert from "node:assert/strict";
import { afterEach, test } from "node:test";

import { type Controller, newState } from "../src/canvas2d/controller.ts";
import { CROSSFADE_MS, crossFading, paintCrossFade, startCrossFade } from "../src/canvas2d/crossfade.ts";
import type { LoopState } from "../src/canvas2d/loop.ts";
import { sceneApi } from "../src/camera-api.ts";
import { newLocalLayer } from "../src/local.ts";
import { lineFrame } from "./support.ts";

interface Painter {
  readonly canvas: { width: number; height: number };
  readonly drawn: number[];
  depth: number;
  transforms: number;
  globalAlpha: number;
  save(): void;
  restore(): void;
  setTransform(): void;
  drawImage(): void;
}

function painter(): Painter {
  return {
    canvas: { width: 160, height: 90 }, drawn: [], depth: 0, transforms: 0, globalAlpha: 1,
    save() { this.depth += 1; },
    restore() { this.depth -= 1; this.globalAlpha = 1; },
    setTransform() { this.transforms += 1; },
    drawImage() { this.drawn.push(this.globalAlpha); },
  };
}

let copies = 0;

class StandInOffscreen {
  readonly width: number;
  readonly height: number;
  constructor(width: number, height: number) {
    this.width = width;
    this.height = height;
  }
  getContext(): { drawImage(): void } {
    return { drawImage: () => void (copies += 1) };
  }
}

function withOffscreen(): void {
  copies = 0;
  Object.defineProperty(globalThis, "OffscreenCanvas", { value: StandInOffscreen, configurable: true, writable: true });
}

afterEach(() => void Reflect.deleteProperty(globalThis, "OffscreenCanvas"));

// The controller reads only getContext from the canvas; the guard stands in for a DOM node lacks.
function isCanvas(value: unknown): value is HTMLCanvasElement {
  return typeof value === "object" && value !== null && "getContext" in value;
}

function view(ctx: unknown = {}): { controller: Controller; state: LoopState } {
  const canvas: unknown = { getContext: () => ctx };
  if (!isCanvas(canvas)) throw new Error("the stand-in canvas lost its getContext");
  const state = newState(canvas, { theme: undefined, policy: undefined, onFrame: () => {} });
  state.viewport = { width: 800, height: 600 };
  state.destroyed = true; // invalidate() then schedules nothing: node has no requestAnimationFrame
  const notify = { hover() {}, select() {}, selection() {}, context() {}, camera() {} };
  return { controller: { canvas, state, notify, fitted: true, gestured: false, local: newLocalLayer() }, state };
}

test("a crossFade copies the screen, then fades the copy out over the paints that follow", () => {
  withOffscreen();
  const ctx = painter();
  const { state } = view(ctx);
  startCrossFade(state);
  assert.equal(copies, 1, "the picture on screen was copied");
  paintCrossFade(state, 1000);
  paintCrossFade(state, 1000 + CROSSFADE_MS / 2);
  assert.deepEqual(ctx.drawn, [1, 0.5], "opaque on the first paint, half way at the midpoint");
  assert.ok(crossFading(state));
  paintCrossFade(state, 1000 + CROSSFADE_MS);
  assert.deepEqual(ctx.drawn, [1, 0.5], "nothing is laid over the scene once the fade is over");
  assert.ok(!crossFading(state), "and the copy is released");
  assert.equal(ctx.depth, 0, "every save was restored");
  assert.equal(ctx.transforms, 2, "the copy is laid in canvas pixels, whatever the camera");
});

test("changes during a fade share its one copy; a change after it copies again", () => {
  withOffscreen();
  const { state } = view(painter());
  startCrossFade(state);
  startCrossFade(state);
  paintCrossFade(state, 0);
  startCrossFade(state);
  assert.equal(copies, 1, "every change before the fade ended landed under the first copy");
  paintCrossFade(state, CROSSFADE_MS);
  startCrossFade(state);
  assert.equal(copies, 2);
});

test("without OffscreenCanvas a crossFade does nothing and the change cuts", () => {
  const { state } = view(painter());
  startCrossFade(state);
  assert.ok(!crossFading(state));
});

test("a resize drops the copy instead of laying it over at the old size", () => {
  withOffscreen();
  const ctx = painter();
  const { state } = view(ctx);
  startCrossFade(state);
  ctx.canvas.width = 320;
  paintCrossFade(state, 0);
  assert.deepEqual(ctx.drawn, []);
  assert.ok(!crossFading(state));
});

test("the view's crossFade starts the fade", () => {
  withOffscreen();
  const { controller, state } = view(painter());
  sceneApi(controller).crossFade(100);
  assert.equal(state.crossFade?.ms, 100);
});

const OLD = { x: [0, 100, 200], y: [0, 0, 0] };

/** A view showing OLD under a camera the next fit will not keep. */
function shown(): { controller: Controller; state: LoopState; before: { x: number; y: number; scale: number } } {
  const { controller, state } = view();
  sceneApi(controller).setFrame(lineFrame(OLD));
  state.camera = { x: 10, y: 20, scale: 2 };
  return { controller, state, before: state.camera };
}

test("a carried start moves the nodes of another document from where they were drawn", () => {
  const { controller, state, before } = shown();
  const next = lineFrame({ x: [500, 0, 900, 40], y: [500, 0, 900, 40] });
  const start = { x: Float32Array.from([200, 0, Number.NaN, 100]), y: Float32Array.from([0, 0, Number.NaN, 0]) };
  sceneApi(controller).setFrame(next, { animate: true, start });
  assert.ok(state.transitionStart >= 0, "it moves although the node count changed");
  assert.equal(state.fromFrame, null, "the old routes are another node order's, so none is morphed");
  const { camera } = state;
  assert.notDeepEqual(camera, before, "the control: the fit chose another camera");
  for (const node of [0, 1, 3]) {
    const screen = (state.fromX[node] ?? 0) * camera.scale + camera.x;
    assert.ok(Math.abs(screen - ((start.x[node] ?? 0) * before.scale + before.x)) < 1e-3, `node ${node} starts at its old screen point`);
  }
  assert.equal(state.fromX[2], 900, "a node the old document lacked starts where it ends");
  assert.deepEqual(Array.from(state.x), Array.from(state.fromX), "the first frame draws the start");
});

test("a start of another length is ignored and a resized frame still cuts", () => {
  const { controller, state } = shown();
  const start = { x: Float32Array.from([0, 1]), y: Float32Array.from([0, 1]) };
  sceneApi(controller).setFrame(lineFrame({ x: [0, 1, 2, 3], y: [0, 1, 2, 3] }), { animate: true, start });
  assert.equal(state.transitionStart, -1);
});

test("a layout switch starts from the picture on screen, not from the old world under the new camera", () => {
  const { controller, state, before } = shown();
  sceneApi(controller).setFrame(lineFrame({ x: [0, 1000, 2000], y: [0, 500, 0] }), { animate: true });
  const { camera } = state;
  const screen = (state.fromX[2] ?? 0) * camera.scale + camera.x;
  assert.ok(Math.abs(screen - (200 * before.scale + before.x)) < 1e-3);
});
