import assert from "node:assert/strict";
import { test } from "node:test";

import { adjacencyOf } from "../src/adjacency.ts";
import { edgeWidth } from "../src/canvas2d/edges.ts";
import type { PaintInput } from "../src/canvas2d/input.ts";
import { paintFrame } from "../src/canvas2d/paint.ts";
import type { Frame } from "../src/frame.ts";
import { newLabelPlan } from "../src/labels.ts";
import { type EdgeLook, styleFrom } from "../src/style.ts";
import { DARK_THEME } from "../src/theme.ts";
import { type Recorder, lineFrame, recorder } from "./support.ts";

const NO_SPRITES = { get: () => null, sphere: () => null, widthOf: () => 0, rasterised: () => 0, baked: () => 0, beginFrame: () => undefined, starved: () => false, reset: () => undefined };
const PLAIN: EdgeLook = { scale: 1, curve: false, arrows: false };

function inputFor(frame: Frame, record: Recorder, edges: Partial<EdgeLook> = {}): PaintInput {
  const style = styleFrom({
    labels: [], weights: new Float32Array(frame.nodeCount), colours: new Uint16Array(frame.nodeCount), palette: ["red"],
    edges: { ...PLAIN, ...edges },
  });
  return {
    ctx: record.ctx, viewport: { width: 400, height: 400 }, dpr: 1, camera: { x: 0, y: 0, scale: 1 },
    theme: DARK_THEME, frame, style, adjacency: adjacencyOf(frame.nodeCount, frame), x: frame.x, y: frame.y,
    extent: style.radius, settled: true, moving: false, focus: -1, lit: new Uint8Array(frame.nodeCount),
    selected: -1, labels: newLabelPlan(8), sprites: NO_SPRITES,
  };
}

const TRIANGLE = lineFrame({ x: [50, 250, 150], y: [50, 100, 300], edges: [[0, 1], [1, 2], [2, 0]] });

test("with arrows off no head is drawn", () => {
  const counts = paintFrame(inputFor(TRIANGLE, recorder()));
  assert.deepEqual([counts.arrows, counts.arrowSize], [0, 0]);
});

test("every edge gets one head and its size follows the thickness", () => {
  const one = paintFrame(inputFor(TRIANGLE, recorder(), { arrows: true, scale: 1 }));
  const two = paintFrame(inputFor(TRIANGLE, recorder(), { arrows: true, scale: 2 }));
  assert.equal(one.arrows, 3);
  assert.equal(two.arrows, 3);
  assert.equal(one.arrowSize, edgeWidth(1, 1) * 6);
  assert.equal(two.arrowSize, one.arrowSize * 2);
});

test("a head is a filled triangle: one fill for the whole batch", () => {
  const record = recorder();
  paintFrame(inputFor(TRIANGLE, record, { arrows: true }));
  assert.equal(record.calls.get("fill"), 2);
});

test("thickness multiplies the stroke width", () => {
  const record = recorder();
  const widths: number[] = [];
  const input = inputFor(TRIANGLE, record, { scale: 5 });
  Object.defineProperty(record.ctx, "lineWidth", { get: () => widths[widths.length - 1] ?? 1, set: (v: number) => void widths.push(v) });
  paintFrame(input);
  assert.equal(widths[0], edgeWidth(1, 1) * 5);
});

test("straight edges have no control point, curve edges have one each", () => {
  const straight = recorder();
  assert.equal(paintFrame(inputFor(TRIANGLE, straight)).curves, 0);
  assert.equal(straight.calls.get("quadraticCurveTo"), undefined);
  const bent = recorder();
  assert.equal(paintFrame(inputFor(TRIANGLE, bent, { curve: true })).curves, 3);
  assert.equal(bent.calls.get("quadraticCurveTo"), 3);
});

test("the counts carry the stroke width the edges were drawn with", () => {
  const one = paintFrame(inputFor(TRIANGLE, recorder(), { scale: 1 }));
  const five = paintFrame(inputFor(TRIANGLE, recorder(), { scale: 5 }));
  assert.equal(one.stroke, edgeWidth(1, 1));
  assert.equal(five.stroke, edgeWidth(1, 1) * 5);
});
