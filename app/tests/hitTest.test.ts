// Hit-testing and neighbour lookup — the two pure queries behind hover and
// click-to-highlight. The exact winner at an overlap and the exact neighbour set
// are pinned, because both are the difference between "clicked the node I meant"
// and "clicked whatever happened to be drawn last".

import assert from "node:assert/strict";
import { test } from "node:test";

import { type DrawList, DrawListError } from "../src/core/drawList.ts";
import { neighborsOf, pickNeighbour, hitTestNode } from "../src/core/hitTest.ts";

function list(over: Partial<DrawList> = {}): DrawList {
  return {
    nodeKind: "Circle",
    edgeKind: "Line",
    nodes: [
      { index: 0, x: 0, y: 0, w: 20, h: 20, r: 10 },
      { index: 1, x: 100, y: 0, w: 20, h: 20, r: 10 },
      { index: 2, x: 100, y: 100, w: 20, h: 20, r: 10 },
    ],
    edges: [
      { index: 0, source: 0, target: 1, pts: new Float32Array(0), degree: 0 },
      { index: 1, source: 1, target: 2, pts: new Float32Array(0), degree: 0 },
    ],
    ...over,
  };
}

test("a hit inside a node's radius returns its dense index", () => {
  assert.equal(hitTestNode(list(), { x: 103, y: 97 }, { scale: 1, tolerancePx: 0 }), 2);
});

test("the tolerance is in screen pixels, converted through the camera scale", () => {
  // Node 0 has r=10, so x=12 misses by 2 world units — 0.1px at scale 0.05. The
  // tolerance is what makes it grabbable again, and it is a SCREEN size: the same
  // pixel count at any zoom.
  assert.equal(hitTestNode(list(), { x: 12, y: 0 }, { scale: 0.05, tolerancePx: 0 }), -1);
  assert.equal(hitTestNode(list(), { x: 12, y: 0 }, { scale: 0.05, tolerancePx: 1 }), 0);
  assert.equal(hitTestNode(list(), { x: 12, y: 0 }, { scale: 0.05, tolerancePx: 0.1 }), 0);
  assert.equal(hitTestNode(list(), { x: 12, y: 0 }, { scale: 0.05, tolerancePx: 0.05 }), -1);
});

test("a miss returns -1, never null", () => {
  assert.equal(hitTestNode(list(), { x: 50, y: 50 }, { scale: 1, tolerancePx: 0 }), -1);
});

test("the topmost node wins an overlap: the last dense index drawn", () => {
  const stacked = list({
    nodes: [
      { index: 0, x: 0, y: 0, w: 0, h: 0, r: 50 },
      { index: 1, x: 0, y: 0, w: 0, h: 0, r: 50 },
    ],
    edges: [],
  });
  assert.equal(hitTestNode(stacked, { x: 0, y: 0 }, { scale: 1, tolerancePx: 0 }), 1);
});

test("a Box node is hit anywhere inside its rectangle, corners included", () => {
  const boxes = list({
    nodeKind: "Box",
    nodes: [
      { index: 0, x: 0, y: 0, w: 100, h: 40, r: 20 },
      { index: 1, x: 0, y: 0, w: 100, h: 40, r: 20 },
    ],
    edges: [],
  });
  assert.equal(hitTestNode(boxes, { x: 50, y: 20 }, { scale: 1, tolerancePx: 0 }), 1);
  assert.equal(hitTestNode(boxes, { x: 51, y: 0 }, { scale: 1, tolerancePx: 0 }), -1);
});

test("neighborsOf is the set of nodes one edge away, excluding the node itself", () => {
  assert.deepEqual([...neighborsOf(list(), 1)].sort((a, b) => a - b), [0, 2]);
  assert.deepEqual([...neighborsOf(list(), 0)], [1]);
  assert.deepEqual([...neighborsOf(list(), 99)], []);
});

test("a self edge does not make a node its own neighbour", () => {
  const loop = list({
    edges: [{ index: 0, source: 0, target: 0, pts: new Float32Array(0), degree: 0 }],
  });
  assert.deepEqual([...neighborsOf(loop, 0)], []);
});

test("neighborsOf is sorted ascending, so a highlight never flickers between orders", () => {
  const many = list({
    edges: [
      { index: 0, source: 2, target: 0, pts: new Float32Array(0), degree: 0 },
      { index: 1, source: 1, target: 0, pts: new Float32Array(0), degree: 0 },
    ],
  });
  assert.deepEqual([...neighborsOf(many, 0)], [1, 2]);
});

test("pickNeighbour returns null past the end and the neighbour otherwise", () => {
  const set = neighborsOf(list(), 1);
  assert.equal(pickNeighbour(set, 0), 0);
  assert.equal(pickNeighbour(set, 1), 2);
  assert.equal(pickNeighbour(set, 2), null);
  assert.equal(pickNeighbour(new Set(), 0), null);
});

test("hitTestNode refuses a list with no nodes rather than looping over nothing", () => {
  assert.throws(() => hitTestNode(list({ nodes: [] }), { x: 0, y: 0 }, { scale: 1, tolerancePx: 0 }), DrawListError);
});
