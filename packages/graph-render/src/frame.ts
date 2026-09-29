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

/** Copies every column out of the snapshot's buffer, so the bytes can be dropped. */
export function frameFrom(snapshot: Snapshot): Frame {
  const factor = worldFactor(boundsOf(snapshot.x, snapshot.y), snapshot.nodeCount);
  const x = scaled(snapshot.x, factor);
  const y = scaled(snapshot.y, factor);
  return {
    nodeKind: snapshot.nodeKind,
    edgeKind: snapshot.edgeKind,
    nodeCount: snapshot.nodeCount,
    edgeCount: snapshot.edgeCount,
    x,
    y,
    r: scaledOrNull(snapshot.r, factor),
    w: scaledOrNull(snapshot.w, factor),
    h: scaledOrNull(snapshot.h, factor),
    source: snapshot.source.slice(),
    target: snapshot.target.slice(),
    curveDegree: snapshot.curveDegree,
    offsets: snapshot.offsets === null ? null : snapshot.offsets.slice(),
    pts: scaledOrNull(snapshot.pts, factor),
    bounds: boundsOf(x, y),
    factor,
  };
}
