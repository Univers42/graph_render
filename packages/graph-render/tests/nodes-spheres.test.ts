/**
 * Nodes drawn as lit spheres instead of flat discs, and the edge width a look carries.
 * The impostor path is one blit per node, so it is only taken for a scene small enough
 * that a frame's allowance covers it; past that the batched fills are the drawing.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { adjacencyOf } from "../src/adjacency.ts";
import type { PaintInput } from "../src/canvas2d/input.ts";
import type { Surface2D } from "../src/canvas2d/surface.ts";
import { paintFrame } from "../src/canvas2d/paint.ts";
import type { SpriteCache } from "../src/canvas2d/sprites.ts";
import { styleFrom } from "../src/style.ts";
import { DARK_THEME } from "../src/theme.ts";
import type { Rgb } from "../src/colour/srgb.ts";
import { cssOf } from "../src/colour/srgb.ts";
import { sampleColormap } from "../src/colour/colormap.ts";
import { type Recorder, lineFrame, randomFrame, recorder } from "./support.ts";

const FIRST = sampleColormap("inferno", 0);
const SECOND = sampleColormap("inferno", 1);
const BASES: readonly Rgb[] = [FIRST, SECOND];
const ASKED: { base: Rgb; size: number }[] = [];
const SPHERE = { image: { name: "sphere" }, width: 1, height: 1 };

/** A cache that answers every sphere and records what it was asked for. */
function spheres(): SpriteCache {
  return {
    get: () => null,
    sphere: (base, size) => {
      ASKED.push({ base, size });
      return SPHERE;
    },
    widthOf: () => 0,
    beginFrame: () => undefined,
    starved: () => false,
    reset: () => undefined,
  };
}

function inputFor(frame: ReturnType<typeof lineFrame>, record: Recorder, patch: Partial<PaintInput> = {}): PaintInput {
  const count = frame.nodeCount;
  const style = styleFrom({
    labels: [], weights: new Float32Array(count),
    colours: Uint16Array.from({ length: count }, (_, i) => i % 2),
    palette: [cssOf(FIRST), cssOf(SECOND)],
    spheres: BASES,
  });
  return {
    ctx: record.ctx, viewport: { width: 1200, height: 1200 }, dpr: 2, camera: { x: 0, y: 0, scale: 1 },
    theme: DARK_THEME, frame, style, adjacency: adjacencyOf(count, frame), x: frame.x, y: frame.y,
    extent: Float32Array.from({ length: count }, () => 4), settled: true, moving: false, focus: -1,
    lit: new Uint8Array(count), selected: -1, labels: { count: 0, node: new Uint32Array(0), alpha: new Float32Array(0), x: new Float32Array(0), y: new Float32Array(0) },
    sprites: spheres(), ...patch,
  };
}

test("a scene with sphere bases is drawn one blit per node, not one fill per bucket", () => {
  ASKED.length = 0;
  const record = recorder();
  const counts = paintFrame(inputFor(lineFrame({ x: [10, 90, 170], y: [10, 90, 170] }), record));
  assert.equal(counts.nodes, 3);
  assert.equal(record.calls.get("drawImage"), 3);
  assert.equal(record.calls.get("fill") ?? 0, 0);
  // Radius 4 at scale 1 and dpr 2 is a 16 px sprite, and the bases follow the buckets.
  assert.deepEqual(ASKED.map((ask) => ask.size), [16, 16, 16]);
  assert.deepEqual(ASKED.map((ask) => cssOf(ask.base)), [cssOf(FIRST), cssOf(SECOND), cssOf(FIRST)]);
});

test("a sprite the cache refuses is a node that is not drawn", () => {
  ASKED.length = 0;
  const given = inputFor(lineFrame({ x: [10, 90], y: [10, 90] }), recorder());
  const empty: SpriteCache = { ...given.sprites, sphere: () => null };
  const counts = paintFrame({ ...given, sprites: empty });
  assert.equal(counts.nodes, 0);
  assert.equal(counts.draws, 0);
});

test("a hidden node takes no sphere, and a lit neighbourhood splits them in two passes", () => {
  const frame = lineFrame({ x: [10, 90, 170], y: [10, 90, 170], edges: [[0, 1]] });
  const hidden = paintFrame(inputFor(frame, recorder(), {
    style: { ...styleFrom({ labels: [], weights: new Float32Array(3), colours: new Uint16Array(3), palette: ["a", "b"], spheres: BASES }), hidden: Uint8Array.from([1, 0, 0]) },
  }));
  assert.equal(hidden.nodes, 2);
  const lit = inputFor(frame, recorder(), { focus: 0, lit: Uint8Array.from([1, 1, 0]) });
  assert.equal(paintFrame(lit).nodes, 3);
});

test("a lit neighbourhood dims the rest of a sphere scene, as it does the flat one", () => {
  const frame = lineFrame({ x: [10, 90, 170], y: [10, 90, 170], edges: [[0, 1]] });
  const given = inputFor(frame, recorder(), { focus: 0, lit: Uint8Array.from([1, 1, 0]) });
  const record = recorder();
  const alphas: number[] = [];
  const ctx = { ...record.ctx };
  ctx.drawImage = (...args: Parameters<Surface2D["drawImage"]>): void => {
    alphas.push(ctx.globalAlpha);
    record.ctx.drawImage(...args);
  };
  const counts = paintFrame({ ...given, ctx });
  // The dim pass first, then the lit one: the unlit node dimmed, the lit two at full alpha.
  assert.equal(counts.nodes, 3);
  assert.deepEqual(alphas, [DARK_THEME.dimAlpha, 1, 1]);
});

test("a style with no sphere bases is drawn as flat fills, as the studio always was", () => {
  const frame = lineFrame({ x: [10, 90], y: [10, 90] });
  const given = inputFor(frame, recorder());
  const flat = { ...given.style, spheres: null };
  const counts = paintFrame({ ...given, style: flat });
  assert.equal(counts.nodes, 2);
  assert.equal(recorder().calls.size, 0);
  assert.deepEqual([...flat.colours], [0, 1]);
});

test("past the impostor budget the batched fills take over again", () => {
  const frame = randomFrame(4097, 0, 5);
  const record = recorder();
  const counts = paintFrame(inputFor(frame, record));
  assert.equal(counts.nodes, 4097);
  // Two buckets, two fills, and no blit at all.
  assert.equal(record.calls.get("fill"), 2);
  assert.equal(record.calls.get("drawImage") ?? 0, 0);
});

test("a look's edge width is in world units and overrides the zoom-driven one", () => {
  const frame = lineFrame({ x: [10, 90], y: [10, 90], edges: [[0, 1]] });
  const given = inputFor(frame, recorder());
  const style = { ...given.style, edgeWidth: 9 };
  const record = recorder();
  const counts = paintFrame({ ...given, style, ctx: record.ctx });
  assert.equal(counts.edges, 1);
  assert.equal(record.ctx.lineWidth, 9);
  // Without one, the width is the zoom-driven default of edges.ts:17-20.
  const plain = recorder();
  paintFrame({ ...given, ctx: plain.ctx });
  assert.equal(plain.ctx.lineWidth, 0.6);
});
