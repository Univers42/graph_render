/**
 * `collide-resolve.wgsl.ts` — the collide pass's last stage: each node's half of every
 * overlap it has, summed in the CPU's order.
 *
 * This is `Gather` and `resolve` (`collide/gather.rs:69-89`, `collide.rs:225-246`), one
 * invocation per node. Node `i` reads its stored cell, lists the buckets a query from that
 * cell reads exactly as `Grid::reads` does (rows `cy - 1..=cy + 1`, three buckets left to right
 * per row, a bucket an earlier row already read skipped), and walks their member lists in that
 * order. The scatter made each list ascending, so the candidates arrive in the CPU's sequence
 * and the sum's terms are added in the CPU's order. The node itself is skipped, as the CPU
 * skips its own place (`gather.rs:126-129`).
 *
 * The push is `resolve`'s, term for term: `l = dx² + dy²`, no overlap when `l` is not under
 * `d2` (a NaN included), the coincidence jiggle on an exactly zero axis, then
 * `(reach − dist) / dist · 0.5`. The arithmetic is `f32` where the CPU's is `f64`, and the
 * positions were narrowed once on upload; that is the error the ceilings measure.
 *
 * ## The window: the CPU's fixed 256, exact
 *
 * The plan leaves the window's shape open (`:1340-1351`). This is its first shape: the
 * candidates are visited a window of `WINDOW = 256` at a time (`gather.rs:18`), every window,
 * so none is dropped and no truncation count is needed. The device keeps no copy of a window:
 * the sequence is read in place, and the window is only its chunking, which
 * `--break collide-window` uses to pick the candidate it drops.
 *
 * Caveat: the window loses no contact. The contacts this stage can lose or gain against the CPU
 * are the pairs whose `f32` distance rounds across the diameter (`l` within an `f32` step of
 * `d2`); the push of such a pair is near zero, so the loss is near zero too. Its cost is a
 * crowd's whole population per query: the measured fixtures read at most 55 candidates per
 * node (50k, settled), and a dense overlap that put thousands in nine buckets would run as long
 * per invocation as the CPU's own quadratic case.
 *
 * ## The jiggle, in 32-bit words
 *
 * `rng.rs`'s `jiggle` is three `fmix64` rounds over `(seed, tick, pass, min(i, j), max(i, j))`.
 * WGSL has no `u64`, so a 64-bit word is a `vec2<u32>` `(lo, hi)` and the product's low 64
 * bits are built from 16-bit limbs; every shift in `fmix64` is 33, so it is one cross-word
 * shift. The 53-bit draw becomes `f32` as `hi·2⁻²¹ + lo·2⁻⁵³`, within 6e-14 of the CPU's `f64`
 * nudge. The keys are the session's `SEED = 0` and tick 0, the tick the fixture's probe ran at
 * (`fixtures/gpu/README.md:36-41`).
 */

import { COLLIDE_GRID_WGSL, COLLIDE_PRELUDE_WGSL } from "./collide.wgsl.ts";

/** The WGSL for `collide_resolve`. */
export const COLLIDE_RESOLVE_WGSL = `${COLLIDE_PRELUDE_WGSL}
${COLLIDE_GRID_WGSL}
@group(0) @binding(4) var<storage, read> cells: array<vec2<i32>>;
@group(0) @binding(7) var<storage, read> start: array<u32>;
@group(0) @binding(10) var<storage, read> order: array<u32>;
@group(0) @binding(11) var<storage, read_write> delta: array<vec2<f32>>;

const WINDOW: u32 = 256u;

// The full 64-bit product of two u32, as (lo, hi), from 16-bit limbs.
fn mul_wide(a: u32, b: u32) -> vec2<u32> {
  let a0 = a & 0xffffu;
  let a1 = a >> 16u;
  let b0 = b & 0xffffu;
  let b1 = b >> 16u;
  let p00 = a0 * b0;
  let p01 = a0 * b1;
  let p10 = a1 * b0;
  let mid = (p00 >> 16u) + (p01 & 0xffffu) + (p10 & 0xffffu);
  return vec2<u32>((mid << 16u) | (p00 & 0xffffu), a1 * b1 + (p01 >> 16u) + (p10 >> 16u) + (mid >> 16u));
}

// u64::wrapping_mul: the low 64 bits of a * b.
fn mul64(a: vec2<u32>, b: vec2<u32>) -> vec2<u32> {
  let p = mul_wide(a.x, b.x);
  return vec2<u32>(p.x, p.y + a.x * b.y + a.y * b.x);
}

// k ^ (k >> 33).
fn xor_shift(k: vec2<u32>) -> vec2<u32> {
  return vec2<u32>(k.x ^ (k.y >> 1u), k.y);
}

// rng.rs fmix64.
fn fmix64(k: vec2<u32>) -> vec2<u32> {
  var h = mul64(xor_shift(k), vec2<u32>(0xed558ccdu, 0xff51afd7u));
  h = mul64(xor_shift(h), vec2<u32>(0x1a85ec53u, 0xc4ceb9feu));
  return xor_shift(h);
}

// rng.rs jiggle(co.seed, co.tick, salt, (i, j)), then jiggle_of's (w * 2^-53 - 0.5) * 1e-6.
fn jiggle(salt: u32, i: u32, j: u32) -> f32 {
  let a = fmix64(vec2<u32>(co.seed, co.tick));
  let b = fmix64(vec2<u32>(a.x ^ min(i, j), a.y ^ salt));
  let h = fmix64(vec2<u32>(b.x ^ a.x, b.y ^ max(i, j) ^ a.y));
  let w_hi = h.y >> 11u;
  var w_lo = (h.x >> 11u) | (h.y << 21u);
  // rng.rs:81: the one word that would make the nudge exactly zero is moved to its neighbour.
  if (w_hi == 0x100000u && w_lo == 0u) {
    w_lo = 1u;
  }
  let u = ldexp(f32(w_hi), -21) + ldexp(f32(w_lo), -53);
  return (u - 0.5) * 1e-6;
}

// collide.rs:225-246 for one pair; offset is the querying node's position minus the other's.
fn push(i: u32, j: u32, offset: vec2<f32>) -> vec2<f32> {
  var dx = offset.x;
  var dy = offset.y;
  var l = dx * dx + dy * dy;
  if (!(l < co.d2)) {
    return vec2<f32>(0.0, 0.0);
  }
  if (dx == 0.0) {
    dx = jiggle(4u, i, j);
    l += dx * dx;
  }
  if (dy == 0.0) {
    dy = jiggle(5u, i, j);
    l += dy * dy;
  }
  let dist = sqrt(l);
  let scale = (co.reach - dist) / dist * 0.5;
  return vec2<f32>(dx * scale, dy * scale);
}

// Grid::reads (collide.rs:172-191): the buckets a query from cell c reads, in order, and how
// many candidates they hold together.
struct Reads {
  at: array<u32, 9>,
  len: u32,
  total: u32,
};

fn reads(c: vec2<i32>) -> Reads {
  var r: Reads;
  var firsts: array<u32, 3>;
  for (var row = 0u; row < 3u; row++) {
    let first = bucket_of(vec2<i32>(c.x - 1, c.y + i32(row) - 1));
    for (var t = 0u; t < 3u; t++) {
      let b = (first + t) & co.mask;
      var seen = false;
      for (var e = 0u; e < row; e++) {
        seen = seen || ((b - firsts[e]) & co.mask) < 3u;
      }
      if (!seen) {
        r.at[r.len] = b;
        r.len++;
        r.total += start[b + 1u] - start[b];
      }
    }
    firsts[row] = first;
  }
  return r;
}

// --break collide-window: the last populated candidate of each window of 256 is not resolved.
fn dropped(at: u32, total: u32) -> bool {
  return co.fault == FAULT_WINDOW && (at % WINDOW == WINDOW - 1u || at + 1u == total);
}

@compute @workgroup_size(256)
fn collide_resolve(@builtin(global_invocation_id) id: vec3<u32>) {
  let i = id.x;
  if (i >= co.n) {
    return;
  }
  let p = nodes[i];
  var r = reads(cells[i]);
  var out = vec2<f32>(0.0, 0.0);
  var at = 0u;
  for (var k = 0u; k < r.len; k++) {
    let b = r.at[k];
    for (var q = start[b]; q < start[b + 1u]; q++) {
      let j = order[q];
      if (j != i && !dropped(at, r.total)) {
        out += push(i, j, p - nodes[j]);
      }
      at++;
    }
  }
  delta[i] = out;
}
`;
