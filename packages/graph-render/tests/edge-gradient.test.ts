// The edge gradient mode: a mixed edge runs from its source's colour to its target's, the
// middle of it is the linear mean of the two, and past the budget nothing takes a gradient.
import assert from "node:assert/strict";
import { test } from "node:test";

import { adjacencyOf } from "../src/adjacency.ts";
import { MIXED_EDGE_BUDGET } from "../src/canvas2d/edgeGradient.ts";
import type { PaintInput } from "../src/canvas2d/input.ts";
import { paintFrame } from "../src/canvas2d/paint.ts";
import { GRADIENT_STOPS, meanLinear } from "../src/colour/blend.ts";
import { byteOf, linearOf } from "../src/colour/srgb.ts";
import type { Frame } from "../src/frame.ts";
import { newLabelPlan } from "../src/labels.ts";
import { type EdgeLook, styleFrom } from "../src/style.ts";
import { DARK_THEME } from "../src/theme.ts";
import { type Recorder, lineFrame, recorder } from "./support.ts";

const NO_SPRITES = { get: () => null, sphere: () => null, widthOf: () => 0, rasterised: () => 0, baked: () => 0, beginFrame: () => undefined, starved: () => false, reset: () => undefined };
const PLAIN: EdgeLook = { scale: 1, curve: false, arrows: false };
const RED = "#ff0000";
const BLUE = "#0000ff";

interface Scene {
  readonly frame: Frame;
  readonly colours: Uint16Array;
  readonly palette: readonly string[];
}

function scene(nodes: readonly (readonly [number, number])[], slots: readonly number[], edges: readonly (readonly [number, number])[], palette: readonly string[]): Scene {
  return {
    frame: lineFrame({ x: nodes.map((at) => at[0]), y: nodes.map((at) => at[1]), edges }),
    colours: Uint16Array.from(slots),
    palette,
  };
}

interface Given extends Scene {
  readonly moving?: boolean;
  readonly arrows?: boolean;
}

function inputFor(given: Given, record: Recorder): PaintInput {
  const { frame, colours, palette } = given;
  const style = styleFrom({
    labels: [], weights: new Float32Array(frame.nodeCount), colours, palette,
    edgeColour: "gradient", edges: { ...PLAIN, arrows: given.arrows === true },
  });
  return {
    ctx: record.ctx, viewport: { width: 400, height: 400 }, dpr: 1, camera: { x: 0, y: 0, scale: 1 },
    theme: DARK_THEME, frame, style, adjacency: adjacencyOf(frame.nodeCount, frame),
    x: frame.x, y: frame.y, extent: style.radius, settled: true, moving: given.moving === true, focus: -1,
    lit: new Uint8Array(frame.nodeCount), selected: -1, labels: newLabelPlan(8), sprites: NO_SPRITES,
  };
}

function paint(given: Given): { readonly counts: ReturnType<typeof paintFrame>; readonly record: Recorder } {
  const record = recorder();
  const counts = paintFrame(inputFor(given, record));
  return { counts, record };
}

function rgb(stop: string): readonly number[] {
  const found = /:rgb\((\d+), (\d+), (\d+)\)$/.exec(stop);
  assert.ok(found !== null, `${stop} is not an rgb() stop`);
  return [Number(found[1]), Number(found[2]), Number(found[3])];
}

/** The mean of the two palette entries, byte for byte: what the middle of the edge is. */
function meanBytes(palette: readonly string[]): readonly number[] {
  const a = linearOf(palette[0] ?? "");
  const b = linearOf(palette[1] ?? "");
  assert.ok(a !== null && b !== null);
  return meanLinear(a, b).map(byteOf);
}

const PAIR = scene([[50, 50], [250, 50]], [0, 1], [[0, 1]], [RED, BLUE]);

test("a two-node edge of two palette buckets takes one gradient from source to target", () => {
  const { record } = paint(PAIR);
  assert.equal(record.gradients.length, 1);
  assert.deepEqual(record.gradients[0], [50, 50, 250, 50]);
  assert.equal(record.stops.length, GRADIENT_STOPS);
});

test("the source colour near the source, the target near the target, the linear mean at the middle", () => {
  const { record } = paint(PAIR);
  const stops = record.stops.map(rgb);
  assert.deepEqual(stops[0], [255, 0, 0], "the source end wears the source node's colour");
  assert.deepEqual(stops[GRADIENT_STOPS - 1], [0, 0, 255], "the target end wears the target node's colour");
  const want = meanBytes([RED, BLUE]);
  for (let channel = 0; channel < 3; channel += 1) {
    assert.ok(
      Math.abs((stops[2]?.[channel] ?? -1) - (want[channel] ?? 0)) <= 1,
      `channel ${channel} of the middle: ${stops[2]?.[channel]} against ${want[channel]}`,
    );
  }
  assert.deepEqual(stops[2], want);
});

test("an edge whose ends share a colour is batched: no gradient, one stroke of its own", () => {
  const { counts, record } = paint(scene([[50, 50], [250, 50], [150, 200]], [0, 0, 1], [[0, 1], [1, 2]], [RED, BLUE]));
  assert.equal(record.gradients.length, 1, "only the edge onto the blue node takes a gradient");
  assert.deepEqual([counts.mixedEdges, counts.gradientStrokes], [1, 1]);
  assert.equal(counts.strokes, 2, "one stroke for the red pair, one for the gradient");
  assert.equal(typeof record.ctx.strokeStyle, "object", "the last stroke style was a gradient");
});

test("past the budget the mixed edges fall back to one flat colour, batched per pair", () => {
  const many = Array.from({ length: MIXED_EDGE_BUDGET + 1 }, () => [0, 1] as const);
  const { counts, record } = paint(scene([[50, 50], [250, 50]], [0, 1], many, [RED, BLUE]));
  assert.equal(counts.mixedEdges, MIXED_EDGE_BUDGET + 1);
  assert.equal(record.gradients.length, 0, "no gradient was asked for");
  assert.equal(counts.gradientStrokes, 0);
  assert.equal(counts.strokes, 1, `one stroke for the one colour pair, not one per edge (${counts.strokes})`);
  assert.equal(record.ctx.strokeStyle, `rgb(${meanBytes([RED, BLUE]).join(", ")})`);
});

test("the stroke count stays bounded by the budget, not by the edge count", () => {
  const many = Array.from({ length: MIXED_EDGE_BUDGET + 1 }, () => [0, 1] as const);
  const { counts } = paint(scene([[50, 50], [250, 50]], [0, 1], many, [RED, BLUE]));
  assert.ok(counts.strokes <= 1 + 1, `${counts.strokes} strokes for one pair`);
});

test("while the view moves the gradient gives up and the mixed edges take their mean", () => {
  const { counts, record } = paint({ ...PAIR, moving: true });
  assert.equal(record.gradients.length, 0);
  assert.equal(counts.gradientStrokes, 0);
  assert.equal(counts.strokes, 1);
  assert.equal(record.ctx.strokeStyle, `rgb(${meanBytes([RED, BLUE]).join(", ")})`);
});

test("at the budget exactly the gradient is still drawn", () => {
  const many = Array.from({ length: MIXED_EDGE_BUDGET }, () => [0, 1] as const);
  const { counts, record } = paint(scene([[50, 50], [250, 50]], [0, 1], many, [RED, BLUE]));
  assert.equal(record.gradients.length, MIXED_EDGE_BUDGET);
  assert.equal(counts.gradientStrokes, MIXED_EDGE_BUDGET);
  assert.equal(counts.strokes, MIXED_EDGE_BUDGET);
});

test("an arrow wears the colour of the node it points at, one fill per palette slot", () => {
  const { counts, record } = paint({ ...scene([[50, 50], [250, 50], [150, 200]], [0, 1, 1], [[0, 1], [1, 2]], [RED, BLUE]), arrows: true });
  assert.equal(counts.arrows, 2);
  assert.equal(counts.arrowFills, 1, "both heads point at the blue node, so one fill");
  assert.equal(record.fills[0], BLUE, "the head batch was filled in the target's colour");
});

test("two heads on two targets are two fills, one per colour", () => {
  const { counts, record } = paint({
    ...scene([[50, 50], [250, 50], [150, 200]], [0, 1, 0], [[0, 1], [1, 2]], [RED, BLUE]), arrows: true,
  });
  assert.equal(counts.arrowFills, 2);
  assert.deepEqual(record.fills.slice(0, 2).sort(), [BLUE, RED].sort());
});

test("the flat mode draws what it always drew and never asks for a gradient", () => {
  const record = recorder();
  const { frame, colours, palette } = PAIR;
  const style = styleFrom({ labels: [], weights: new Float32Array(frame.nodeCount), colours, palette });
  const input = inputFor(PAIR, record);
  const counts = paintFrame({ ...input, style });
  assert.equal(record.gradients.length, 0);
  assert.deepEqual([counts.strokes, counts.edgeStyles, counts.mixedEdges, counts.gradientStrokes], [1, 1, 0, 0]);
  assert.equal(record.ctx.strokeStyle, DARK_THEME.edge);
});