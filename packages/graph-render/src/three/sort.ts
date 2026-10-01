/**
 * Painting a 3D drawing in Canvas2D: the painter's algorithm. Canvas2D has no z-buffer, so
 * depth is carried by order — the furthest node is drawn first and the nearest over it.
 *
 * The order is a counting sort on a quantised depth, not a comparison sort: `depth` is a
 * f64 the projection produced, and sorting floats in place is a determinism risk (D2/D3:
 * fixed-order reductions, ties broken by a dense index). The quantiser here is exact
 * integer code, and the tie-break is the node's own index, so two nodes in one bucket
 * always paint in dense order whichever way the sort is run.
 *
 * Ponytail: one bucket per world unit of depth, so the sort is linear in the node count and
 * a drawing 4000 units deep gets 4000 buckets. That is the price of not sorting floats; a
 * comparison sort would be `O(n log n)` and would need its tie-break argued for instead.
 */
import { NEAR } from "./orbit.ts";

/** Depth buckets per world unit. One bucket is one world unit of depth. */
export const BUCKETS_PER_UNIT = 1;

/**
 * The node indices in the order they are painted: furthest first, so each node is drawn
 * over everything behind it. A node at or behind the eye plane (`depth <= NEAR`) is not in
 * the order at all, and the painter draws no trace of it.
 *
 * `out` is filled and returned; it must hold at least one entry per node, and its own
 * leading entries are what the painter walks, so the view keeps one per frame.
 */
export function depthOrder(depths: Float64Array, out: Uint32Array): Uint32Array {
  const count = depths.length;
  if (out.length < count) throw new RangeError(`the order holds ${out.length} of ${count} nodes`);
  const min = minDepth(depths);
  const buckets = Math.max(1, Math.ceil((maxDepth(depths) - min) * BUCKETS_PER_UNIT));
  // The runs are laid out furthest bucket first, so slot 0 is the furthest node and the
  // painter walks the array forwards. Within a run the nodes go in dense index order, so a
  // pair at one depth always paints index-then-index however the depths were arranged.
  const counts = new Int32Array(buckets);
  for (let node = 0; node < count; node += 1) {
    const depth = depths[node] ?? 0;
    if (depth > NEAR) {
      const bucket = bucketOf(depth, min, buckets);
      counts[bucket] = (counts[bucket] ?? 0) + 1;
    }
  }
  const starts = new Int32Array(buckets);
  let slot = 0;
  for (let bucket = buckets - 1; bucket >= 0; bucket -= 1) {
    starts[bucket] = slot;
    slot += counts[bucket] ?? 0;
  }
  const filled = new Int32Array(buckets);
  for (let node = 0; node < count; node += 1) {
    const depth = depths[node] ?? 0;
    if (!(depth > NEAR)) continue;
    const bucket = bucketOf(depth, min, buckets);
    out[(starts[bucket] ?? 0) + (filled[bucket] ?? 0)] = node;
    filled[bucket] = (filled[bucket] ?? 0) + 1;
  }
  return out.subarray(0, slot);
}

function minDepth(depths: Float64Array): number {
  let min = Infinity;
  for (let node = 0; node < depths.length; node += 1) {
    const depth = depths[node] ?? 0;
    if (depth > NEAR && depth < min) min = depth;
  }
  return min === Infinity ? 0 : min;
}

function maxDepth(depths: Float64Array): number {
  let max = 0;
  for (let node = 0; node < depths.length; node += 1) {
    const depth = depths[node] ?? 0;
    if (depth > max) max = depth;
  }
  return max;
}

/** A depth's bucket: 0 is the nearest, the last is the furthest. */
function bucketOf(depth: number, min: number, buckets: number): number {
  const at = Math.floor((depth - min) * BUCKETS_PER_UNIT);
  return at < 0 ? 0 : at >= buckets ? buckets - 1 : at;
}
