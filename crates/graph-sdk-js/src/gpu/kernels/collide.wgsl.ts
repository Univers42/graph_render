/**
 * `collide.wgsl.ts` — what every stage of the collide pass shares: its uniform, the fault
 * numbers, and the grid's cell and bucket functions.
 *
 * The pass is `collide.rs` in four stages, one module each (`collide-hash`, `collide-scan`,
 * `collide-scatter`, `collide-resolve`); this file has no entry point, as charge's prelude has
 * none. WGSL has no include, so each module composes these strings and the layout is written
 * once.
 *
 * ## The uniform, sixteen 4-byte members
 *
 * | field | | read by |
 * |---|---|---|
 * | `n` | node count | every per-node stage's guard |
 * | `buckets`, `mask` | `max(4, (2n).next_power_of_two())` and that minus one (`collide.rs:68`) | hash, scan, resolve |
 * | `rows` | the row table's length | `bucket_of` |
 * | `blocks`, `chunk` | the scan's workgroups, and block sums per invocation in its second dispatch | scan |
 * | `seed`, `tick` | the jiggle's keys: the session's `SEED = 0` and tick 0 (`fixtures/gpu/README.md:36-41`) | resolve |
 * | `fault` | which `--break` fault, 0 for none | scatter, resolve |
 * | `origin_x`, `origin_y` | the finite positions' minimum, `frame::bounds`'s `lo` (`collide.rs:146`) | `cell_of` |
 * | `inv_size` | `1 / diameter`, narrowed once | `cell_of` |
 * | `d2`, `reach` | the squared diameter and the diameter (`collide.rs:262-273`) | resolve |
 *
 * ## The row table
 *
 * `hash.rs:30` hashes a row as `(cy as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> shift`, a
 * 64-bit product WGSL does not have. The host evaluates it once per row in `BigInt` and uploads
 * `rows[cy + 1]` for `cy` in `-1..=rowMax + 1`, so the kernel's bucket is one add and one mask
 * and the 64-bit hash is the CPU's to the bit. `u32(cx)` wraps the way `cx as u64` does, and
 * the mask is under 2³², so the sum agrees with `hash.rs:24-25` modulo the bucket count.
 *
 * Caveat: the table is sized from the host's own `cellOf`, which mirrors `cell_of` op for op
 * (both correctly rounded). A position whose row the device computed differently would read
 * the clamped end row: a wrong bucket, so a lost contact, which the comparator sees as `rms`.
 * Ponytail: a resident tick has no host-side extent; it moves the 64-bit product into the
 * kernel (16-bit limbs, as the jiggle does) and drops the table.
 */

/** The `--break` faults this pass knows, by the number its uniform carries. */
export const COLLIDE_FAULTS = {
  /** The scatter ranks node `i` as `n - 1 - i`: every member list comes out descending. */
  "collide-order": 1,
  /** The resolve drops the last populated candidate of each 256-candidate window. */
  "collide-window": 2,
} as const;

/** The uniform block and the fault numbers, shared verbatim by every collide stage. */
export const COLLIDE_PRELUDE_WGSL = `
struct Collide {
  n: u32,
  buckets: u32,
  mask: u32,
  rows: u32,
  blocks: u32,
  chunk: u32,
  seed: u32,
  tick: u32,
  fault: u32,
  pad0: u32,
  pad1: u32,
  origin_x: f32,
  origin_y: f32,
  inv_size: f32,
  d2: f32,
  reach: f32,
};

@group(0) @binding(0) var<uniform> co: Collide;

const FAULT_ORDER: u32 = ${COLLIDE_FAULTS["collide-order"]}u;
const FAULT_WINDOW: u32 = ${COLLIDE_FAULTS["collide-window"]}u;
`;

/** The positions, the row table, and the two grid functions the hash and the resolve share. */
export const COLLIDE_GRID_WGSL = `
@group(0) @binding(1) var<storage, read> nodes: array<vec2<f32>>;
@group(0) @binding(2) var<storage, read> rows: array<u32>;

// hash.rs:19-22: ((v - o) / size) as i64, truncating and saturating; a NaN lands in 0. WGSL's
// i32(f32) truncates and saturates the same way, and inv_size makes the division a multiply,
// exact while the diameter is a power of two (32 at the frozen parameters).
fn axis_cell(v: f32, o: f32) -> i32 {
  let u = (v - o) * co.inv_size;
  if (u != u) {
    return 0;
  }
  return i32(u);
}

fn cell_of(p: vec2<f32>) -> vec2<i32> {
  return vec2<i32>(axis_cell(p.x, co.origin_x), axis_cell(p.y, co.origin_y));
}

// hash.rs:24-25 with the row's 64-bit hash read from the host's table.
fn bucket_of(c: vec2<i32>) -> u32 {
  let at = u32(clamp(c.y + 1, 0, i32(co.rows) - 1));
  return (rows[at] + u32(c.x)) & co.mask;
}
`;
