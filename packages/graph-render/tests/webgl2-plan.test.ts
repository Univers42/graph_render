/** The pure half of the GPU layer: the backend choice, the arrays it uploads, its colours, and the paint hook. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { adjacencyOf } from "../src/adjacency.ts";
import type { PaintInput } from "../src/canvas2d/input.ts";
import { paintFrame } from "../src/canvas2d/paint.ts";
import { newLabelPlan } from "../src/labels.ts";
import { styleFrom } from "../src/style.ts";
import { DARK_THEME } from "../src/theme.ts";
import { bytesOf, normaliserOf, paletteTexels, rgbaOf } from "../src/webgl2/colour.ts";
import { BULK_THRESHOLD, backendOf, bulkWanted, nodeHalves, spreadShown, visibleEdges } from "../src/webgl2/plan.ts";
import { randomFrame, recorder } from "./support.ts";

test("a backend name parses, and anything else is auto", () => {
  assert.deepEqual(["canvas2d", "webgl2", "auto", "gl", null, undefined].map(backendOf), ["canvas2d", "webgl2", "auto", "auto", "auto", "auto"]);
});

test("auto takes the GPU from the threshold up, the two forced choices ignore it, and no GPU means 2D", () => {
  assert.equal(bulkWanted("auto", BULK_THRESHOLD - 1, true), false);
  assert.equal(bulkWanted("auto", BULK_THRESHOLD, true), true);
  assert.equal(bulkWanted("webgl2", 3, true), true);
  assert.equal(bulkWanted("canvas2d", 10 * BULK_THRESHOLD, true), false);
  assert.equal(bulkWanted("webgl2", 10 * BULK_THRESHOLD, false), false);
});

test("an edge touching a hidden node leaves the element list, the rest keep their order", () => {
  const source = Uint32Array.of(0, 1, 2, 3);
  const target = Uint32Array.of(1, 2, 3, 0);
  assert.deepEqual([...visibleEdges(source, target, null)], [0, 1, 1, 2, 2, 3, 3, 0]);
  assert.deepEqual([...visibleEdges(source, target, Uint8Array.of(0, 0, 1, 0))], [0, 1, 3, 0]);
});

test("a disc is its radius on both axes, a box its half sides, and a hidden node -1", () => {
  const extent = Float32Array.of(2, 3, 4);
  const hidden = Uint8Array.of(0, 1, 0);
  assert.deepEqual([...nodeHalves({ extent, w: null, h: null, hidden })], [2, 2, -1, -1, 4, 4]);
  const boxes = nodeHalves({ extent, w: Float32Array.of(10, 6, 2), h: Float32Array.of(4, 6, 8), hidden: null });
  assert.deepEqual([...boxes], [5, 2, 3, 3, 1, 4]);
  assert.equal(spreadShown(nodeHalves({ extent, w: null, h: null, hidden })).length, 2);
});

test("the two normalised forms read as bytes, and anything else is null", () => {
  assert.deepEqual(rgbaOf("#1b1b1f"), [0x1b, 0x1b, 0x1f, 255]);
  assert.deepEqual(rgbaOf("rgba(150, 152, 165, 0.34)"), [150, 152, 165, 87]);
  assert.deepEqual(rgbaOf("rgb(1, 2, 3)"), [1, 2, 3, 255]);
  assert.equal(rgbaOf("rgba(1, 2, 3, 1.5)"), null);
  assert.equal(rgbaOf("red"), null);
});

test("a colour the normaliser cannot read falls back to the 2D painter's grey, one texel per entry", () => {
  const seen: { fillStyle: unknown } = { fillStyle: "" };
  const normalise = normaliserOf({
    get fillStyle(): unknown { return seen.fillStyle; },
    set fillStyle(value: unknown) { if (value !== "nonsense") seen.fillStyle = value; },
  });
  assert.deepEqual(bytesOf("#ff0000", normalise), [255, 0, 0, 255]);
  assert.deepEqual(bytesOf("nonsense", normalise), [0, 0, 0, 255]);
  assert.deepEqual([...paletteTexels(["#010203", "red"], normalise)], [1, 2, 3, 255, 0x9a, 0x9a, 0x9a, 255]);
  assert.equal(paletteTexels([], normalise).length, 4);
});

const NO_SPRITES = { get: () => null, sphere: () => null, widthOf: () => 0, rasterised: () => 0, baked: () => 0, beginFrame: () => undefined, starved: () => false, reset: () => undefined };

function inputFor(bulk: PaintInput["bulk"], focus: number): { input: PaintInput; record: ReturnType<typeof recorder> } {
  const frame = randomFrame(400, 1200, 5);
  const record = recorder();
  const colours = Uint16Array.from({ length: frame.nodeCount }, (_, i) => i % 3);
  const style = styleFrom({ labels: [], weights: new Float32Array(frame.nodeCount), colours, palette: ["red", "green", "blue"] });
  const adjacency = adjacencyOf(frame.nodeCount, frame);
  const lit = new Uint8Array(frame.nodeCount);
  if (focus >= 0) lit[focus] = 1;
  const input: PaintInput = {
    ctx: record.ctx, viewport: { width: 1200, height: 1200 }, dpr: 1, camera: { x: 0, y: 0, scale: 1 },
    theme: DARK_THEME, frame, style, adjacency, x: frame.x, y: frame.y, extent: style.radius, settled: true,
    moving: false, focus, lit, selected: focus, labels: newLabelPlan(8), sprites: NO_SPRITES, ...(bulk === undefined ? {} : { bulk }),
  };
  return { input, record };
}

test("a frame the GPU layer drew issues no 2D edge stroke and no node fill of its own", () => {
  const { input, record } = inputFor((_, counts) => {
    counts.nodes = 400;
    counts.bulk = 2;
    return true;
  }, -1);
  const counts = paintFrame(input);
  assert.equal(record.calls.get("fill") ?? 0, 0);
  assert.equal(record.calls.get("stroke") ?? 0, 0);
  assert.deepEqual([counts.nodes, counts.bulk], [400, 2]);
});

test("under a focus the GPU frame gets the lit edges, the lit node and its ring on top, counted once", () => {
  const { input, record } = inputFor((_, counts) => {
    counts.nodes = 400;
    return true;
  }, 7);
  const counts = paintFrame(input);
  assert.ok((record.calls.get("stroke") ?? 0) >= 2, "the lit edges and the ring are stroked");
  assert.equal(record.calls.get("fill"), 1);
  assert.equal(counts.nodes, 400);
});

test("a hook that declines leaves the frame to the 2D passes", () => {
  const declined = inputFor(() => false, -1);
  const absent = inputFor(undefined, -1);
  assert.deepEqual(paintFrame(declined.input), paintFrame(absent.input));
  assert.deepEqual([...declined.record.calls], [...absent.record.calls]);
});
