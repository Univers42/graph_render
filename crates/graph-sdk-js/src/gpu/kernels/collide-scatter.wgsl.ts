/**
 * `collide-scatter.wgsl.ts` — the collide pass's third stage: every node into its bucket's
 * span, each span in ascending node index.
 *
 * This is `Grid::build`'s scatter (`collide.rs:157-165`). The CPU's loop is one thread's, so
 * reading the nodes in index order is what makes each bucket's list ascending. A device has no
 * such order: an `atomicAdd` cursor hands out places in whatever order the invocations ran.
 * So the stage is two dispatches, and only the second decides the order:
 *
 * 1. `collide_place` — node `i` at `start[b] + arrival[i]`, the place the hash's `atomicAdd`
 *    returned: the bucket's members, in no particular order, in `loose`.
 * 2. `collide_scatter` — invocation `i` handles node `i`: its rank is the number of members of
 *    its bucket with a smaller index, and it writes itself at `start[b] + rank` in `order`. The
 *    ranks of distinct indices are a permutation of the span, so every list comes out
 *    ascending whatever order the adds ran in, and two runs write the same bytes.
 *
 * Caveat: the rank is a scan of the node's own bucket, so a bucket of `k` nodes costs `k²`
 * reads. The measured fixtures hold at most 11 nodes per bucket (50k, settled); a dense
 * overlap that put thousands in one bucket would make this stage quadratic in that crowd, as it
 * makes the resolve.
 *
 * `--break collide-order` ranks node `i` by `n - 1 - i`: the same members, every list
 * descending. The resolve then sums the same candidates in another order, which no `f32` bound
 * sees; the `order` check does.
 */

import { COLLIDE_PRELUDE_WGSL } from "./collide.wgsl.ts";

/** The WGSL for `collide_place` and `collide_scatter`. */
export const COLLIDE_SCATTER_WGSL = `${COLLIDE_PRELUDE_WGSL}
@group(0) @binding(5) var<storage, read> bucket: array<u32>;
@group(0) @binding(6) var<storage, read> arrival: array<u32>;
@group(0) @binding(7) var<storage, read> start: array<u32>;
@group(0) @binding(9) var<storage, read_write> loose: array<u32>;
@group(0) @binding(10) var<storage, read_write> order: array<u32>;

@compute @workgroup_size(256)
fn collide_place(@builtin(global_invocation_id) id: vec3<u32>) {
  let i = id.x;
  if (i >= co.n) {
    return;
  }
  loose[start[bucket[i]] + arrival[i]] = i;
}

// The key a span is ranked by: the node index, or its mirror under --break collide-order.
fn rank_key(j: u32) -> u32 {
  if (co.fault == FAULT_ORDER) {
    return co.n - 1u - j;
  }
  return j;
}

@compute @workgroup_size(256)
fn collide_scatter(@builtin(global_invocation_id) id: vec3<u32>) {
  let i = id.x;
  if (i >= co.n) {
    return;
  }
  let b = bucket[i];
  let lo = start[b];
  let hi = start[b + 1u];
  let mine = rank_key(i);
  var rank = 0u;
  for (var q = lo; q < hi; q++) {
    rank += select(0u, 1u, rank_key(loose[q]) < mine);
  }
  order[lo + rank] = i;
}
`;
