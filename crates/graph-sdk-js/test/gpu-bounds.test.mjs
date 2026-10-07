// The comparator and the ceiling table, node-only.
//
// What it pins: a report that breaches one ceiling has `pass: false` and names `(n, state)`
// and the check; and every ceiling row sits under the guard that is derived independently of
// it. The second is the test that keeps the measured table honest — a table that was widened
// until the rows passed would fail here, because the `rmsRel` guard is a fixed 1e-4 and the
// 1M `maxAbs` guard is a derivation from the deposit quantum that no measurement sets.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/gpu-bounds.test.mjs

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  RMS_REL_GUARD,
  ceilingFor,
  ceilings,
  compare,
  expectedUnits,
  kernelNorm,
  maxAbsGuard,
  verdict,
} from "../src/gpu/bounds.ts";

/** A report whose numbers are all comfortably inside every bound, to breach one at a time. */
const CLEAN = {
  n: 10_000,
  state: 0,
  arm: "hardware",
  kernelNorm: 0.2,
  rmsRel: 1e-7,
  maxAbs: 1e-9,
  depositedUnits: expectedUnits(10_000),
  repeatEqual: true,
  boundsExact: true,
};

test("the_comparator_names_the_failing_case", () => {
  // A clean report passes, which is the half of the test that says the comparator is not
  // refusing everything.
  assert.deepEqual(verdict(CLEAN).failures, [], "a clean report must not name a failure");

  const ceiling = ceilingFor("hardware", 10_000, 0);
  assert.ok(ceiling, "the 10k start hardware row must have a measured ceiling");

  // One breach at a time, each naming (n, state) and its own check.
  const overRms = verdict({ ...CLEAN, rmsRel: ceiling.rmsRel * 10 });
  assert.equal(overRms.pass, false, "breaching the rmsRel ceiling must fail the case");
  assert.ok(
    overRms.failures.some((f) => f.startsWith("rms ") && f.includes("(n=10000, state=0)")),
    `the rms breach must name (n, state) and the check; got ${JSON.stringify(overRms.failures)}`,
  );

  const overMax = verdict({ ...CLEAN, maxAbs: ceiling.maxAbs * 10 });
  assert.equal(overMax.pass, false, "breaching the maxAbs ceiling must fail the case");
  assert.ok(
    overMax.failures.some((f) => f.startsWith("max ") && f.includes("(n=10000, state=0)")),
    `the max breach must name (n, state) and the check; got ${JSON.stringify(overMax.failures)}`,
  );
});

test("every_ceiling_sits_under_its_guard", () => {
  const table = ceilings();
  const rows = Object.keys(table);
  assert.ok(rows.length > 0, "the ceiling table must not be empty after a measurement run");
  for (const key of rows) {
    const row = table[key];
    assert.ok(row, `${key}: a ceiling row must exist`);
    assert.ok(
      row.rmsRel <= RMS_REL_GUARD,
      `${key}: measured rmsRel ${row.rmsRel} is over the ${RMS_REL_GUARD} guard`,
    );
    const at1m = key.includes(":1000000:");
    if (at1m) {
      // The 1M guard is per-fixture because the two 1M files have different spectra, so the
      // arm's ceiling is checked against the guard for the kernel norm it was measured at.
      // Amendment 2 derives ‖g‖₂ = P·‖spectrum‖₂ as 0.1712 start and 0.2085 settled, and the
      // guard is |charge·alpha|·(2⁻¹¹/√3)·‖g‖₂·6 — 2.61e-2 start, 3.17e-2 settled.
      const norm = key.endsWith(":0") ? 0.1712 : 0.2085;
      const guard = maxAbsGuard(norm);
      assert.ok(
        row.maxAbs <= guard,
        `${key}: measured maxAbs ${row.maxAbs} is over the derived 1M guard ${guard}`,
      );
    }
  }
});

test("the_charge_guard_reads_the_kernel_norm", () => {
  // Amendment 2's guard: |charge·alpha|·(2⁻¹¹/√3)·‖g‖₂·6, with ‖g‖₂ = P·‖spectrum‖₂ read from
  // the fixture's own spectrum section. A toy spectrum whose 2-norm is known: side 2 and
  // spectrum [3, 4, 0, 0] (imaginary zero) has ‖spectrum‖₂ = 5, so ‖g‖₂ = 2·5 = 10.
  const side = 2;
  const re = Float64Array.from([3, 4, 0, 0]);
  const im = Float64Array.from([0, 0, 0, 0]);
  const norm = kernelNorm(side, re, im);
  assert.equal(norm, 10, "the toy spectrum's kernel norm must be P times the spectrum's 2-norm");
  const guard = maxAbsGuard(norm);
  const want = 90 * (2 ** -11 / Math.sqrt(3)) * 10 * 6;
  assert.ok(
    Math.abs(guard - want) <= 1e-12 * want,
    `the guard must be the quantum times the kernel norm times the peak; got ${guard}, want ${want}`,
  );
  // The guard reads the norm: doubling the spectrum's norm doubles the guard, which is the
  // property that makes it a bound on the field the kernel actually produces.
  const doubled = kernelNorm(side, Float64Array.from([6, 8, 0, 0]), im);
  assert.ok(
    Math.abs(maxAbsGuard(doubled) / guard - 2) < 1e-12,
    "doubling the spectrum's norm must double the guard",
  );
});

test("the_exact_deposit_total_is_n_times_the_scale", () => {
  // The one check in the tier with no tolerance. scaleFor(1M) is 2^11, so the total is
  // 2 048 000 000, and it must be reached exactly — a deposit that is merely close fails.
  assert.equal(expectedUnits(1_000_000), 1_000_000 * 2048);
  assert.equal(expectedUnits(1_000), 1_000 * 2_097_152);
  const off = verdict({ ...CLEAN, depositedUnits: expectedUnits(10_000) - 1 });
  assert.ok(
    off.failures.some((f) => f.startsWith("deposit ")),
    "one unit short must name the deposit check",
  );
});

test("the_comparator_narrows_the_reference_with_fround", () => {
  // The reference arrives as f64 and the arm as f32, so the comparator is where the two meet.
  // A reference value that f32 cannot hold must be compared after narrowing, or the reported
  // rmsAbs carries an error the device never made.
  const reference = Float64Array.from([1e-30, -1e-30]);
  const got = Float32Array.from([Math.fround(1e-30), Math.fround(-1e-30)]);
  const equal = compare({ got, reference });
  assert.equal(equal.rmsAbs, 0, "an f32-narrowed reference must compare equal to the device");
  // And a one-ULP difference must show up, which is the property a narrowed reference buys.
  const off = compare({ got: Float32Array.from([Math.fround(1e-30) * 2, 0]), reference });
  assert.ok(off.maxAbs > 0, "a real difference must not be rounded away");
});

test("repeat_and_bounds_each_name_their_own_check", () => {
  // Two failures that are not numeric bounds at all, so they cannot be folded into rms/max:
  // a second run that differs and an extent that is not the host's.
  const repeat = verdict({ ...CLEAN, repeatEqual: false });
  assert.ok(
    repeat.failures.some((f) => f.startsWith("repeat ")),
    "a differing second run must name the repeat check",
  );
  const bounds = verdict({ ...CLEAN, boundsExact: false });
  assert.ok(
    bounds.failures.some((f) => f.startsWith("bounds ")),
    "an extent that is not the host's must name the bounds check",
  );
});