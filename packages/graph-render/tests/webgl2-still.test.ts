// The kept GPU picture starts again on anything it was drawn from, and never on the focus or the dim.
import assert from "node:assert/strict";
import { test } from "node:test";

import { adjacencyOf } from "../src/adjacency.ts";
import type { PaintInput } from "../src/canvas2d/input.ts";
import { newLabelPlan } from "../src/labels.ts";
import { styleFrom } from "../src/style.ts";
import { DARK_THEME } from "../src/theme.ts";
import { viewOf } from "../src/webgl2/still.ts";
import { sameRefs } from "../src/webgl2/sync.ts";
import { randomFrame, recorder } from "./support.ts";

const NO_SPRITES = { get: () => null, sphere: () => null, widthOf: () => 0, rasterised: () => 0, baked: () => 0, beginFrame: () => undefined, starved: () => false, reset: () => undefined };

function settledInput(): PaintInput {
  const frame = randomFrame(50, 120, 3);
  const style = styleFrom({ labels: [], weights: new Float32Array(frame.nodeCount), colours: new Uint16Array(frame.nodeCount), palette: ["red"] });
  return {
    ctx: recorder().ctx, viewport: { width: 800, height: 600 }, dpr: 1, camera: { x: 0, y: 0, scale: 1 },
    theme: DARK_THEME, frame, style, adjacency: adjacencyOf(frame.nodeCount, frame), x: frame.x, y: frame.y,
    extent: style.radius, settled: true, moving: false, focus: -1, lit: new Uint8Array(frame.nodeCount), selected: -1,
    labels: newLabelPlan(8), sprites: NO_SPRITES,
  };
}

test("a focus and a fade keep the picture", () => {
  const input = settledInput();
  const hovered = { ...input, focus: 7, selected: 7, theme: { ...input.theme, dimAlpha: 0.2 } };
  assert.equal(sameRefs(viewOf(input, 0), viewOf(hovered, 0)), true);
});

test("a camera step, a resize, an in-place move or a new edge colour starts it again", () => {
  const input = settledInput();
  const before = viewOf(input, 0);
  assert.equal(sameRefs(before, viewOf({ ...input, camera: { ...input.camera, x: 1 } }, 0)), false);
  assert.equal(sameRefs(before, viewOf({ ...input, viewport: { width: 801, height: 600 } }, 0)), false);
  assert.equal(sameRefs(before, viewOf(input, 1)), false);
  assert.equal(sameRefs(before, viewOf({ ...input, theme: { ...input.theme, edge: "#123456" } }, 0)), false);
});
