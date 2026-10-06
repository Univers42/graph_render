// The frame module's host half, node-only: the bounds fold, the step ladder, the origin snap.
//
// What it pins: `frameOf` reproduces graph-core's `place_over` — the same rung, the same cell
// size, the same origin, the same cell count — for every fixture the gate emits, bit for bit.
// The fixtures are graph-core's own output, so a mismatch here is a transcription error in
// frame.ts, not a difference of opinion.
//
// Each test imports its module itself, so a missing module fails that test alone.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/gpu-frame.test.mjs

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

/** The eight fixtures the gate emits, as their stem names. */
const STEMS = [
  "mesh-1k-start",
  "mesh-1k-settled",
  "mesh-10k-start",
  "mesh-10k-settled",
  "mesh-50k-start",
  "mesh-50k-settled",
  "mesh-1m-start",
  "mesh-1m-settled",
];

/** The force's reach, `distance_max` from ForceParams::default. */
const DMAX = 520;

/**
 * One fixture's bytes, or `undefined` when neither the gate's emitted copy nor the committed
 * one is present. The 1M pair is only written by a full gate run, so its absence is a skip,
 * not a failure; the committed 1k pair must be there.
 */
function fixtureBytes(stem) {
  const emitted = new URL(`../../../target/gpu-fixtures/${stem}.gmfx`, import.meta.url);
  if (existsSync(emitted)) {
    const bytes = readFileSync(emitted);
    return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
  }
  const committed = new URL(`../../../fixtures/gpu/${stem}.gmfx`, import.meta.url);
  if (existsSync(committed)) {
    const bytes = readFileSync(committed);
    return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
  }
  return undefined;
}

test("the_frame_matches_every_fixture_header", async () => {
  const { frameOf } = await import("../src/gpu/frame.ts");
  const { loadFixture } = await import("../src/gpu/fixture.ts");
  let checked = 0;
  for (const stem of STEMS) {
    const bytes = fixtureBytes(stem);
    if (bytes === undefined) {
      continue;
    }
    const fixture = loadFixture(bytes);
    const frame = frameOf(fixture.posX, fixture.posY, fixture.side, DMAX);
    assert.ok(frame !== null, `${stem}: the fixture's positions must yield a frame`);
    assert.equal(frame.step, fixture.step, `${stem}: step`);
    assert.equal(frame.h, fixture.h, `${stem}: h`);
    assert.equal(frame.originX, fixture.originX, `${stem}: originX`);
    assert.equal(frame.originY, fixture.originY, `${stem}: originY`);
    assert.equal(frame.cells, fixture.cells, `${stem}: cells`);
    checked += 1;
  }
  assert.ok(checked >= 2, `at least the committed 1k pair must be checked, got ${checked}`);
});

test("bounds_skips_non_finite_positions", async () => {
  const { bounds } = await import("../src/gpu/frame.ts");
  const posX = Float64Array.from([1, NaN, 3, Infinity, -2]);
  const posY = Float64Array.from([0, 1, -4, 2, 5]);
  const b = bounds(posX, posY);
  assert.ok(b !== null, "the finite positions must yield bounds");
  assert.equal(b.loX, -2);
  assert.equal(b.hiX, 3);
  assert.equal(b.loY, -4);
  assert.equal(b.hiY, 5);
});

test("bounds_is_null_when_no_position_is_finite", async () => {
  const { bounds } = await import("../src/gpu/frame.ts");
  const posX = Float64Array.from([NaN, Infinity]);
  const posY = Float64Array.from([1, 2]);
  assert.equal(bounds(posX, posY), null);
});

test("place_returns_null_for_a_non_finite_span", async () => {
  const { place } = await import("../src/gpu/frame.ts");
  const b = { loX: -Infinity, loY: 0, hiX: Infinity, hiY: 1 };
  assert.equal(place(b, 256, DMAX), null);
});
