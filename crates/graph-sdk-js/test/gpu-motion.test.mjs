// The motion pass's host half: the centre's fold, node-only.
//
// What it pins: `centreShift` is bit-for-bit the CPU's `center_shift`
// (`barnes_hut/sim.rs:225-237`) over the same positions — a f64 left fold in node order,
// then `(mean) * strength`. The fold's order is the whole test: a f32 accumulation, a
// pairwise sum, or a reversed order would all pass a tolerance and fail these bytes. The
// f32 positions widen to f64 exactly, so the fold is over the same numbers the CPU folds.
//
// Each test imports its module itself, so a missing module fails that test alone.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/gpu-motion.test.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

/** The committed 1k fixture, as the bytes the page would fetch. */
function committed(state) {
  const bytes = readFileSync(new URL(`../../../fixtures/gpu/mesh-1k-${state}.gmfx`, import.meta.url));
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
}

/** A seeded LCG: deterministic, and `Math.random` is banned by the determinism rules. */
function synthetic(n) {
  const positions = new Float32Array(n * 2);
  let state = 0x2f6e2b1;
  for (let i = 0; i < n; i += 1) {
    state = (state * 1664525 + 1013904223) >>> 0;
    positions[2 * i] = (state / 0xffffffff) * 2000 - 1000;
    state = (state * 1664525 + 1013904223) >>> 0;
    positions[2 * i + 1] = (state / 0xffffffff) * 2000 - 1000;
  }
  return positions;
}

/** The CPU's fold over interleaved f32 positions, verbatim: f64, node order (`sim.rs:230-236`). */
function referenceFold32(positions, n, strength) {
  let sx = 0;
  let sy = 0;
  for (let i = 0; i < n; i += 1) {
    sx += positions[2 * i];
    sy += positions[2 * i + 1];
  }
  return { dx: (sx / n) * strength, dy: (sy / n) * strength };
}

/** The same fold over separate f64 position columns. */
function referenceFold64(posX, posY, n, strength) {
  let sx = 0;
  let sy = 0;
  for (let i = 0; i < n; i += 1) {
    sx += posX[i];
    sy += posY[i];
  }
  return { dx: (sx / n) * strength, dy: (sy / n) * strength };
}

test("the_centre_shift_is_the_cpus_fold", async () => {
  const { CENTER_STRENGTH, centreShift, centreShiftF64 } = await import("../src/gpu/motion.ts");
  const { loadFixture } = await import("../src/gpu/fixture.ts");

  // 1. Ten thousand synthetic positions, narrowed to f32 once by the array's own store.
  const n = 10000;
  const positions = synthetic(n);
  const got = centreShift(positions, n);
  const want = referenceFold32(positions, n, CENTER_STRENGTH);
  assert.equal(got.dx, want.dx, "dx over the synthetic 10k");
  assert.equal(got.dy, want.dy, "dy over the synthetic 10k");

  // 2. The committed 1k fixture, its f64 positions narrowed to f32 the way the upload does.
  const fixture = loadFixture(committed("start"));
  const f32 = new Float32Array(fixture.n * 2);
  for (let i = 0; i < fixture.n; i += 1) {
    f32[2 * i] = Math.fround(fixture.posX[i]);
    f32[2 * i + 1] = Math.fround(fixture.posY[i]);
  }
  const gotF32 = centreShift(f32, fixture.n);
  const wantF32 = referenceFold32(f32, fixture.n, CENTER_STRENGTH);
  assert.equal(gotF32.dx, wantF32.dx, "dx over the fixture's f32 positions");
  assert.equal(gotF32.dy, wantF32.dy, "dy over the fixture's f32 positions");

  // 3. The same fold over the fixture's own f64 positions, un-narrowed.
  const got64 = centreShiftF64(fixture.posX, fixture.posY, fixture.n);
  const want64 = referenceFold64(fixture.posX, fixture.posY, fixture.n, CENTER_STRENGTH);
  assert.equal(got64.dx, want64.dx, "dx over the fixture's f64 positions");
  assert.equal(got64.dy, want64.dy, "dy over the fixture's f64 positions");
});
