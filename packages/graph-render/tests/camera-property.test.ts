/**
 * Property rows over seeded cameras, beside the hand-written ones in camera.test.ts.
 *
 * Ponytail: the draws come from `mulberry32` (support.ts:5) over fixed ranges — offset
 * ±100, world ±500 — and the 1e-9 tolerance in the first row holds only while
 * `|world·scale| + |offset|` stays under about `scale · 1e-9 / 2⁻⁵²` (≈ 90 at the 0.02
 * floor, ≈ 1.8e6 at ×40). The generator stays inside that, so it cannot produce the
 * cancellation that breaks the round trip; the second row widens the draw to ±1e7 and
 * asserts the bound that is exact at any magnitude instead, and it is the row to read when
 * changing the ranges. It draws no non-finite scale or factor either: nothing in the DOM
 * can hand `zoomAt` a NaN, so the clamp's NaN propagation is untested rather than covered.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import {
  type Bounds, type Camera, type Point, type ZoomLimits, DEFAULT_LIMITS, limitsFor, screenToWorld, worldToScreen, zoomAt,
} from "../src/camera.ts";
import { mulberry32 } from "./support.ts";

const SEED = 0x9e3779b9;
const DRAWS = 5000;
const VIEWPORT = { width: 1440, height: 900 };

interface Draws {
  next(): number;
  /** A real in `[min, max)`, so a point on the limit is not drawn twice. */
  between(min: number, max: number): number;
  within(limits: ZoomLimits): number;
}

function draws(seed: number): Draws {
  const next = mulberry32(seed);
  const between = (min: number, max: number): number => min + next() * (max - min);
  return { next, between, within: (limits) => limits.min + next() * (limits.max - limits.min) };
}

function cameraAt(draws_: Draws, limits: ZoomLimits, reach: number): Camera {
  return { x: draws_.between(-reach, reach), y: draws_.between(-reach, reach), scale: draws_.within(limits) };
}

function pointAt(draws_: Draws, reach: number): Point {
  return { x: draws_.between(-reach, reach), y: draws_.between(-reach, reach) };
}

function boundsAt(draws_: Draws): Bounds {
  const minX = draws_.between(-1e6, 1e6);
  const minY = draws_.between(-1e6, 1e6);
  return { minX, minY, maxX: minX + draws_.between(0, 2e6), maxY: minY + draws_.between(0, 2e6) };
}

test(`world → screen → world is the same point within 1e-9, on ${DRAWS} seeded cameras`, (t) => {
  t.diagnostic(`seed=${SEED} draws=${DRAWS} offset ±100 world ±500 scale ${DEFAULT_LIMITS.min}..${DEFAULT_LIMITS.max}`);
  const rng = draws(SEED);
  let worst = 0;
  for (let at = 0; at < DRAWS; at += 1) {
    const camera = cameraAt(rng, DEFAULT_LIMITS, 100);
    const world = pointAt(rng, 500);
    const back = screenToWorld(camera, worldToScreen(camera, world));
    const error = Math.max(Math.abs(back.x - world.x), Math.abs(back.y - world.y));
    if (error > worst) worst = error;
    assert.ok(error < 1e-9, `draw ${at} at seed ${SEED}: ${error} at ${JSON.stringify({ camera, world })}`);
  }
  t.diagnostic(`worst round-trip error ${worst}, tolerance 1e-9`);
});

/** The magnitude a round trip loses digits against: the screen number itself. */
function exactAxis(camera: Camera, world: number, offset: number, back: number): boolean {
  const screen = Math.abs(world * camera.scale) + Math.abs(offset);
  return Math.abs(back - world) <= (8 * Number.EPSILON * screen) / camera.scale + 1e-9;
}

test("the round trip is exact to a few ulps of the screen number, at any offset", (t) => {
  t.diagnostic(`seed=${SEED} draws=${DRAWS} offset ±1e7 world ±1e6 scale ${DEFAULT_LIMITS.min}..${DEFAULT_LIMITS.max}`);
  const rng = draws(SEED);
  let worst = 0;
  for (let at = 0; at < DRAWS; at += 1) {
    const camera = cameraAt(rng, DEFAULT_LIMITS, 1e7);
    const world = pointAt(rng, 1e6);
    const back = screenToWorld(camera, worldToScreen(camera, world));
    assert.ok(exactAxis(camera, world.x, camera.x, back.x) && exactAxis(camera, world.y, camera.y, back.y),
      `draw ${at} at seed ${SEED}: ${JSON.stringify({ camera, world, back })}`);
    worst = Math.max(worst, Math.abs(back.x - world.x), Math.abs(back.y - world.y));
  }
  t.diagnostic(`worst absolute round-trip error ${worst}`);
});

/** A wheel notch, a pinch, and the edges a caller can ask for. Nothing non-finite. */
const FACTORS: readonly number[] = [
  1, 1.001, 0.999, 0.1, 10, 0, -1, -0.5, 1e-9, 1e9,
  Math.exp(-120 * 0.0016), Math.exp(120 * 0.0016), Math.exp(-3 * 0.012), Math.exp(3 * 0.012),
];

function factorAt(rng: Draws): number {
  if (rng.next() < 0.35) return FACTORS[Math.floor(rng.next() * FACTORS.length)] ?? 1;
  return Math.exp(-rng.between(-1000, 1000) * (rng.next() < 0.5 ? 0.0016 : 0.012));
}

test(`a zoom lands inside the limits whatever factor is asked for, over ${DRAWS} draws`, (t) => {
  t.diagnostic(`seed=${SEED} draws=${DRAWS} limits from limitsFor and the studio's own`);
  const rng = draws(SEED);
  for (let at = 0; at < DRAWS; at += 1) {
    const limits = at % 4 === 0 ? DEFAULT_LIMITS : limitsFor(boundsAt(rng), VIEWPORT);
    const camera = cameraAt(rng, limits, 1000);
    const zoomed = zoomAt(camera, pointAt(rng, 1000), factorAt(rng), limits);
    assert.ok(limits.min > 0 && limits.max >= limits.min, `draw ${at} drew limits ${JSON.stringify(limits)}`);
    assert.ok(zoomed.scale >= limits.min && zoomed.scale <= limits.max,
      `draw ${at} at seed ${SEED}: scale ${zoomed.scale} outside ${JSON.stringify(limits)} ` +
      `from ${JSON.stringify(camera)}`);
  }
});

test("a bigger factor never gives a smaller scale", (t) => {
  t.diagnostic(`seed=${SEED} draws=${DRAWS}`);
  const rng = draws(SEED);
  for (let at = 0; at < DRAWS; at += 1) {
    const limits = limitsFor(boundsAt(rng), VIEWPORT);
    const camera = cameraAt(rng, limits, 1000);
    const cursor = pointAt(rng, 1000);
    const less = factorAt(rng);
    const more = less * 1.5;
    const low = zoomAt(camera, cursor, less, limits).scale;
    const high = zoomAt(camera, cursor, more, limits).scale;
    assert.ok(low <= high, `draw ${at} at seed ${SEED}: ${less} gave ${low} but ${more} gave ${high}`);
  }
});
