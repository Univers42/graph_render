// What a POST pass does to the studio's copy of a run. The pass runs in wasm and
// replaces the handle's edge geometry, so the studio has to throw away the edge
// columns it read before and re-read every column of the same handle — including
// the geometry KINDS, which a pass changes (`post.route.grid` over a `Line` layout
// yields a `Polyline`). The regressions these pin are a pass that keeps the
// pre-pass report (the panel would say `Line` while the canvas walks a routed
// arc) and a pass that keeps the pre-pass edge count or column list.

import assert from "node:assert/strict";
import { test } from "node:test";

import { type ColumnInput, POINT_RADIUS } from "../src/core/drawList.ts";
import { type RunReport, layoutReport, withPost } from "../src/motor/report.ts";
import type { PostResult } from "../../crates/graph-sdk-js/src/index.ts";

function f32(values: number[]): Float32Array {
  return new Float32Array(values);
}

function u32(values: number[]): Uint32Array {
  return new Uint32Array(values);
}

/** A two-node, one-edge LINE run, as a layout would hand it over. */
function lineColumns(): ColumnInput {
  return {
    nodeKind: "Point",
    edgeKind: "Line",
    x: f32([0, 10]),
    y: f32([0, 0]),
    r: null,
    w: null,
    h: null,
    source: u32([0]),
    target: u32([1]),
    offsets: null,
    pts: null,
    curveDegree: null,
  };
}

/** The same graph after `post.style.quadratic`: a Curve carrying three INTERIOR
 *  points per edge — the endpoints are the node positions, which is why the
 *  first point is at x=5 and the last at x=9, between the two nodes. */
function curveColumns(): ColumnInput {
  return {
    ...lineColumns(),
    edgeKind: "Curve",
    offsets: u32([0, 3]),
    pts: f32([5, 0, 7, 0, 9, 0]),
    curveDegree: u32([3]),
  };
}

const RUN = { nodeKind: "Point", edgeKind: "Line", nodeCount: 2 } as const;
const TIMINGS = { buildMs: 3, layoutMs: 7 };

function baseRun(): RunReport {
  return layoutReport("layout.grid", RUN, TIMINGS, lineColumns());
}

function postResult(over: Partial<PostResult> = {}): PostResult {
  return {
    handle: 1 as PostResult["handle"],
    id: "post.style.quadratic",
    nodeKind: "Point",
    edgeKind: "Curve",
    nodeCount: 2,
    ...over,
  };
}

test("a layout report carries no pass, and counts the edges the draw list found", () => {
  const report = baseRun();
  assert.equal(report.postId, null);
  assert.equal(report.postMs, 0);
  assert.equal(report.edgeKind, "Line");
  assert.equal(report.edgeCount, 1);
  assert.equal(report.list.edges[0].pts.length, 0);
  assert.equal(report.columns.find((column) => column.name === "edge.pts")?.length, null);
});

test("a post pass replaces the draw list with the columns read after it", () => {
  const report = withPost(baseRun(), postResult(), curveColumns(), 4);
  assert.equal(report.postId, "post.style.quadratic");
  assert.equal(report.postMs, 4);
  assert.equal(report.edgeKind, "Curve", "the pass's own edge kind, not the layout's");
  assert.equal(report.nodeKind, "Point");
  assert.equal(report.list.edgeKind, "Curve");
  assert.equal(report.list.edges.length, 1);
  assert.deepEqual([...report.list.edges[0].pts], [5, 0, 7, 0, 9, 0]);
  assert.equal(report.list.nodes[0].r, POINT_RADIUS, "the nodes are the layout's own, unmoved");
});

test("a post pass restates the columns, so edge.pts stops reading absent", () => {
  const report = withPost(baseRun(), postResult(), curveColumns(), 4);
  assert.equal(report.columns.find((column) => column.name === "edge.pts")?.length, 6, "three interior points");
  assert.equal(report.columns.find((column) => column.name === "edge.curveDegree")?.length, 1);
  assert.equal(report.columns.find((column) => column.name === "edge.offsets")?.length, 2);
});

test("a post pass takes the node count from the pass, not from the layout report", () => {
  const report = withPost({ ...baseRun(), nodeCount: 99 }, postResult({ nodeCount: 2 }), curveColumns(), 1);
  assert.equal(report.nodeCount, 2);
  assert.equal(report.edgeCount, 1, "the edge count comes from the rebuilt draw list");
});

test("the edge count is the REBUILT list's, even when it disagrees with the base report's", () => {
  // A pass redraws the layout's edges, so on real output the two always agree —
  // which is exactly why a report that copied the base's would be unobservable
  // and would go on being wrong the day a pass ever emitted a different count.
  const threeEdges: ColumnInput = {
    ...curveColumns(),
    source: u32([0, 0, 1]),
    target: u32([1, 1, 0]),
    offsets: u32([0, 3, 6, 9]),
    pts: f32([5, 0, 7, 0, 9, 0, 5, 0, 7, 0, 9, 0, 5, 0, 7, 0, 9, 0]),
  };
  const report = withPost(baseRun(), postResult(), threeEdges, 1);
  assert.equal(report.edgeCount, 3);
  assert.equal(report.list.edges.length, 3);
  assert.equal(baseRun().edgeCount, 1, "the base still has its own one edge");
});

test("a pass that stays on Line costs no offsets and no pts", () => {
  const report = withPost(baseRun(), postResult({ id: "post.style.straight", edgeKind: "Line" }), lineColumns(), 2);
  assert.equal(report.edgeKind, "Line");
  assert.equal(report.postId, "post.style.straight");
  assert.equal(report.list.edges[0].pts.length, 0);
  assert.equal(report.columns.find((column) => column.name === "edge.pts")?.length, null);
});

test("the layout's own identity and durations survive the pass", () => {
  const report = withPost(baseRun(), postResult(), curveColumns(), 4);
  assert.equal(report.layoutId, "layout.grid");
  assert.equal(report.buildMs, 3);
  assert.equal(report.layoutMs, 7);
});

test("a second pass over a first one reports only the second, off the layout's own columns", () => {
  // Each pass reads the LAYOUT's edges, so pass-then-pass must equal pass-alone.
  // Reporting the previous pass's id or its columns would be a lie about which
  // capability produced the picture on screen.
  const alone = withPost(baseRun(), postResult({ id: "post.route.grid", edgeKind: "Polyline" }), curveColumns(), 1);
  const second = withPost(withPost(baseRun(), postResult(), curveColumns(), 4), postResult({ id: "post.route.grid", edgeKind: "Polyline" }), curveColumns(), 1);
  assert.deepEqual({ ...second }, { ...alone });
  assert.equal(second.postId, "post.route.grid");
  assert.equal(second.postMs, 1, "only this pass's own duration is reported");
});
