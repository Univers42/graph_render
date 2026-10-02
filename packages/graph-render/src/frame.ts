/**
 * One layout run in the renderer's world units, as flat columns. The motor lays a graph
 * out in its own units (a grid cell is 1); the painter's sizes are pixel-scale, so one
 * uniform factor per run brings the typical node spacing to TARGET_SPACING. Uniform, so
 * the geometry stays exactly similar.
 */
import type { Bounds } from "./camera.ts";
import type { EdgeKind, NodeKind, Snapshot } from "./snapshot/decode.ts";

export const TARGET_SPACING = 56;

export interface Frame {
  readonly nodeKind: NodeKind;
  readonly edgeKind: EdgeKind;
  readonly nodeCount: number;
  readonly edgeCount: number;
  readonly x: Float32Array;
  readonly y: Float32Array;
  /**
   * The third coordinate, in world units; `null` for a 2D layout. Its presence is the only
   * thing that tells a 3D frame from a 2D one, and the painter asks for nothing else.
   */
  readonly z: Float32Array | null;
  /** Circle radius. `null` for Point and Box: a Point's radius is a style. */
  readonly r: Float32Array | null;
  readonly w: Float32Array | null;
  readonly h: Float32Array | null;
  readonly source: Uint32Array;
  readonly target: Uint32Array;
  readonly curveDegree: number;
  readonly offsets: Uint32Array | null;
  readonly pts: Float32Array | null;
  /** Over node centres; `null` for no nodes. */
  readonly bounds: Bounds | null;
  /** Motor units to world units. */
  readonly factor: number;
}

function boundsOf(x: Float32Array, y: Float32Array): Bounds | null {
  if (x.length === 0) return null;
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (let i = 0; i < x.length; i += 1) {
    const px = x[i] ?? 0;
    const py = y[i] ?? 0;
    if (px < minX) minX = px;
    if (px > maxX) maxX = px;
    if (py < minY) minY = py;
    if (py > maxY) maxY = py;
  }
  return { minX, minY, maxX, maxY };
}

/**
 * The bounds a fit uses: the node hull grown by every interior edge vertex, in world units.
 * A `Polyline` or `Curve` vertex outside the node hull is otherwise invisible by
 * construction — the bounds a fit reads do not name it, so the fit cannot see it and a
 * routed link runs off the frame.
 *
 * The world factor is deliberately NOT taken from this hull (see `frameFrom`): the factor is
 * the uniform scale of the whole drawing, so widening this box must not rescale a layout.
 */
function hullWith(bounds: Bounds | null, pts: Float32Array | null): Bounds | null {
  if (bounds === null || pts === null) return bounds;
  let minX = bounds.minX;
  let minY = bounds.minY;
  let maxX = bounds.maxX;
  let maxY = bounds.maxY;
  // The column is x,y interleaved, one pair per interior vertex, in edge order (decode.ts).
  for (let i = 0; i + 1 < pts.length; i += 2) {
    const px = pts[i] ?? 0;
    const py = pts[i + 1] ?? 0;
    if (px < minX) minX = px;
    if (px > maxX) maxX = px;
    if (py < minY) minY = py;
    if (py > maxY) maxY = py;
  }
  return { minX, minY, maxX, maxY };
}

/**
 * Ponytail: "typical spacing" is sqrt(bounding-box area / n), the spacing of a uniform
 * spread. One dense clump with a few far outliers reads as sparse and is under-scaled
 * (the clump overlaps); a collinear layout falls back to extent / (n - 1). Zooming in is
 * the escape hatch.
 */
export function worldFactor(bounds: Bounds | null, nodeCount: number): number {
  if (bounds === null || nodeCount < 2) return 1;
  const width = bounds.maxX - bounds.minX;
  const height = bounds.maxY - bounds.minY;
  const spacing = width * height > 0
    ? Math.sqrt((width * height) / nodeCount)
    : Math.max(width, height) / (nodeCount - 1);
  return spacing > 0 && Number.isFinite(spacing) ? TARGET_SPACING / spacing : 1;
}

function scaled(column: Float32Array, factor: number): Float32Array {
  const out = new Float32Array(column.length);
  for (let i = 0; i < column.length; i += 1) out[i] = (column[i] ?? 0) * factor;
  return out;
}

function scaledOrNull(column: Float32Array | null, factor: number): Float32Array | null {
  return column === null ? null : scaled(column, factor);
}

/**
 * Copies every column out of the snapshot's buffer, so the bytes can be dropped. The z
 * column is scaled by the same factor as x and y and kept or dropped with it, so a 3D frame
 * is the 2D frame of the same drawing plus one column and nothing else.
 */
export function frameFrom(snapshot: Snapshot): Frame {
  // The node hull alone, and only the node hull: the factor is the uniform scale of the whole
  // drawing, so an edge that swings far outside must not rescale the layout around it.
  const factor = worldFactor(boundsOf(snapshot.x, snapshot.y), snapshot.nodeCount);
  const x = scaled(snapshot.x, factor);
  const y = scaled(snapshot.y, factor);
  const pts = scaledOrNull(snapshot.pts, factor);
  return {
    nodeKind: snapshot.nodeKind,
    edgeKind: snapshot.edgeKind,
    nodeCount: snapshot.nodeCount,
    edgeCount: snapshot.edgeCount,
    x,
    y,
    z: scaledOrNull(snapshot.z, factor),
    r: scaledOrNull(snapshot.r, factor),
    w: scaledOrNull(snapshot.w, factor),
    h: scaledOrNull(snapshot.h, factor),
    source: snapshot.source.slice(),
    target: snapshot.target.slice(),
    curveDegree: snapshot.curveDegree,
    offsets: snapshot.offsets === null ? null : snapshot.offsets.slice(),
    pts,
    bounds: hullWith(boundsOf(x, y), pts),
    factor,
  };
}
