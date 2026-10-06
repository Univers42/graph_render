/**
 * `collide-scan.wgsl.ts` — the collide pass's second stage: each bucket's first slot, the
 * exclusive prefix sum of the counts, as three dispatches of 256.
 *
 * This is `Grid::build`'s `for b in 1..start.len()` (`collide.rs:154-156`), the one stage of
 * the pass wider than a workgroup: `2²¹` buckets at 1M. The answer is dispatches, not width
 * (plan `:1318-1324`):
 *
 * | dispatch | workgroups | does |
 * |---|---:|---|
 * | `scan_blocks` | `blocks = ⌈buckets/256⌉`, 8 192 at 1M | a Blelloch scan of each block of 256 counts; the block's total into `sums` |
 * | `scan_sums` | 1 | each invocation folds `chunk = ⌈blocks/256⌉` block sums, a Blelloch over the 256 folds, then each chunk's exclusive prefix written back; the grand total into `start[buckets]` |
 * | `scan_add` | `blocks` | each bucket's block offset added in |
 *
 * Every count is a `u32` and every sum an integer add, so the scan is exact and the order of
 * its adds cannot change a bit. `start[buckets]` is `n`, the end of the last bucket's span, so
 * every bucket's span is `start[b]..start[b + 1]` with no special case. `blocks` stays under
 * 65 535 per dimension up to `2²⁴` buckets (n ≈ 8M), and the host refuses a grid past that.
 */

import { COLLIDE_PRELUDE_WGSL } from "./collide.wgsl.ts";

/** The WGSL for `scan_blocks`, `scan_sums` and `scan_add`. */
export const COLLIDE_SCAN_WGSL = `${COLLIDE_PRELUDE_WGSL}
@group(0) @binding(3) var<storage, read> counts: array<u32>;
@group(0) @binding(7) var<storage, read_write> start: array<u32>;
@group(0) @binding(8) var<storage, read_write> sums: array<u32>;

var<workgroup> tile: array<u32, 256>;

// One workgroup's exclusive Blelloch scan of tile, in place; returns the tile's total. Called
// from uniform control flow only: every barrier is reached by all 256 invocations.
fn exclusive_scan(l: u32) -> u32 {
  var offset = 1u;
  for (var d = 128u; d > 0u; d = d >> 1u) {
    workgroupBarrier();
    if (l < d) {
      tile[offset * (2u * l + 2u) - 1u] += tile[offset * (2u * l + 1u) - 1u];
    }
    offset = offset * 2u;
  }
  workgroupBarrier();
  let total = tile[255];
  workgroupBarrier();
  if (l == 0u) {
    tile[255] = 0u;
  }
  for (var d = 1u; d < 256u; d = d * 2u) {
    offset = offset >> 1u;
    workgroupBarrier();
    if (l < d) {
      let ai = offset * (2u * l + 1u) - 1u;
      let bi = offset * (2u * l + 2u) - 1u;
      let t = tile[ai];
      tile[ai] = tile[bi];
      tile[bi] += t;
    }
  }
  workgroupBarrier();
  return total;
}

@compute @workgroup_size(256)
fn scan_blocks(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) l: u32) {
  let b = group.x * 256u + l;
  tile[l] = select(0u, counts[min(b, co.buckets - 1u)], b < co.buckets);
  let total = exclusive_scan(l);
  if (b < co.buckets) {
    start[b] = tile[l];
  }
  if (l == 0u) {
    sums[group.x] = total;
  }
}

// The chunk loops run to the uniform co.chunk and test each index inside: a loop whose trip
// count is the invocation's own would leave the barriers after it in non-uniform control flow.
@compute @workgroup_size(256)
fn scan_sums(@builtin(local_invocation_index) l: u32) {
  var folded = 0u;
  for (var k = 0u; k < co.chunk; k++) {
    let at = l * co.chunk + k;
    if (at < co.blocks) {
      folded += sums[at];
    }
  }
  tile[l] = folded;
  let total = exclusive_scan(l);
  var running = tile[l];
  for (var k = 0u; k < co.chunk; k++) {
    let at = l * co.chunk + k;
    if (at < co.blocks) {
      let v = sums[at];
      sums[at] = running;
      running += v;
    }
  }
  if (l == 0u) {
    start[co.buckets] = total;
  }
}

@compute @workgroup_size(256)
fn scan_add(@builtin(global_invocation_id) id: vec3<u32>) {
  let b = id.x;
  if (b < co.buckets) {
    start[b] += sums[b / 256u];
  }
}
`;
