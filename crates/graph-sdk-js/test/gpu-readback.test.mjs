// The report's arm, node-only.
//
// What it pins: a run is held to its own arm's measured ceiling, not the hardware one's. The
// ceiling table is keyed by arm, and a software adapter's reassociation and transcendentals are
// not the hardware's, so the two arms have different rows and a report that looked up the wrong
// arm would grade a software run against numbers it never produced.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/gpu-readback.test.mjs

import assert from "node:assert/strict";
import { test } from "node:test";

import { ceilingFor, expectedUnits, setCeiling } from "../src/gpu/bounds.ts";
import { report } from "../src/gpu/readback.ts";

/** A fixture whose charge deltas are a constant, so the comparator's numbers are predictable. */
function fixture(n, state) {
  const side = 128;
  return {
    n,
    state,
    side,
    h: 26.9,
    posX: new Float64Array(n),
    posY: new Float64Array(n),
    spectrumRe: new Float64Array(side * side),
    spectrumIm: new Float64Array(side * side),
    delta: { charge: { x: new Float64Array(n).fill(1), y: new Float64Array(n).fill(1) } },
  };
}

/** A run whose delta is one f32 step off the reference, and whose deposit is exact. */
function ran(n, side) {
  const density = new Int32Array(side * side);
  density[0] = expectedUnits(n);
  // One f32 ULP at 1.0 is 2⁻²³, so this is the smallest relative difference f32 can show —
  // small enough to sit under the hardware arm's measured ceiling and over a zero one.
  const delta = new Float32Array(n * 2).fill(1 + 2 ** -23);
  const extent = new Float32Array([0, 0, 0, 0]);
  return { density, delta, extent };
}

test("a_run_is_held_to_its_own_arm_ceiling", () => {
  const n = 10_000;
  const state = 0;
  const side = 128;
  // A software ceiling the hardware arm does not have, and that any nonzero measurement
  // breaches. Saved and restored so the production table is left as it was.
  const original = ceilingFor("software", n, state);
  setCeiling("software", n, state, { rmsRel: 0, maxAbs: 0 });
  try {
    const fx = fixture(n, state);
    const run = ran(n, side);
    // A software run (fallback: true) must be held to the software ceiling, which its tiny
    // but nonzero measurement breaches.
    const software = report(fx, run, run, "acme/gpu", true);
    assert.equal(
      software.pass,
      false,
      "a software run must be held to the software ceiling, not the hardware one's",
    );
    assert.ok(
      software.failures.some((f) => f.startsWith("rms ") || f.startsWith("max ")),
      `the software run must name the ceiling breach; got ${JSON.stringify(software.failures)}`,
    );
    // A hardware run (fallback: false) with the same numbers is not held to the software
    // ceiling, so it passes — the guard (rmsRel ≤ 1e-4) is the only thing above it.
    const hardware = report(fx, run, run, "acme/gpu", false);
    assert.equal(
      hardware.pass,
      true,
      "a hardware run must not be held to the software ceiling",
    );
  } finally {
    if (original) {
      setCeiling("software", n, state, original);
    }
  }
});
