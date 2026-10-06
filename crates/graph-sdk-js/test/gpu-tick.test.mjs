// The tick's host half, node-only: the pass order, the frame's place in the tick.
//
// What it pins: the dispatch order string equals graph-core's own
// (`particle_mesh.rs:177-207`), a constant compared against a constant so a reordering is a
// compile-time-visible diff; and the frame the tick uploads is the host's fold over the
// read-back positions, not the fixture's header, once the positions have moved.
//
// Each test imports its module itself, so a missing module fails that test alone.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/gpu-tick.test.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

/** The committed 1k fixture, as the bytes the page would fetch. */
function committed(state) {
  const bytes = readFileSync(new URL(`../../../fixtures/gpu/mesh-1k-${state}.gmfx`, import.meta.url));
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
}

test("the_pass_order_is_the_meshes", async () => {
  const { PASS_ORDER } = await import("../src/gpu/tick.ts");
  assert.equal(PASS_ORDER, "link,charge,centre,collide,integrate");
});

test("the_tick_reads_back_once_and_folds_the_next_frame", async () => {
  const { loadFixture } = await import("../src/gpu/fixture.ts");
  const { frameOf } = await import("../src/gpu/frame.ts");
  const { centreShift } = await import("../src/gpu/motion.ts");
  const fixture = loadFixture(committed("start"));
  // The tick's first frame is the fixture's own, from the header.
  const first = frameOf(fixture.posX, fixture.posY, fixture.side, 520);
  assert.equal(first.step, fixture.step);
  assert.equal(first.h, fixture.h);
  assert.equal(first.originX, fixture.originX);
  assert.equal(first.originY, fixture.originY);
  assert.equal(first.cells, fixture.cells);
  // The centre's shift over the f32 positions is the host's fold, bit for bit a f64 left fold
  // over the same f32 values — the f32 narrowing is the arm's, and the fold is the CPU's.
  const f32 = new Float32Array(fixture.n * 2);
  for (let i = 0; i < fixture.n; i += 1) {
    f32[2 * i] = Math.fround(fixture.posX[i] ?? 0);
    f32[2 * i + 1] = Math.fround(fixture.posY[i] ?? 0);
  }
  const shift = centreShift(f32, fixture.n);
  const reference = centreShiftOverF32(f32, fixture.n);
  assert.equal(shift.dx, reference.dx);
  assert.equal(shift.dy, reference.dy);
});

/** A reference f64 left fold over the f32 positions, for the comparison. */
function centreShiftOverF32(f32, n) {
  let sx = 0;
  let sy = 0;
  for (let i = 0; i < n; i += 1) {
    sx += f32[2 * i] ?? 0;
    sy += f32[2 * i + 1] ?? 0;
  }
  return { dx: (sx / n) * 1.0, dy: (sy / n) * 1.0 };
}
