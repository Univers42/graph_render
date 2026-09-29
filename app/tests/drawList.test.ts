// Column → draw list. The mapping is where a snapshot's six geometry kinds
// (Point/Circle/Box, Line/Polyline/Curve) become something a canvas can draw, so
// every branch is pinned: a swapped constant, an off-by-one in the offsets slice
// or a `pts` view aliased instead of copied fails here.

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  type ColumnInput,
  DrawListError,
  POINT_RADIUS,
  buildDrawList,
  describeColumns,
} from "../src/core/drawList.ts";

function f32(values: number[]): Float32Array {
  return new Float32Array(values);
}

function u32(values: number[]): Uint32Array {
  return new Uint32Array(values);
}

/** A two-node, one-edge snapshot with every optional column absent. */
function baseInput(over: Partial<ColumnInput> = {}): ColumnInput {
  return {
    nodeKind: "Point",
    edgeKind: "Line",
    x: f32([10, 20]),
    y: f32([-5, 5]),
    r: null,
    w: null,
    h: null,
    source: u32([0]),
    target: u32([1]),
    offsets: null,
    pts: null,
    curveDegree: null,
    ...over,
  };
}

test("Point nodes get the fixed studio radius and zero extent", () => {
  const list = buildDrawList(baseInput());
  assert.equal(list.nodeKind, "Point");
  assert.equal(list.nodes.length, 2);
  assert.deepEqual(
    { ...list.nodes[0] },
    { index: 0, x: 10, y: -5, w: 0, h: 0, r: POINT_RADIUS },
  );
  assert.deepEqual({ ...list.nodes[1] }, { index: 1, x: 20, y: 5, w: 0, h: 0, r: POINT_RADIUS });
});

test("POINT_RADIUS is the pinned default a Point node draws at", () => {
  assert.equal(POINT_RADIUS, 4.5);
});

test("Circle nodes take r from the r column, extent 2r", () => {
  const list = buildDrawList(baseInput({ nodeKind: "Circle", r: f32([3, 12]) }));
  assert.equal(list.nodes[0].r, 3);
  assert.equal(list.nodes[0].w, 6);
  assert.equal(list.nodes[0].h, 6);
  assert.equal(list.nodes[1].r, 12);
  assert.equal(list.nodes[1].w, 24);
});

test("Box nodes take w/h from their columns and r is half the short side", () => {
  const list = buildDrawList(baseInput({ nodeKind: "Box", w: f32([40, 10]), h: f32([20, 30]) }));
  assert.deepEqual(
    list.nodes.map((n) => ({ w: n.w, h: n.h, r: n.r })),
    [
      { w: 40, h: 20, r: 10 },
      { w: 10, h: 30, r: 5 },
    ],
  );
});

test("a Circle snapshot with no r column is refused, not drawn at radius 0", () => {
  assert.throws(() => buildDrawList(baseInput({ nodeKind: "Circle" })), DrawListError);
  assert.throws(
    () => buildDrawList(baseInput({ nodeKind: "Circle" })),
    /Circle nodes need the r column/,
  );
});

test("a Box snapshot missing w or h is refused", () => {
  assert.throws(() => buildDrawList(baseInput({ nodeKind: "Box", w: f32([1, 2]) })), /Box nodes need the w and h columns/);
  assert.throws(() => buildDrawList(baseInput({ nodeKind: "Box", h: f32([1, 2]) })), /Box nodes need the w and h columns/);
});

test("a y column of a different length than x is refused", () => {
  assert.throws(() => buildDrawList(baseInput({ y: f32([1, 2, 3]) })), /x and y must be the same length/);
});

test("Line edges carry no points and no curve degree", () => {
  const list = buildDrawList(baseInput());
  assert.equal(list.edgeKind, "Line");
  assert.equal(list.edges.length, 1);
  assert.deepEqual(
    { ...list.edges[0], pts: list.edges[0].pts.length },
    { index: 0, source: 0, target: 1, pts: 0, degree: 0 },
  );
});

test("Polyline edges are sliced out of pts by the offsets column", () => {
  const offsets = u32([0, 4, 10]);
  const pts = f32([0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9]);
  const list = buildDrawList(
    baseInput({ edgeKind: "Polyline", source: u32([0, 1]), target: u32([1, 0]), offsets, pts }),
  );
  assert.equal(list.edges[0].pts.length, 8);
  assert.deepEqual([...list.edges[0].pts], [0, 0, 1, 1, 2, 2, 3, 3]);
  assert.equal(list.edges[1].pts.length, 12);
  assert.deepEqual([...list.edges[1].pts], [4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9]);
  assert.equal(list.edges[0].degree, 0);
});

test("a Polyline edge's points are a copy, not a window on the motor's memory", () => {
  const pts = f32([0, 0, 1, 1, 2, 2, 3, 3]);
  const list = buildDrawList(baseInput({ edgeKind: "Polyline", offsets: u32([0, 4]), pts }));
  pts[0] = 999;
  assert.equal(list.edges[0].pts[0], 0);
});

test("Curve edges carry the one snapshot-wide degree on every edge", () => {
  const list = buildDrawList(
    baseInput({
      edgeKind: "Curve",
      offsets: u32([0, 2]),
      pts: f32([0, 0, 1, 1]),
      curveDegree: u32([3]),
    }),
  );
  assert.equal(list.edges[0].degree, 3);
  assert.equal(list.edges[0].pts.length, 4);
});

test("a Curve snapshot with no degree column is refused", () => {
  assert.throws(
    () => buildDrawList(baseInput({ edgeKind: "Curve", offsets: u32([0, 2]), pts: f32([0, 0, 1, 1]) })),
    /Curve edges need the degree column/,
  );
});

test("a Polyline/Curve snapshot with no offsets column is refused", () => {
  assert.throws(
    () => buildDrawList(baseInput({ edgeKind: "Polyline", pts: f32([0, 0, 1, 1]) })),
    /Polyline and Curve edges need the offsets column/,
  );
});

test("offsets must start at 0, never grow, and cover m+1 entries", () => {
  assert.throws(
    () => buildDrawList(baseInput({ edgeKind: "Polyline", offsets: u32([2, 4]), pts: f32([0, 0, 1, 1]) })),
    /offsets must start at 0/,
  );
  assert.throws(
    () => buildDrawList(baseInput({ edgeKind: "Polyline", source: u32([0, 1]), target: u32([1, 0]), offsets: u32([0, 4, 2]), pts: f32([0, 0, 1, 1, 2, 2, 3, 3]) })),
    /offsets must not decrease/,
  );
  assert.throws(
    () => buildDrawList(baseInput({ edgeKind: "Polyline", offsets: u32([0, 2, 4]), pts: f32([0, 0, 1, 1, 2, 2, 3, 3]) })),
    /offsets must have one entry per edge plus one/,
  );
  assert.throws(
    () => buildDrawList(baseInput({ edgeKind: "Polyline", offsets: u32([0, 4]), pts: f32([0, 0, 1, 1]) })),
    /offsets run past the end of pts/,
  );
  assert.throws(
    () => buildDrawList(baseInput({ edgeKind: "Polyline", offsets: u32([0, 2]) })),
    /Polyline and Curve edges need the pts column/,
  );
});

test("an edge endpoint outside the node range is refused", () => {
  assert.throws(() => buildDrawList(baseInput({ target: u32([2]) })), /edge 0 targets node 2 of 2/);
  assert.throws(() => buildDrawList(baseInput({ source: u32([7]) })), /edge 0 starts at node 7 of 2/);
});

test("describeColumns reports a length per column and null for an absent one", () => {
  const rows = describeColumns(
    baseInput({ nodeKind: "Circle", edgeKind: "Polyline", r: f32([1, 2]), offsets: u32([0, 2]), pts: f32([0, 0, 1, 1]) }),
  );
  assert.deepEqual(
    rows.map((row) => [row.name, row.length]),
    [
      ["node.x", 2],
      ["node.y", 2],
      ["node.r", 2],
      ["node.w", null],
      ["node.h", null],
      ["edge.source", 1],
      ["edge.target", 1],
      ["edge.offsets", 2],
      ["edge.pts", 4],
      ["edge.curveDegree", null],
      ["note.code", null],
      ["note.index", null],
    ],
  );
});
