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
 * The bucket WIDTH comes from the drawing's own depth range (`range / buckets`) and not
 * from a fixed number of world units, so the same relative spread of depths gives the same
 * order whatever the scale the drawing was laid out at. A frame whose nodes all sit within
 * one world unit of each other used to land entirely in one bucket and fall back to dense
 * index order; now it gets one bucket per node like any other frame.
 *
 * Ponytail: the width is `range / buckets` with `buckets` between the in-front node count
 * and `ceil(range)`. Keeping the `ceil(range)` term means a drawing deeper than the bucket
 * cap still gets the old one-world-unit resolution, where an integral depth lands in a
 * bucket of its own and z-fighting at that spacing is invisible. What it gets wrong: a
 * drawing deeper than `MAX_BUCKETS` world units quantises the depth to `range /
 * MAX_BUCKETS`, so two nodes nearer each other than that share a bucket and paint in dense
 * index order instead of by depth. Zooming in shrinks the range and is the escape hatch.
 *
 * Ponytail: `MAX_BUCKETS` bounds the three counting arrays, which are otherwise as wide as
 * the drawing is deep — a 4000-unit-deep drawing would otherwise allocate 4000 ints three
 * times over, for no visible gain past the point where the buckets are narrower than a
 * pixel.
 */
import { NEAR } from "./orbit.ts";

/** The most buckets the counting arrays get, whatever the drawing's depth range. */
export const MAX_BUCKETS = 1024;

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
  const span = maxDepth(depths) - min;
  const buckets = bucketCount(depths, span);
  const width = span > 0 ? span / buckets : 1;
  // The runs are laid out furthest bucket first, so slot 0 is the furthest node and the
  // painter walks the array forwards. Within a run the nodes go in dense index order, so a
  // pair at one depth always paints index-then-index however the depths were arranged.
  const counts = new Int32Array(buckets);
  for (let node = 0; node < count; node += 1) {
    const depth = depths[node] ?? 0;
    if (!(depth > NEAR)) continue;
    const bucket = bucketOf(depth, min, width, buckets);
    counts[bucket] = (counts[bucket] ?? 0) + 1;
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
    const bucket = bucketOf(depth, min, width, buckets);
    out[(starts[bucket] ?? 0) + (filled[bucket] ?? 0)] = node;
    filled[bucket] = (filled[bucket] ?? 0) + 1;
  }
  return out.subarray(0, slot);
}

/**
 * How many buckets the counting arrays get: at least one per node in front of the eye and,
 * where the range is deeper than that, one per world unit of depth — bounded by
 * `MAX_BUCKETS`. A drawing with nothing in front, or every node at one depth, gets the one
 * bucket, which is also the whole drawing: no division and no NaN index.
 */
function bucketCount(depths: Float64Array, span: number): number {
  if (!(span > 0)) return 1;
  let inFront = 0;
  for (let node = 0; node < depths.length; node += 1) if ((depths[node] ?? 0) > NEAR) inFront += 1;
  return Math.min(MAX_BUCKETS, Math.max(inFront, Math.ceil(span)));
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
    if (depth > NEAR && depth > max) max = depth;
  }
  return max;
}

/**
 * A depth's bucket: 0 is the nearest, the last is the furthest.
 *
 * The `?? 0` on every typed-array read below is NOT dead code, whatever a reviewer says:
 * `noUncheckedIndexedAccess` is on in `tsconfig.json`, so `depths[node]` is `number | undefined`
 * to the checker even though the array is in range. The read is in range by construction —
 * `depths` is indexed by `0..depths.length - 1` and nowhere else — so 0 is the unreachable
 * case, and naming it is what lets the arithmetic below stay free of assertions.
 */
function bucketOf(depth: number, min: number, width: number, buckets: number): number {
  const at = Math.floor((depth - min) / width);
  return at < 0 ? 0 : at >= buckets ? buckets - 1 : at;
}