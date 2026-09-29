// The frame the transition produces is pure maths over two draw lists, so it is
// tested like one: the first run, a matched pair, and the two mismatches that
// have to cross-fade instead of morphing.

import assert from "node:assert/strict";
import { test } from "node:test";

import { frameFor } from "../src/core/frame.ts";
import type { DrawList } from "../src/core/drawList.ts";

function list(nodeCount: number, over: Partial<DrawList> = {}): DrawList {
  return {
    nodeKind: "Point",
    edgeKind: "Line",
    nodes: Array.from({ length: nodeCount }, (_, index) => ({ index, x: index * 10, y: 0, w: 0, h: 0, r: 4.5 })),
    edges: [],
    ...over,
  };
}

function polyline(pointRuns: number[][]): DrawList {
  return list(2, {
    edgeKind: "Polyline",
    edges: pointRuns.map((pts, index) => ({
      index, source: 0, target: 1, pts: new Float32Array(pts), degree: 0,
    })),
  });
}

test("a first run draws its own geometry at full alpha, with nothing to morph from", () => {
  const frame = frameFor(list(3), null, 0.5);
  assert.deepEqual([...frame.x], [0, 10, 20]);
  assert.equal(frame.alpha, 1);
  assert.equal(frame.morph, false);
  assert.deepEqual(frame.paths, []);
});

test("a first run still carries a Polyline edge's OWN points — they are its geometry", () => {
  // Not an interpolation result: a routed edge's points are the layout's output,
  // and dropping them because there is nothing to interpolate FROM would draw a
  // first-run polyline as nothing at all.
  const to = polyline([[0, 0, 5, 5, 10, 10]]);
  const frame = frameFor(to, null, 0);
  assert.deepEqual(frame.paths[0] === null ? null : [...frame.paths[0]], [0, 0, 5, 5, 10, 10]);
});

test("a matched pair interpolates node positions and reports morphing", () => {
  const frame = frameFor(list(3), list(3), 0.5);
  assert.equal(frame.morph, true);
  assert.equal(frame.alpha, 1);
  assert.equal(frame.x[0], 0);
  assert.equal(frame.x[1], 10);
});

test("a different node count cross-fades the target instead of morphing", () => {
  const frame = frameFor(list(5), list(3), 0.25);
  assert.equal(frame.morph, false);
  assert.equal(frame.alpha, 0.25);
  assert.equal(frame.x.length, 5);
  assert.equal(frame.x[4], 40);
});

test("a path of fewer than two points is not a path", () => {
  // One point cannot be stroked, and passing it on would draw a zero-length
  // stroke for every frame of a transition. The renderer also checks, but the
  // frame is where the decision belongs.
  const frame = frameFor(polyline([[7, 7]]), null, 1);
  assert.equal(frame.paths[0], null);
});

test("a Line edge has no path of its own, whatever t is", () => {
  const to = list(2, { edges: [{ index: 0, source: 0, target: 1, pts: new Float32Array(0), degree: 0 }] });
  assert.deepEqual(frameFor(to, list(2), 0.5).paths, [null]);
});

test("matching polyline paths are interpolated, unmatched ones fall back to endpoints", () => {
  const to = polyline([[0, 0, 5, 5], [0, 0, 1, 1]]);
  const from = polyline([[0, 0, 10, 10], [0, 0, 1, 1, 2, 2]]);
  const frame = frameFor(to, from, 0.5);
  assert.deepEqual(frame.paths[0] === null ? null : [...frame.paths[0]], [0, 0, 7.5, 7.5]);
  assert.equal(frame.paths[1], null);
});

test("edges only the target has are drawn from the node positions", () => {
  const to = polyline([[0, 0, 1, 1], [0, 0, 2, 2]]);
  const frame = frameFor(to, polyline([[0, 0, 1, 1]]), 0.5);
  assert.equal(frame.paths.length, 2);
  assert.equal(frame.paths[1], null);
});

test("a Line -> Polyline change never morphs: the kinds disagree", () => {
  const to = polyline([[0, 0, 1, 1]]);
  const from = list(2, { edges: [{ index: 0, source: 0, target: 1, pts: new Float32Array(0), degree: 0 }] });
  assert.equal(frameFor(to, from, 0.5).paths[0], null);
});
