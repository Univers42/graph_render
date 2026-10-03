/** The edge sample: how much of the edge set covers a screen, and how much a moving frame then draws. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { type EdgeShape, compensate, measurePairs, sampleStep } from "../src/webgl2/sample.ts";

/** The 1M-node layout.random at fit: sides of 56 000 world units (TARGET_SPACING), scale 0.017. */
const FIT_1M: EdgeShape = { mean: 29200, spanX: 56000, spanY: 56000 };
const FIT_1M_AT = { scale: 0.017, width: 1920, height: 1080, dpr: 1 };

test("measurePairs reads the mean chord and the bounding span of the drawn pairs", () => {
  // Two pairs: (0,0)-(3,4) is 5 long, (1,1)-(1,7) is 6 long; mean 5.5, spans 0..3 and 0..7.
  const shape = measurePairs(
    new Float32Array([0, 1, 3, 1]),
    new Float32Array([0, 1, 4, 7]),
    new Uint32Array([0, 2, 1, 3]),
  );
  assert.equal(shape.mean, 5.5);
  assert.equal(shape.spanX, 3);
  assert.equal(shape.spanY, 7);
});

test("a parity-sized graph keeps the full draw: its few chords cannot cover a screen", () => {
  // 2000 nodes, ~4000 edges of ~400 screen px over a 1400x900 viewport: about 1.3 coverings.
  const shape: EdgeShape = { mean: 1306, spanX: 2504, spanY: 2504 };
  assert.equal(sampleStep(shape, 4000, { scale: 0.308, width: 1400, height: 900, dpr: 1 }), 1);
});

test("a graph covering the screen many times over draws a spread sample of its edges", () => {
  // 1M at fit: 2M chords of ~496 px over 1920x1080 is about 479 coverings, drawn one in eight.
  assert.equal(sampleStep(FIT_1M, 2_000_000, FIT_1M_AT), 8);
  // 200k at fit: 400k chords of the same 496 px is about 96 coverings, drawn one in three.
  const small: EdgeShape = { mean: 13058, spanX: 25044, spanY: 25044 };
  assert.equal(sampleStep(small, 400_000, { scale: 0.038, width: 1920, height: 1080, dpr: 1 }), 3);
});

test("zoomed in, the graph is larger than the viewport and the estimate is not trusted", () => {
  // The same 1M graph at x40 covers the screen's area a thousand times over, but almost every
  // chord is off screen, so the raw ratio would say eight and thin a sparse view.
  assert.equal(sampleStep(FIT_1M, 2_000_000, { ...FIT_1M_AT, scale: 0.68 }), 1);
});

test("fewer device pixels are fewer coverings, and no step is ever above eight", () => {
  // 250k chords at fit: 60 coverings at DPR 1 (a step of two), 30 at DPR 2 (a step of one).
  assert.equal(sampleStep(FIT_1M, 250_000, FIT_1M_AT), 2);
  assert.equal(sampleStep(FIT_1M, 250_000, { ...FIT_1M_AT, dpr: 2 }), 1);
  // A dense view is never cut finer than one edge in eight.
  const dense: EdgeShape = { mean: 100, spanX: 180, spanY: 180 };
  assert.equal(sampleStep(dense, 1_000_000, { scale: 1, width: 64, height: 64, dpr: 1 }), 8);
});

test("nothing measured, and nothing drawn, are both the full draw", () => {
  assert.equal(sampleStep(null, 2_000_000, FIT_1M_AT), 1);
  assert.equal(sampleStep(FIT_1M, 0, FIT_1M_AT), 1);
});

test("compensate scales u_alpha to the coverage of step stacked lines", () => {
  assert.equal(compensate(0.34, 1), 1);
  assert.equal(compensate(1, 8), 1);
  assert.equal(compensate(0, 8), 1);
  // Two lines of 0.5 read as 0.75 of the canvas showing through, one line of compensate(0.5, 2).
  assert.equal(0.5 * compensate(0.5, 2), 0.75);
});
