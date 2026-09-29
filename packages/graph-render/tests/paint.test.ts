import assert from "node:assert/strict";
import { test } from "node:test";

import { adjacencyOf } from "../src/adjacency.ts";
import { MOVING_BUDGET, edgeWidth } from "../src/canvas2d/edges.ts";
import type { PaintInput } from "../src/canvas2d/input.ts";
import { paintFrame } from "../src/canvas2d/paint.ts";
import type { Frame } from "../src/frame.ts";
import { newLabelPlan } from "../src/labels.ts";
import { styleFrom } from "../src/style.ts";
import { DARK_THEME } from "../src/theme.ts";
import { type Recorder, lineFrame, randomFrame, recorder } from "./support.ts";

const NO_SPRITES = { get: () => null, sphere: () => null, widthOf: () => 0, beginFrame: () => undefined, starved: () => false, reset: () => undefined };

function inputFor(frame: Frame, record: Recorder, patch: Partial<PaintInput> = {}): PaintInput {
  const colours = Uint16Array.from({ length: frame.nodeCount }, (_, i) => i % 3);
  const style = styleFrom({ labels: [], weights: new Float32Array(frame.nodeCount), colours, palette: ["red", "green", "blue"] });
  return {
    ctx: record.ctx, viewport: { width: 1200, height: 1200 }, dpr: 1, camera: { x: 0, y: 0, scale: 1 },
    theme: DARK_THEME, frame, style, adjacency: adjacencyOf(frame.nodeCount, frame), x: frame.x, y: frame.y,
    extent: style.radius, settled: true, moving: false, focus: -1, lit: new Uint8Array(frame.nodeCount),
    selected: -1, labels: newLabelPlan(8), sprites: NO_SPRITES, ...patch,
  };
}

test("6000 edges are three strokes and 2000 nodes are three fills", () => {
  const record = recorder();
  const counts = paintFrame(inputFor(randomFrame(2000, 6000, 3), record));
  assert.equal(counts.edges, 6000);
  assert.equal(counts.nodes, 2000);
  assert.equal(record.calls.get("stroke"), 3);
  assert.equal(record.calls.get("fill"), 3);
  assert.deepEqual(record.fills, ["red", "green", "blue"]);
  assert.equal(counts.draws, 6);
});

test("an edge with both ends off one side is not traced", () => {
  const record = recorder();
  const frame = lineFrame({ x: [-500, -900, 100], y: [10, 900, 100], edges: [[0, 1], [0, 2]] });
  assert.equal(paintFrame(inputFor(frame, record)).edges, 1);
});

test("a hidden node takes its edges with it", () => {
  const record = recorder();
  const frame = lineFrame({ x: [10, 50, 90], y: [10, 50, 90], edges: [[0, 1], [1, 2]] });
  const given = inputFor(frame, record);
  const counts = paintFrame({ ...given, style: { ...given.style, hidden: Uint8Array.from([1, 0, 0]) } });
  assert.deepEqual([counts.nodes, counts.edges], [2, 1]);
});

test("a quadratic curve is drawn as one, and straight while the nodes move", () => {
  const curve: Frame = {
    ...lineFrame({ x: [10, 90], y: [10, 90], edges: [[0, 1]] }),
    edgeKind: "Curve", curveDegree: 2, offsets: Uint32Array.from([0, 1]), pts: Float32Array.from([90, 10]),
  };
  const settled = recorder();
  paintFrame(inputFor(curve, settled));
  assert.equal(settled.calls.get("quadraticCurveTo"), 1);
  const moving = recorder();
  paintFrame(inputFor(curve, moving, { settled: false }));
  assert.equal(moving.calls.get("quadraticCurveTo"), undefined);
  assert.equal(moving.calls.get("lineTo"), 1);
});

test("a polyline passes through its interior points", () => {
  const routed: Frame = {
    ...lineFrame({ x: [10, 90], y: [10, 90], edges: [[0, 1]] }),
    edgeKind: "Polyline", offsets: Uint32Array.from([0, 2]), pts: Float32Array.from([10, 50, 90, 50]),
  };
  const record = recorder();
  paintFrame(inputFor(routed, record));
  assert.equal(record.calls.get("lineTo"), 3);
});

test("while moving, a frame past the budget draws a sample and says so in its count", () => {
  const frame = randomFrame(300, MOVING_BUDGET * 3, 5);
  const still = paintFrame(inputFor(frame, recorder()));
  const moving = paintFrame(inputFor(frame, recorder(), { moving: true }));
  assert.equal(still.edges, frame.edgeCount);
  assert.equal(moving.edges, MOVING_BUDGET);
});

test("a lit neighbourhood is drawn over a dimmed graph", () => {
  const record = recorder();
  const frame = lineFrame({ x: [10, 50, 90, 130], y: [10, 50, 90, 130], edges: [[0, 1], [2, 3]] });
  const counts = paintFrame(inputFor(frame, record, { focus: 0, lit: Uint8Array.from([1, 1, 0, 0]) }));
  assert.deepEqual([counts.nodes, counts.edges], [4, 2]);
  // Two edge strokes (dimmed, lit) and one ring.
  assert.equal(record.calls.get("stroke"), 3);
});

test("boxes are rects with a rim", () => {
  const record = recorder();
  const frame: Frame = {
    ...lineFrame({ x: [100, 300], y: [100, 100] }), nodeKind: "Box",
    w: Float32Array.from([80, 80]), h: Float32Array.from([40, 40]),
  };
  paintFrame(inputFor(frame, record));
  assert.equal(record.calls.get("rect"), 2);
  assert.equal(record.calls.get("arc"), undefined);
});

test("edges are one device pixel wide until the zoom is close", () => {
  assert.equal(edgeWidth(0.2, 2), 0.5);
  assert.equal(edgeWidth(0.2, 1), 1);
  assert.equal(edgeWidth(5, 2), 1.5);
});

test("a flat backdrop fills the ground with its own colour; the theme's ground is not used", () => {
  const record = recorder();
  const theme = { ...DARK_THEME, backdrop: { mode: "flat", colour: "#20232a" } } as const;
  paintFrame(inputFor(lineFrame({ x: [], y: [] }), record, { theme }));
  assert.equal(record.ctx.fillStyle, "#20232a");
  assert.equal(record.calls.get("fillRect"), 1);
  assert.equal(record.gradients.length, 0);
});

test("an aurora backdrop is one gradient across the viewport, one rect, no frame scheduled", () => {
  const record = recorder();
  const theme = { ...DARK_THEME, backdrop: { mode: "aurora", stops: ["#111111", "#222222", "#333333"] } } as const;
  paintFrame(inputFor(lineFrame({ x: [], y: [] }), record, { theme }));
  assert.deepEqual(record.gradients, [[0, 0, 1200, 1200]]);
  assert.deepEqual(record.stops, ["0:#111111", "0.5:#222222", "1:#333333"]);
  assert.equal(record.calls.get("fillRect"), 1);
});
