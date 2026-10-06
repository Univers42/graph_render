// The WGSL's own invariants, node-only and with no browser.
//
// What it pins: every `@compute` entry point in `src/gpu/kernels/*.wgsl.ts` is
// `@workgroup_size(256)` and no kernel source mentions `f16`. Both are properties of the
// source text, so neither needs a device; that is why this is a gate row rather than part of
// the browser arm (`scripts/orch/rows/gpu-g1b.rows`, `negctl-workgroup`).
//
// The two rules and the numbers behind them are in `gpu-wgsl-scan.mjs`, which
// `gpu-wgsl.control.mjs` reuses so the negative control asks this exact question of a
// deliberately broken copy.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/gpu-wgsl.test.mjs

import assert from "node:assert/strict";
import { test } from "node:test";

import { scanWgsl } from "./gpu-wgsl-scan.mjs";
import { BOUNDS_WGSL } from "../src/gpu/kernels/bounds.wgsl.ts";
import { COLLIDE_HASH_WGSL } from "../src/gpu/kernels/collide-hash.wgsl.ts";
import { DEPOSIT_WGSL } from "../src/gpu/kernels/deposit.wgsl.ts";
import { FFT_WGSL } from "../src/gpu/kernels/fft.wgsl.ts";
import { READ_WGSL } from "../src/gpu/kernels/read.wgsl.ts";
import { ZERO_WGSL } from "../src/gpu/kernels/zero.wgsl.ts";

/** Every kernel module, in the order `mesh.rs` runs them. */
const SOURCES = [
  { name: "zero", code: ZERO_WGSL },
  { name: "bounds", code: BOUNDS_WGSL },
  { name: "deposit", code: DEPOSIT_WGSL },
  { name: "fft", code: FFT_WGSL },
  { name: "read", code: READ_WGSL },
];

/** The collide pass's modules, one per stage of `collide.rs`; scanned with the charge pass's. */
const COLLIDE_SOURCES = [
  { name: "collide-hash", code: COLLIDE_HASH_WGSL },
];

test("every_kernel_is_256_wide_and_f32", () => {
  const problems = scanWgsl([...SOURCES, ...COLLIDE_SOURCES]);
  assert.deepEqual(problems, [], problems.join("; "));
});

test("every_stage_of_the_charge_pass_has_a_kernel", () => {
  // The five names are the plan's, one per stage `mesh.rs:151-185` runs: the zeroing, the
  // bounds fold, the deposit, the transforms and the field read. A stage with no kernel is
  // the bug the plan calls "the omission that reads last year's density" (plan :1102), and
  // a scan over a shorter list would not notice it.
  assert.deepEqual(
    SOURCES.map((source) => source.name),
    ["zero", "bounds", "deposit", "fft", "read"],
  );
});