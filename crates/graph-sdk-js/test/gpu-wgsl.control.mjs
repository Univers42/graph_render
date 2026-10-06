// The WGSL scan's negative control. It is a *script* and not a `node:test` case because the
// gate row runs it directly: `negctl-workgroup|nonzero|… node control.mjs; test $? -ne 0`
// (scripts/orch/rows/gpu-g1b.rows:12).
//
// What it proves: **both halves of `every_kernel_is_256_wide_and_f32` can go red.** The
// control takes the kernels as they are, rewrites one entry point to `@workgroup_size(64)`
// and prepends `enable f16;` to another — in memory, on copies, so no file on disk changes —
// and asks `scanWgsl` the question the test asks. So this script exits **0 when the scan
// refuses both** and non-zero when it would not (a scan that accepts a 64-wide kernel or an
// f16 kernel would let a hardware-only path through on an arm with no `shader-f16`).
//
// Two faults, not one: a scan that checked the workgroup size and ignored `f16` would pass
// half this control, and the row's `test $? -ne 0` inverts the script's exit — so a
// half-blind scan makes the row red here and green nowhere.
//
// Run: node --experimental-strip-types crates/graph-sdk-js/test/gpu-wgsl.control.mjs

import assert from "node:assert/strict";

import { scanWgsl } from "./gpu-wgsl-scan.mjs";
import { BOUNDS_WGSL } from "../src/gpu/kernels/bounds.wgsl.ts";
import { DEPOSIT_WGSL } from "../src/gpu/kernels/deposit.wgsl.ts";
import { FFT_WGSL } from "../src/gpu/kernels/fft.wgsl.ts";
import { READ_WGSL } from "../src/gpu/kernels/read.wgsl.ts";
import { ZERO_WGSL } from "../src/gpu/kernels/zero.wgsl.ts";

/** The same five kernels the test scans, in the same order. */
const HONEST = [
  { name: "zero", code: ZERO_WGSL },
  { name: "bounds", code: BOUNDS_WGSL },
  { name: "deposit", code: DEPOSIT_WGSL },
  { name: "fft", code: FFT_WGSL },
  { name: "read", code: READ_WGSL },
];

/** The honest sources with one kernel's entry point narrowed to 64 invocations. */
const NARROWED = HONEST.map((source) =>
  source.name === "deposit"
    ? { ...source, code: source.code.replace("@workgroup_size(256)", "@workgroup_size(64)") }
    : source);

/** The honest sources with `enable f16;` in front of a second kernel. */
const HALFED = HONEST.map((source) =>
  source.name === "fft" ? { ...source, code: `enable f16;\n${source.code}` } : source);

const narrowProblems = scanWgsl(NARROWED);
const halfProblems = scanWgsl(HALFED);

assert.ok(
  narrowProblems.some((problem) => problem.startsWith("deposit:")),
  `a 64-wide entry point must be refused; the scan said ${JSON.stringify(narrowProblems)}`,
);
assert.ok(
  halfProblems.some((problem) => problem.startsWith("fft:")),
  `an f16 kernel must be refused; the scan said ${JSON.stringify(halfProblems)}`,
);
assert.deepEqual(
  scanWgsl(HONEST),
  [],
  "the honest kernels must pass, or this control proves nothing about the scan",
);

console.error(
  `negctl-workgroup: the scan refused a 64-wide entry point (${narrowProblems.length} problems) ` +
    `and an f16 kernel (${halfProblems.length} problems) — both halves of the width and ` +
    `precision rules are load-bearing`,
);
process.exit(0);