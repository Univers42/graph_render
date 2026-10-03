/**
 * `focus()` asked for while the nodes are still easing to a new layout.
 *
 * The choice, and the reason it is a choice: a focus centres the position the node *settles*
 * at, not the position it is passing through. The camera is placed there in one step and
 * nothing moves it afterwards, so for the rest of the tween the node walks into the middle of
 * the frame rather than the middle chasing it — the picture the user asked for is the picture
 * they end up with, and a focus that followed the eased pose would have to be re-aimed every
 * frame of the tween to land anywhere.
 *
 * The controls: the settled frame after the tween centres the same place (so this is not a
 * focus that only works mid-tween), and a focus on a settled frame is unchanged (so nothing
 * here moved the settled path).
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { type Controller, newState, showFrame } from "../src/canvas2d/controller.ts";
import { type LoopState, advance } from "../src/canvas2d/loop.ts";
import { cameraApi } from "../src/camera-api.ts";
import { screenToWorld } from "../src/camera.ts";
import { newLocalLayer } from "../src/local.ts";
import { TRANSITION_MS } from "../src/transition.ts";
import { lineFrame } from "./support.ts";

const START = { x: [0, 100, 200], y: [0, 0, 0] };
const END = { x: [0, 300, 600], y: [0, 100, 0] };
const NODE = 2;
const VIEWPORT = { width: 800, height: 600 };

// The controller reads only getContext from the canvas; the guard stands in for a DOM node lacks.
function isCanvas(value: unknown): value is HTMLCanvasElement {
  return typeof value === "object" && value !== null && "getContext" in value;
}

function view(travelling: boolean): { controller: Controller; state: LoopState } {
  const canvas: unknown = { getContext: () => ({}) };
  if (!isCanvas(canvas)) throw new Error("the stand-in canvas lost its getContext");
  const state = newState(canvas, { theme: undefined, policy: undefined, onFrame: () => {} });
  state.viewport = VIEWPORT;
  showFrame(state, lineFrame(START), false);
  state.camera = { x: 0, y: 0, scale: 1 };
  state.destroyed = true; // invalidate() then schedules nothing: node has no requestAnimationFrame
  if (travelling) {
    showFrame(state, lineFrame(END), true);
    advance(state, state.transitionStart + TRANSITION_MS / 2);
  }
  const notify = { hover() {}, select() {}, selection() {}, context() {}, camera() {} };
  const controller = { canvas, state, notify, fitted: true, local: newLocalLayer() };
  return { controller, state };
}

/** The world point the middle of the viewport is looking at. */
function centred(controller: Controller): { x: number; y: number } {
  const { state } = controller;
  return screenToWorld(state.camera, { x: VIEWPORT.width / 2, y: VIEWPORT.height / 2 });
}

test("a focus mid-tween centres where the node settles, not where it is passing", () => {
  const { controller, state } = view(true);
  const eased = { x: state.x[NODE] ?? 0, y: state.y[NODE] ?? 0 };
  const settled = { x: state.scene.frame.x[NODE] ?? 0, y: state.scene.frame.y[NODE] ?? 0 };
  assert.notDeepEqual(eased, settled, "the control: the node really is between the two");

  cameraApi(controller).focus(NODE);
  assert.deepEqual(centred(controller), settled, "the camera is aimed at the settled position");
  assert.notDeepEqual(centred(controller), eased, "and not at the eased pose, which would drift");
});

test("the camera stays on that position for the rest of the tween", () => {
  const { controller, state } = view(true);
  cameraApi(controller).focus(NODE);
  const placed = state.camera;
  for (let frame = 1; frame <= 20; frame += 1) advance(state, state.transitionStart + (frame * TRANSITION_MS) / 20);
  assert.equal(state.camera, placed, "no camera tween exists, so nothing moved it");
  assert.deepEqual(centred(controller), { x: 600, y: 0 });
  assert.equal(state.transitionStart, -1, "the tween ended on its own clock");
});

test("a focus is not a fit: a later safe area does not re-frame over it", () => {
  const { controller } = view(true);
  cameraApi(controller).focus(NODE);
  assert.equal(controller.fitted, false, "the camera was chosen, not fitted");
});

test("the control: a focus on a settled frame aims at the same place", () => {
  const { controller } = view(false);
  cameraApi(controller).focus(NODE);
  assert.deepEqual(centred(controller), { x: 200, y: 0 });
  assert.deepEqual(cameraApi(controller).selection(), [NODE], "and it selects, as a focus always has");
});