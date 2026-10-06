/**
 * `deposit.wgsl.ts` — one unit charge per node into the fixed-point density, four
 * `atomicAdd<i32>` each, and the widening back to `f32` the transforms read.
 *
 * ## What is transcribed, and from where
 *
 * | this kernel | the CPU |
 * |---|---|
 * | `cell`, a truncating cast and a `min` against `cells − 2` | `frame.rs:170-172` |
 * | the four weights `(1−fx)(1−fy), fx(1−fy), (1−fx)fy, fx·fy` | `deposit.rs:186-193` |
 * | the four cells `at, at+1, at+P, at+P+1` | `deposit.rs:167-170` |
 * | `atomicAdd` per cell, only `.re` — the imaginary column is +0 throughout | `deposit.rs:173` |
 *
 * **The weights are fixed point at `scaleFor(n)`, and the fourth is the remainder.** Three
 * weights go out through a correctly-rounded `round`, and `w3 = scale − w0 − w1 − w2`, so the
 * four sum to `scale` *exactly* in integer arithmetic and every node deposits exactly one
 * unit. That is what makes `depositedUnits === n · scale` an equality with no tolerance, the
 * one check in the tier with none (`gpu-force-tier.md:48-52`). Three rounded and one
 * remainder is the only arrangement with that property: four rounded weights sum to
 * `scale ± 2` quanta and the total drifts with `n`.
 *
 * Caveat: `w3` can come out **negative** when the three rounded weights exceed `scale`, which
 * happens for a node whose fractional cell offset puts almost all its charge on three of the
 * four cells. `atomicAdd` accepts a negative `i32` and the density total is still exact, but a
 * cell's *intermediate* value can dip below zero — invisible in the sum and visible in the
 * spectrum. Failing input: a fixture whose deposit is dominated by single-cell nodes. The
 * direction of the error is one quantum, 2⁻¹¹ at 1M, against a ceiling derived from it.
 *
 * **`cell` is a truncating cast and a `min`, never a `floor` of a clamped value.** The CPU's
 * `u as usize` truncates toward zero *and* saturates, so a negative `u` lands on 0 and a huge
 * one lands on `cells − 2` (`frame.rs:167-172`). WGSL's `u32(f32)` is undefined for a negative
 * or out-of-range argument, so the cast is guarded by the same `min` the CPU applies —
 * `clamp` before `u32`, not after — and the guard is written once, in `cell()`.
 *
 * ## The density is `i32`, and the widening is one exact multiply
 *
 * The CPU's density is a `f64` complex column. Here it is an `i32` fixed-point column, so
 * `widen_density` converts with `f32(v) · inv_scale`: `inv_scale` is a power of two
 * (`scale_for` returns `2^(31−⌈log₂n⌉)`), so the multiply is exact for every value the buffer
 * can hold, and the transform then sees exactly the density the deposit meant. The imaginary
 * column is `+0` because the CPU's deposit only ever accumulates `.re` (`deposit.rs:173`).
 */

import { NODES_WGSL, PRELUDE_WGSL } from "./prelude.wgsl.ts";

/** The WGSL for the fixed-point deposit and the widening. */
export const DEPOSIT_WGSL = `${PRELUDE_WGSL}
${NODES_WGSL}
@group(0) @binding(2) var<storage, read_write> density: array<atomic<i32>>;
@group(0) @binding(3) var<storage, read_write> field: array<vec2<f32>>;

// The lower cell of scaled coordinate u on one axis, inside 0..cells-1. frame.rs:170-172:
// (u as usize).min(cells - 2), whose truncation toward zero and saturation make it a floor for
// u >= 0 and a clamp to 0 for u < 0. WGSL's u32() is undefined outside [0, 2^32), so the
// clamp comes first.
fn cell(u: f32) -> u32 {
  let hi = f32(frame.cells - 2u);
  return u32(clamp(u, 0.0, hi));
}

// The CIC stencil of one scaled position: the lower cell per axis and the weight of the upper.
// frame.rs:176-187, which clamps the fraction to [0, 1] for the same reason.
fn stencil(ux: f32, uy: f32) -> vec4<f32> {
  let cx = cell(ux);
  let cy = cell(uy);
  let fx = clamp(ux - f32(cx), 0.0, 1.0);
  let fy = clamp(uy - f32(cy), 0.0, 1.0);
  // (cx, cy, fx, fy) packed for the caller; WGSL has no tuple return.
  return vec4<f32>(f32(cx), f32(cy), fx, fy);
}

// The four CIC weights in the CPU's cell order (x, y), (x+1, y), (x, y+1), (x+1, y+1).
// deposit.rs:186-193.
fn weights(fx: f32, fy: f32) -> vec4<f32> {
  return vec4<f32>(
    (1.0 - fx) * (1.0 - fy),
    fx * (1.0 - fy),
    (1.0 - fx) * fy,
    fx * fy,
  );
}

// The same four weights in fixed point: three correctly rounded, the fourth the remainder, so
// they sum to scale exactly. WGSL's round() is ties-to-even, which is why it is the only
// rounding here and not round-half-away-from-zero: the property being bought is the exact sum,
// not a particular tie, and ties-to-even is exactly reproducible on both arms.
fn fixed_weights(fx: f32, fy: f32) -> vec4<i32> {
  let w = weights(fx, fy);
  let scale = f32(frame.scale);
  let w0 = i32(round(w.x * scale));
  let w1 = i32(round(w.y * scale));
  let w2 = i32(round(w.z * scale));
  // The fourth is the remainder, so the four sum to scale in integer arithmetic whatever the
  // three roundings did. --break weight adds one quantum here, which is the fault that makes
  // every node deposit scale + 1 and the exactness check red.
  var w3 = i32(frame.scale) - w0 - w1 - w2;
  if (frame.fault == 4u) {
    w3 = w3 + 1;
  }
  return vec4<i32>(w0, w1, w2, w3);
}

// One unit charge per finite node, CIC weighted, into the fixed-point density. Four
// atomicAdd<i32>, in the CPU's cell order. mesh.rs:190-211 calls this; the runner's clear of
// the density is the zero kernel's dispatch, not a read here.
@compute @workgroup_size(256)
fn deposit(@builtin(global_invocation_id) id: vec3<u32>) {
  let k = id.x;
  if (k >= frame.n) {
    return;
  }
  let p = nodes[k];
  if (p.x != p.x || p.y != p.y) {
    return;
  }
  // The frame's scaled coordinate, (p - origin) / h. frame.rs:157-165 returns NaN for a
  // non-finite position, and the non-finite node was already returned above.
  let ux = (p.x - frame.origin_x) / frame.h;
  let uy = (p.y - frame.origin_y) / frame.h;
  let st = stencil(ux, uy);
  let cx = u32(st.x);
  let cy = u32(st.y);
  let q = fixed_weights(st.z, st.w);
  // --break deposit moves every node one cell along x, weights unchanged: the fault that
  // proves the weights are read and not merely summed, because the total still deposits
  // exactly one unit per node and only the *field* moves.
  var at = cy * frame.side + cx;
  if (frame.fault == 2u) {
    at = at + 1u;
  }
  atomicAdd(&density[at], q.x);
  atomicAdd(&density[at + 1u], q.y);
  atomicAdd(&density[at + frame.side], q.z);
  atomicAdd(&density[at + frame.side + 1u], q.w);
}

// The fixed point back to the f32 complex column the transforms read. inv_scale is a power of
// two, so this multiply is exact and the imaginary column is +0 because the CPU's deposit
// accumulates only .re (deposit.rs:173).
@compute @workgroup_size(256)
fn widen_density(@builtin(global_invocation_id) id: vec3<u32>) {
  let at = id.x;
  if (at < frame.side * frame.side) {
    field[at] = vec2<f32>(f32(atomicLoad(&density[at])) * frame.inv_scale, 0.0);
  }
}
`;