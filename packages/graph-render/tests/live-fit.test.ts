/**
 * A live simulation's positions are a frame like any other. While the view still owns the
 * camera — `fitted`, which only a fit sets — a frame that lands off the box the camera was
 * fitted to is fitted again, because otherwise the drawing walks off the screen and never
 * comes back. Once a hand has taken the camera, the live positions move the drawing and
 * nothing else, or every frame a running layout sends would yank the view back.
 *
 * The controller is driven directly, as `live-positions.test.ts` does: `createView` needs an
 * OffscreenCanvas and a requestAnimationFrame that node does not have.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { panBy, worldToScreen } from "../src/camera.ts";
import { cameraApi } from "../src/camera-api.ts";
import {
  type Controller, fit, moveTo, newState, showFrame,
} from "../src/canvas2d/controller.ts";
import { type Frame, frameFrom } from "../src/frame.ts";
import { newLocalLayer } from "../src/local.ts";
import { decodeSnapshot } from "../src/snapshot/decode.ts";
import { lineFrame, spaceBytes } from "./support.ts";

const VIEWPORT = { width: 800, height: 600 };
/** Far enough past the fitted box that no camera left where it was could still show it. */
const FAR = Float64Array.from([9000, 9100, 9200]);
const FAR_Y = Float64Array.from([9000, 9100, 9000]);

// The controller reads only getContext from the canvas; the guard stands in for a DOM that node lacks.
function isCanvas(value: unknown): value is HTMLCanvasElement {
  return typeof value === "object" && value !== null && "getContext" in value;
}

const noop = (): void => {};

/** A controller showing `frame` at a real viewport, with no camera fitted yet. */
function showing(frame: Frame): Controller {
  const canvas: unknown = { getContext: () => ({}) };
  if (!isCanvas(canvas)) throw new Error("the stand-in canvas lost its getContext");
  const setup = { theme: undefined, policy: undefined, onFrame: noop };
  const state = newState(canvas, setup);
  showFrame(state, frame, false);
  state.viewport = VIEWPORT;
  state.destroyed = true; // invalidate() then schedules nothing: node has no requestAnimationFrame
  const notify = { hover: noop, select: noop, selection: noop, context: noop, camera: noop };
  return { canvas, state, notify, fitted: false, local: newLocalLayer() };
}

/** Three nodes on a line, fitted: the drawing both camera tests start from. */
function fittedLine(): Controller {
  const controller = showing(lineFrame({ x: [0, 100, 200], y: [0, 0, 0], edges: [[0, 1]] }));
  fit(controller);
  return controller;
}

/** Every node's drawn point, so a fit can be checked against the box rather than the camera. */
function onScreen(controller: Controller): { x: number; y: number }[] {
  const { state } = controller;
  const out: { x: number; y: number }[] = [];
  for (let node = 0; node < state.scene.frame.nodeCount; node += 1) {
    out.push(worldToScreen(state.camera, { x: state.x[node] ?? 0, y: state.y[node] ?? 0 }));
  }
  return out;
}

/** Asserts every node is inside the viewport, naming the first one that is not. */
function assertInside(controller: Controller, why: string): void {
  for (const [node, at] of onScreen(controller).entries()) {
    assert.ok(at.x >= 0 && at.x <= VIEWPORT.width && at.y >= 0 && at.y <= VIEWPORT.height,
      `${why}: node ${node} is drawn at ${at.x},${at.y}`);
  }
}

test("a live frame that lands off the box the camera was fitted to is fitted again", () => {
  const controller = fittedLine();
  assertInside(controller, "the fit under test already shows the drawing");
  cameraApi(controller).setPositions(FAR, FAR_Y);
  assertInside(controller, "a camera the view owns still shows the drawing after a live frame");
  assert.equal(controller.fitted, true, "the view still owns the camera: a live frame did not take it");
});

test("a live frame does not take back a camera the user moved", () => {
  const controller = fittedLine();
  moveTo(controller, panBy(controller.state.camera, { x: 300, y: 0 }), false);
  const taken = cameraApi(controller).camera();
  cameraApi(controller).setPositions(FAR, FAR_Y);
  assert.deepEqual(cameraApi(controller).camera(), taken, "a hand's camera survives a live frame");
});

test("a pair of another graph's length leaves the drawing and the camera alone", () => {
  const controller = fittedLine();
  const before = [0, 1, 2].map((node) => cameraApi(controller).position(node));
  const camera = cameraApi(controller).camera();
  const others = Float64Array.from([9, 9, 9, 9, 9]);
  cameraApi(controller).setPositions(others, others);
  assert.deepEqual([0, 1, 2].map((node) => cameraApi(controller).position(node)), before, "no node moved");
  assert.deepEqual(cameraApi(controller).camera(), camera, "and the camera is the one it was fitted to");
});

test("a live frame on a 3D drawing refits the orbit", () => {
  const bytes = spaceBytes({ x: [0, 100, 200], y: [0, 0, 0], z: [0, 50, 0], edges: [[0, 1]] });
  const controller = showing(frameFrom(decodeSnapshot(bytes)));
  fit(controller);
  const orbit = cameraApi(controller).orbit();
  assert.ok(orbit !== null, "a 3D frame is drawn through an orbit");
  cameraApi(controller).setPositions(FAR, FAR_Y);
  assert.notDeepEqual(cameraApi(controller).orbit(), orbit, "the drawing moved, so the orbit did too");
});
