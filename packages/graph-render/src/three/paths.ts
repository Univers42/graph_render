/**
 * The shapes a disc and a chord cannot draw, projected once per frame: a node's own w/h as
 * screen half-extents, and the interior points of a routed or curved edge as screen points.
 *
 * `projection.ts` calls these while the frame is being drawn and the painter reads what they
 * leave in `Drawn`, so a 3D frame costs one projection of every shape it draws and the
 * painter never projects — the same bargain the node points already keep.
 *
 * Ponytail: an interior edge point is 2D in the contract (`docs/contract/binary-layout.md`:
 * "Edge paths stay 2D whatever `dim` is"), so it has no z and one has to be invented. This
 * module interpolates it linearly between the two endpoint nodes' z, by the vertex's
 * parameter along the path measured as the run of the interior points in world units. That
 * fails for a curve whose control points bunch at one end — the control point nearest the
 * target then takes the source's z and the edge's depth sags on the far side — and for a
 * degenerate path whose points all coincide, which takes the source's z throughout. The
 * escape hatch is a layout that carries 3D edge points; until then a z-banded edge draws
 * through the wrong depth rather than not at all.
 *
 * Ponytail: a box's half-extent is `radiusOnScreen(focal, w / 2, depth)` — the same
 * perspective scale the radius uses — so it does not foreshorten with the orbit's yaw and
 * pitch: a box seen edge-on is still a full-size screen rectangle. It fails for a drawing
 * that is mostly seen from the side, where the true projection would squash it. The escape
 * hatch is per-axis extents from a perspective divide of the rotated corner offsets.
 */
import type { Frame } from "../frame.ts";
import type { Drawn, Projection, Setup } from "./projection.ts";
import { project, radiusOnScreen } from "./orbit.ts";

/** The two ends of one edge, and the z's a point between them is interpolated from. */
interface Ends {
  readonly s: number;
  readonly t: number;
  readonly zs: number;
  readonly zt: number;
}

/** One edge's projection: the arrays it writes and the numbers it reads. */
interface Edge {
  readonly out: Float32Array;
  readonly wanted: Projection;
  readonly drawn: Drawn;
  readonly setup: Setup;
}

/** Two interior points of a path apart, in the plane the contract keeps them in. */
function chord(pts: Float32Array, a: number, b: number): number {
  const dx = (pts[2 * b] ?? 0) - (pts[2 * a] ?? 0);
  const dy = (pts[2 * b + 1] ?? 0) - (pts[2 * a + 1] ?? 0);
  return Math.sqrt(dx * dx + dy * dy);
}

/** The run from the first interior point to the one at `at`, in world units. */
function runTo(pts: Float32Array, from: number, at: number): number {
  let run = 0;
  for (let p = from + 1; p <= at; p += 1) run += chord(pts, p - 1, p);
  return run;
}

/**
 * The path's whole length in that plane: the run of the interior points plus the last leg to
 * the target node, whose position is the 2D camera's own blend rather than the frame's.
 */
function lengthOf(edge: Edge, ends: Ends, from: number, to: number): number {
  const last = to - 1;
  const pts = edge.wanted.frame.pts ?? new Float32Array(0);
  const run = runTo(pts, from, last);
  if (last < from) return run;
  const dx = (edge.wanted.x[ends.t] ?? 0) - (pts[2 * last] ?? 0);
  const dy = (edge.wanted.y[ends.t] ?? 0) - (pts[2 * last + 1] ?? 0);
  return run + Math.sqrt(dx * dx + dy * dy);
}

/** The two ends of an edge that is drawable, or null when it is not (see `paint3d.ts`). */
function endsOf(frame: Frame, depth: Float64Array, edge: number): Ends | null {
  const s = frame.source[edge] ?? 0;
  const t = frame.target[edge] ?? 0;
  const z = frame.z;
  if (z === null) return null;
  if ((depth[s] ?? 0) <= 0 || (depth[t] ?? 0) <= 0) return null;
  return { s, t, zs: z[s] ?? 0, zt: z[t] ?? 0 };
}

/**
 * The chord's midpoint, which stands in for an interior point whose invented z lands it
 * behind the eye — a path bowing past the camera. Without it the stroke would run to the
 * origin, which is on screen and looks like a bug rather than like the caveat.
 */
function middleOf(edge: Edge, ends: Ends): { x: number; y: number } {
  const { x, y } = edge.drawn;
  return {
    x: ((x[ends.s] ?? 0) + (x[ends.t] ?? 0)) / 2,
    y: ((y[ends.s] ?? 0) + (y[ends.t] ?? 0)) / 2,
  };
}

function projectEdge(edge: Edge, index: number, ends: Ends): void {
  const { out, wanted, setup } = edge;
  const { frame } = wanted;
  const pts = frame.pts ?? new Float32Array(0);
  const from = frame.offsets?.[index] ?? 0;
  const to = frame.offsets?.[index + 1] ?? 0;
  const total = lengthOf(edge, ends, from, to);
  const middle = middleOf(edge, ends);
  let run = 0;
  for (let p = from; p < to; p += 1) {
    if (p > from) run += chord(pts, p - 1, p);
    const z = ends.zs + (ends.zt - ends.zs) * (total > 0 ? run / total : 0);
    const found = project(setup.basis, { x: pts[2 * p] ?? 0, y: pts[2 * p + 1] ?? 0, z }, setup.focal, setup.centre);
    out[2 * p] = found?.x ?? middle.x;
    out[2 * p + 1] = found?.y ?? middle.y;
  }
}

/**
 * Every interior point of every edge, projected, into `out` — two entries per point, in the
 * frame's own point order, so the painter asks for the same index `frame.pts` holds. A fresh
 * array only when the one it was handed does not fit.
 */
export function interiorPoints(into: Float32Array | null, wanted: Projection, drawn: Drawn, setup: Setup): Float32Array {
  const { frame } = wanted;
  const count = (frame.pts?.length ?? 0) >> 1;
  const out = into !== null && into.length >= count * 2 ? into : new Float32Array(count * 2);
  out.fill(0);
  if (frame.pts === null || frame.offsets === null || frame.z === null) return out;
  const edge: Edge = { out, wanted, drawn, setup };
  for (let index = 0; index < frame.edgeCount; index += 1) {
    const ends = endsOf(frame, drawn.depth, index);
    if (ends !== null) projectEdge(edge, index, ends);
  }
  return out;
}

/**
 * Each node's own projected half width and half height in CSS pixels — two entries per node,
 * both zero for a frame with no w/h column or for a node behind the eye, as its `radius` is.
 */
export function boxHalfExtents(into: Float32Array | null, frame: Frame, depth: Float64Array, focal: number): Float32Array {
  const count = frame.nodeCount * 2;
  const out = into !== null && into.length >= count ? into : new Float32Array(count);
  for (let node = 0; node < frame.nodeCount; node += 1) {
    const at = node * 2;
    if (frame.w === null || frame.h === null || (depth[node] ?? 0) <= 0) {
      out[at] = 0;
      out[at + 1] = 0;
      continue;
    }
    const here = depth[node] ?? 1;
    out[at] = radiusOnScreen(focal, (frame.w[node] ?? 0) / 2, here);
    out[at + 1] = radiusOnScreen(focal, (frame.h[node] ?? 0) / 2, here);
  }
  return out;
}

/**
 * Pixels one world unit covers, as the drawing's own single number: `focal / depth` per node
 * in front of the eye, averaged. Summed in dense index order and not in `Drawn.order`,
 * because the order changes as the camera turns and a sum whose sequence changes is a
 * stroke width that jitters while the user drags. 0 for a drawing with nothing in front.
 */
export function pixelsPerUnit(drawn: Drawn, focal: number): number {
  let sum = 0;
  let count = 0;
  for (let node = 0; node < drawn.depth.length; node += 1) {
    const depth = drawn.depth[node] ?? 0;
    if (depth <= 0) continue;
    sum += focal / depth;
    count += 1;
  }
  return count > 0 ? sum / count : 0;
}