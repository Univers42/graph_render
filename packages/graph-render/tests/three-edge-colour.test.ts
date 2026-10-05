/**
 * A 3D frame's edges in the gradient mode: an edge whose ends share a palette entry is
 * stroked in it, and a mixed one in the linear mean of its two, one stroke per colour. The
 * flat mode is the negative control: one stroke for the set, in the theme's edge colour.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { adjacencyOf } from "../src/adjacency.ts";
import { meanLinear } from "../src/colour/blend.ts";
import { cssOf, linearOf } from "../src/colour/srgb.ts";
import type { PaintInput } from "../src/canvas2d/input.ts";
import type { Surface2D } from "../src/canvas2d/surface.ts";
import { paintFrame } from "../src/canvas2d/paint.ts";
import type { Frame } from "../src/frame.ts";
import { newLabelPlan } from "../src/labels.ts";
import { type EdgeColour, styleFrom } from "../src/style.ts";
import { DARK_THEME } from "../src/theme.ts";
import { boxOf, fitOrbit } from "../src/three/orbit.ts";
import { newProjection, projectFrame } from "../src/three/projection.ts";
import { lineFrame, recorder } from "./support.ts";

const VIEWPORT = { width: 1200, height: 1200 };
const NO_SPRITES = {
  get: () => null, sphere: () => null, widthOf: () => 0, rasterised: () => 0, baked: () => 0,
  beginFrame: () => undefined, starved: () => false, reset: () => undefined,
};
const RED = "#ff0000";
const GREEN = "#00ff00";

/** Two red nodes, two green: edges red-red, green-green and one red-green. */
function spaceFrame(): Frame {
  return {
    ...lineFrame({ x: [-150, -50, 50, 150], y: [0, 0, 0, 0], z: [0, 40, 80, 120], edges: [[0, 1], [2, 3], [1, 2]] }),
    nodeKind: "Circle",
    r: Float32Array.from([10, 10, 10, 10]),
  };
}

/** The stroke colour in force at every `stroke()`, in call order. */
function strokesOf(frame: Frame, edgeColour: EdgeColour): string[] {
  const strokes: string[] = [];
  const ctx: Surface2D = {
    ...recorder().ctx,
    stroke: () => void strokes.push(typeof ctx.strokeStyle === "string" ? ctx.strokeStyle : "a gradient"),
  };
  const style = styleFrom({
    labels: [], weights: new Float32Array(frame.nodeCount), colours: Uint16Array.from([0, 0, 1, 1]),
    palette: [RED, GREEN], edgeColour,
  });
  const orbit = fitOrbit(boxOf(frame.x, frame.y, frame.z ?? new Float32Array(0)));
  const space = projectFrame(newProjection(frame.nodeCount), {
    frame, x: frame.x, y: frame.y, extent: frame.r ?? new Float32Array(frame.nodeCount), orbit, viewport: VIEWPORT,
  });
  const input: PaintInput = {
    ctx, viewport: VIEWPORT, dpr: 1, camera: { x: 0, y: 0, scale: 1 }, theme: DARK_THEME, frame, style,
    adjacency: adjacencyOf(frame.nodeCount, frame), x: frame.x, y: frame.y, extent: style.radius,
    settled: true, moving: false, focus: -1, lit: new Uint8Array(frame.nodeCount), selected: -1,
    labels: newLabelPlan(8), sprites: NO_SPRITES, space,
  };
  paintFrame(input);
  return strokes;
}

test("the flat mode strokes every 3D edge once, in the theme's edge colour", () => {
  assert.deepEqual(strokesOf(spaceFrame(), "flat"), [DARK_THEME.edge]);
});

test("the gradient mode strokes each 3D colour once: both palette entries, then their mean", () => {
  const mean = cssOf(meanLinear(linearOf(RED) ?? [0, 0, 0], linearOf(GREEN) ?? [0, 0, 0]));
  assert.deepEqual(strokesOf(spaceFrame(), "gradient"), [RED, GREEN, mean]);
});
