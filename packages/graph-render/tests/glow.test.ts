import assert from "node:assert/strict";
import { test } from "node:test";

import { adjacencyOf } from "../src/adjacency.ts";
import type { PaintInput } from "../src/canvas2d/input.ts";
import { paintFrame } from "../src/canvas2d/paint.ts";
import { newLabelPlan } from "../src/labels.ts";
import { styleFrom } from "../src/style.ts";
import { DARK_THEME } from "../src/theme.ts";
import { type Recorder, lineFrame, recorder } from "./support.ts";

const NO_SPRITES = { get: () => null, sphere: () => null, widthOf: () => 0, rasterised: () => 0, baked: () => 0, beginFrame: () => undefined, starved: () => false, reset: () => undefined };
const FRAME = lineFrame({ x: [60, 200, 120], y: [60, 100, 220], edges: [[0, 1]] });

function paint(glow: number | undefined): { readonly record: Recorder; readonly radii: number[]; readonly alphas: number[] } {
  const record = recorder();
  const radii: number[] = [];
  const alphas: number[] = [];
  const ctx = { ...record.ctx, arc: (_x: number, _y: number, radius: number): void => void radii.push(radius) };
  Object.defineProperty(ctx, "globalAlpha", { get: () => alphas[alphas.length - 1] ?? 1, set: (v: number) => void alphas.push(v) });
  const style = styleFrom({ labels: [], weights: new Float32Array(3), colours: new Uint16Array(3), palette: ["red"], ...(glow === undefined ? {} : { glow }) });
  const input: PaintInput = {
    ctx, viewport: { width: 300, height: 300 }, dpr: 1, camera: { x: 0, y: 0, scale: 1 }, theme: DARK_THEME,
    frame: FRAME, style, adjacency: adjacencyOf(3, FRAME), x: FRAME.x, y: FRAME.y, extent: style.radius, settled: true,
    moving: false, focus: -1, lit: new Uint8Array(3), selected: -1, labels: newLabelPlan(1), sprites: NO_SPRITES,
  };
  paintFrame(input);
  return { record, radii, alphas };
}

test("glow off draws exactly what no glow draws", () => {
  const none = paint(undefined);
  const off = paint(0);
  assert.deepEqual(off.radii, none.radii);
  assert.deepEqual(off.alphas, none.alphas);
  assert.deepEqual([...off.record.calls], [...none.record.calls]);
});

test("glow adds two translucent halos per node, wider with strength", () => {
  const none = paint(0);
  const one = paint(1);
  const two = paint(2);
  assert.equal(one.radii.length, none.radii.length + 6);
  assert.ok(Math.max(...two.radii) > Math.max(...one.radii));
  assert.ok(Math.max(...one.radii) > Math.max(...none.radii));
  assert.ok(one.alphas.some((alpha) => alpha > 0 && alpha < 0.5));
});
