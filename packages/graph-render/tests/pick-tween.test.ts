/**
 * Picking while the nodes are easing: a node is found where it is drawn, on both backends.
 *
 * The pick grid indexes the *target* frame, so it cannot answer a mid-tween query, and the
 * guard that used to answer `-1` for the whole tween (`canvas2d/controller.pickAt`) meant a
 * moving node could not be clicked, hovered or labelled by a pointer at all. The tween's own
 * pose — its two halves and the eased fraction the loop already publishes — is scanned once
 * instead (`scene.pickEased`), which is one pass of a few flops a node and runs at most once a
 * frame per query.
 *
 * The controls are the two rows that are not the headline: the target frame is *not* what is
 * drawn, so a row that found the node at its target would prove the grid answered; and the
 * settled frame after the tween picks through the grid again, so a build that always scanned
 * would fail it too.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { type LoopState, advance } from "../src/canvas2d/loop.ts";
import { newState, pickAt, showFrame } from "../src/canvas2d/controller.ts";
import { type EasedPose, pickEased, pickIn, sceneOf } from "../src/scene.ts";
import { plainStyle } from "../src/style.ts";
import type { BackendChoice } from "../src/webgl2/plan.ts";
import { TRANSITION_MS } from "../src/transition.ts";
import { lineFrame, mulberry32, randomFrame as randFrame } from "./support.ts";

const START = { x: [0, 100, 200], y: [0, 0, 0] };
const END = { x: [0, 300, 600], y: [0, 100, 0] };

// The controller reads only getContext from the canvas; the guard stands in for a DOM node lacks.
function isCanvas(value: unknown): value is HTMLCanvasElement {
  return typeof value === "object" && value !== null && "getContext" in value;
}

/** Three nodes, a layout switch in flight, stepped to half the tween with no browser frames. */
function halfTween(backend: BackendChoice): LoopState {
  const canvas: unknown = { getContext: () => ({}) };
  if (!isCanvas(canvas)) throw new Error("the stand-in canvas lost its getContext");
  const state = newState(canvas, { theme: undefined, policy: undefined, onFrame: () => {}, backend });
  showFrame(state, lineFrame(START), false);
  state.camera = { x: 0, y: 0, scale: 1 };
  state.destroyed = true; // invalidate() then schedules nothing: node has no requestAnimationFrame
  showFrame(state, lineFrame(END), true);
  const start = state.transitionStart;
  advance(state, start + TRANSITION_MS / 2);
  return state;
}

/** The eased pose, mixed the way the node shader's `place()` mixes it. */
function drawnAt(state: LoopState, node: number): { x: number; y: number } {
  const tween = state.bulk.tween;
  if (tween === null) throw new Error("a tween in flight publishes its two halves and its fraction");
  const mix = (from: number, to: number): number => from + (to - from) * tween.eased;
  return { x: mix(tween.fromX[node] ?? 0, tween.toX[node] ?? 0), y: mix(tween.fromY[node] ?? 0, tween.toY[node] ?? 0) };
}

test("at half a tween a click finds the node where the backend draws it", () => {
  for (const backend of ["canvas2d", "webgl2"] as const) {
    const state = halfTween(backend);
    const tween = state.bulk.tween;
    assert.notEqual(tween, null, `${backend}: the loop published the tween`);
    assert.equal(tween?.eased, 0.5, `${backend}: half the tween is half eased`);
    for (const node of [1, 2]) {
      const drawn = drawnAt(state, node);
      assert.equal(pickAt(state, drawn), node, `${backend}: node ${node} is picked where it is drawn`);
    }
  }
});

test("the target frame is not what is drawn, so a pick there misses", () => {
  const state = halfTween("canvas2d");
  assert.equal(pickAt(state, { x: 600, y: 0 }), -1, "node 2 is not at its target yet");
  assert.equal(pickAt(state, { x: 200, y: 0 }), -1, "nor at the place it came from");
});

test("a hover and a click on empty space are -1, not a nearest node", () => {
  const state = halfTween("canvas2d");
  assert.equal(pickAt(state, { x: 900, y: 900 }), -1);
  assert.equal(pickAt(state, { x: 404, y: 0 }), 2, "the tolerance still reaches the node it has");
});

test("the settled frame after a tween picks through the grid again", () => {
  const state = halfTween("canvas2d");
  const start = state.transitionStart;
  advance(state, start + 10_000);
  assert.equal(state.transitionStart, -1, "the tween ended");
  assert.equal(state.bulk.tween, null, "and published nothing more to mix");
  assert.equal(pickAt(state, { x: 600, y: 0 }), 2, "the node is picked at its settled place");
  assert.equal(pickAt(state, { x: 400, y: 0 }), -1, "and not at the place it was passing through");
});

/**
 * The scan and the grid have to be the same pick, not two that mostly agree.
 *
 * At `eased` of 1 the eased pose *is* the frame, so every query must return what `pickIn`
 * returns for it — same node, same -1, ties included. This is the control the axis early-out in
 * `pickEased` needs: it rejects a node on `dx` alone before reading `dy`, `extent` or `hidden`,
 * and a span that is one unit too small would show up here as a miss near the edge of the
 * tolerance rather than as a wrong node. 600 seeded queries over a 200-node graph, half of them
 * within a node radius of a centre and half anywhere, so both the hits and the near misses are
 * covered.
 */
test("at eased 1 the scan returns exactly what the grid returns", () => {
  const nodes = 200;
  const { frame, scene } = sceneOf(randFrame(nodes, 2 * nodes, 3), plainStyle(nodes), null);
  const settled: EasedPose = { fromX: frame.x.slice(), fromY: frame.y.slice(), toX: frame.x, toY: frame.y, eased: 1 };
  const query = { x: 0, y: 0, tolerance: 4, floor: 1.25 };
  const next = mulberry32(11);
  let hits = 0;
  for (let round = 0; round < 600; round += 1) {
    const on = round % 2 === 0;
    const node = Math.floor(next() * nodes);
    query.x = on ? (frame.x[node] ?? 0) + (next() - 0.5) * 20 : next() * 1000;
    query.y = on ? (frame.y[node] ?? 0) + (next() - 0.5) * 20 : next() * 1000;
    const scanned = pickEased(scene, query, settled);
    assert.equal(scanned, pickIn(scene, query), `round ${round} at ${query.x.toFixed(2)},${query.y.toFixed(2)}`);
    if (scanned >= 0) hits += 1;
  }
  assert.ok(hits > 20, `the queries have to reach the nodes to prove anything (${hits} hits)`);
});