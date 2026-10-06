/**
 * A routed edge mid-transition (canvas2d/morph.ts): bent part of the way between its old and
 * new route instead of drawn straight until the nodes arrive. One edge from FROM to TO with
 * one interior point, so every expected point is plain arithmetic on the chord.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { adjacencyOf } from "../src/adjacency.ts";
import { paintEdges } from "../src/canvas2d/edges.ts";
import { newCounts, type PaintInput } from "../src/canvas2d/input.ts";
import { type EdgeTween, morphSource, sameEdges } from "../src/canvas2d/morph.ts";
import type { EdgeKind } from "../src/snapshot/decode.ts";
import type { Frame } from "../src/frame.ts";
import { newLabelPlan } from "../src/labels.ts";
import { styleFrom } from "../src/style.ts";
import { DARK_THEME } from "../src/theme.ts";
import { type Recorder, recorder } from "./support.ts";

const NO_SPRITES = { get: () => null, sphere: () => null, widthOf: () => 0, rasterised: () => 0, baked: () => 0, beginFrame: () => undefined, starved: () => false, reset: () => undefined };
/** Camera is the identity and the viewport holds every point below, so screen = world. */
const FROM = { x: 20, y: 20 };
const TO = { x: 320, y: 20 };
const DOWN: readonly [number, number] = [170, 120];
const UP: readonly [number, number] = [170, -80];

/** One edge FROM → TO, routed through `interior` unless the kind is Line. */
function edgeFrame(kind: EdgeKind, interior: readonly (readonly [number, number])[] = []): Frame {
  const routed = kind !== "Line";
  return {
    nodeKind: "Point", edgeKind: kind, nodeCount: 2, edgeCount: 1,
    x: Float32Array.from([FROM.x, TO.x]), y: Float32Array.from([FROM.y, TO.y]),
    z: null, r: null, w: null, h: null,
    source: Uint32Array.from([0]), target: Uint32Array.from([1]),
    curveDegree: kind === "Curve" ? interior.length + 1 : 0,
    offsets: routed ? Uint32Array.from([0, interior.length]) : null,
    pts: routed ? Float32Array.from(interior.flat()) : null,
    bounds: { minX: 20, minY: -80, maxX: 320, maxY: 120 }, factor: 1,
  };
}

interface Scenario {
  readonly frame: Frame;
  readonly tween: EdgeTween | null;
  readonly curve?: boolean;
  /** Where the nodes are on this frame, when not at the frame's own positions. */
  readonly at?: { readonly x: readonly number[]; readonly y: readonly number[] };
}

function inputFor(scenario: Scenario, record: Recorder): PaintInput {
  const { frame } = scenario;
  const style = styleFrom({
    labels: [], weights: new Float32Array(frame.nodeCount), colours: new Uint16Array(frame.nodeCount),
    palette: ["red"], edges: { scale: 1, curve: scenario.curve ?? false, arrows: false },
  });
  return {
    ctx: record.ctx, viewport: { width: 800, height: 800 }, dpr: 1, camera: { x: 0, y: 0, scale: 1 },
    theme: DARK_THEME, frame, style, adjacency: adjacencyOf(frame.nodeCount, frame),
    x: scenario.at === undefined ? frame.x : Float32Array.from(scenario.at.x),
    y: scenario.at === undefined ? frame.y : Float32Array.from(scenario.at.y),
    extent: style.radius, settled: false, tween: scenario.tween, moving: false, focus: -1,
    lit: new Uint8Array(frame.nodeCount), selected: -1, labels: newLabelPlan(8), sprites: NO_SPRITES,
  };
}

/** Every lineTo the edge pass issued, and the recorder's own counters. */
function painted(scenario: Scenario): { readonly lines: number[][]; readonly record: Recorder } {
  const record = recorder();
  const lines: number[][] = [];
  const lineTo = record.ctx.lineTo.bind(record.ctx);
  record.ctx.lineTo = (x: number, y: number): void => {
    lines.push([x, y]);
    lineTo(x, y);
  };
  paintEdges(inputFor(scenario, record), newCounts());
  return { lines, record };
}

function assertPoints(actual: readonly number[][], expected: readonly (readonly number[])[]): void {
  assert.equal(actual.length, expected.length, `points ${JSON.stringify(actual)}`);
  expected.forEach((point, at) => {
    assert.ok(Math.abs((actual[at]?.[0] ?? NaN) - (point[0] ?? 0)) < 1e-3, `x of point ${at}: ${JSON.stringify(actual)}`);
    assert.ok(Math.abs((actual[at]?.[1] ?? NaN) - (point[1] ?? 0)) < 1e-3, `y of point ${at}: ${JSON.stringify(actual)}`);
  });
}

const END = [TO.x, TO.y];

test("the old route straightens: at rest at the start, half bent halfway, straight on arrival", () => {
  const from = edgeFrame("Polyline", [DOWN]);
  const frame = edgeFrame("Line");
  assertPoints(painted({ frame, tween: { from, eased: 0 } }).lines, [DOWN, END]);
  assertPoints(painted({ frame, tween: { from, eased: 0.5 } }).lines, [[170, 70], END]);
  assertPoints(painted({ frame, tween: { from, eased: 1 } }).lines, [[170, 20], END]);
});

test("a new route grows out of the straight line when the old frame had none", () => {
  const frame = edgeFrame("Polyline", [DOWN]);
  assertPoints(painted({ frame, tween: { from: null, eased: 0 } }).lines, [[170, 20], END]);
  assertPoints(painted({ frame, tween: { from: null, eased: 1 } }).lines, [DOWN, END]);
});

test("between two routes the old one flattens over the first half and the new one grows over the second", () => {
  const from = edgeFrame("Polyline", [DOWN]);
  const frame = edgeFrame("Polyline", [UP]);
  assertPoints(painted({ frame, tween: { from, eased: 0.25 } }).lines, [[170, 70], END]);
  assertPoints(painted({ frame, tween: { from, eased: 0.75 } }).lines, [[170, -30], END]);
});

test("the route follows the chord the edge has now, not the one it was laid out on", () => {
  const from = edgeFrame("Polyline", [DOWN]);
  const at = { x: [20, 620], y: [20, 20] };
  // Twice the chord: the interior point is twice as far along and twice as far off it.
  assertPoints(painted({ frame: edgeFrame("Line"), tween: { from, eased: 0 }, at }).lines, [[320, 220], [620, 20]]);
});

test("a curve keeps its canvas call mid-move", () => {
  const frame = edgeFrame("Curve", [DOWN]);
  const { record } = painted({ frame, tween: { from: null, eased: 0.5 } });
  assert.equal(record.calls.get("quadraticCurveTo"), 1);
});

test("an unsettled frame without a tween draws the routed edge straight", () => {
  assertPoints(painted({ frame: edgeFrame("Polyline", [DOWN]), tween: null }).lines, [END]);
});

test("the curve style bends a Line frame while the layout is still settling", () => {
  const { record, lines } = painted({ frame: edgeFrame("Line"), tween: { from: null, eased: 0.5 }, curve: true });
  assert.equal(record.calls.get("quadraticCurveTo"), 1);
  assert.equal(lines.length, 0);
});

test("only a routed old frame with the same edges is morphed from", () => {
  const routed = edgeFrame("Polyline", [DOWN]);
  const next = edgeFrame("Line");
  assert.ok(sameEdges(routed, next));
  assert.equal(morphSource(routed, next), routed);
  assert.equal(morphSource(edgeFrame("Line"), next), null);
  const reversed = { ...next, source: Uint32Array.from([1]), target: Uint32Array.from([0]) };
  assert.ok(!sameEdges(routed, reversed));
  assert.equal(morphSource(routed, reversed), null);
  assert.equal(morphSource(routed, { ...next, edgeCount: 0 }), null);
});
