/**
 * `collide-hash.wgsl.ts` — the collide pass's first stage: every node's cell and bucket, and
 * one `atomicAdd` on its bucket's count.
 *
 * This is `Buckets::step_range` (`collide/hash.rs:47-54`) plus the counting half of
 * `Grid::build`'s sort (`collide.rs:150-153`), one invocation per node. The count is the only
 * thing the atomic decides that a later stage reads as a number, and a sum of ones is the same
 * sum in any order, so the counts are exact.
 *
 * `arrival` is the value `atomicAdd` returned: a distinct place in `0..count` within the
 * bucket, in whatever order the device ran the adds. It is **not** an order and nothing reads
 * it as one — `collide_place` uses it only to put each node somewhere in its bucket's span, and
 * `collide_scatter` then ranks the span by node index. The cell is stored rather than recomputed
 * by the resolve, so the stage that sorts and the stage that queries read one cell per node.
 */

import { COLLIDE_GRID_WGSL, COLLIDE_PRELUDE_WGSL } from "./collide.wgsl.ts";

/** The WGSL for `collide_hash`. */
export const COLLIDE_HASH_WGSL = `${COLLIDE_PRELUDE_WGSL}
${COLLIDE_GRID_WGSL}
@group(0) @binding(3) var<storage, read_write> counts: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> cells: array<vec2<i32>>;
@group(0) @binding(5) var<storage, read_write> bucket: array<u32>;
@group(0) @binding(6) var<storage, read_write> arrival: array<u32>;

@compute @workgroup_size(256)
fn collide_hash(@builtin(global_invocation_id) id: vec3<u32>) {
  let i = id.x;
  if (i >= co.n) {
    return;
  }
  let c = cell_of(nodes[i]);
  let b = bucket_of(c);
  cells[i] = c;
  bucket[i] = b;
  arrival[i] = atomicAdd(&counts[b], 1u);
}
`;
