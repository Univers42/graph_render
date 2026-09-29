/**
 * One frame's worth of geometry: where every node is at progress `t` between the
 * previous run and this one, and which edges carry their own interpolated path.
 *
 * Pure, because the transition's two awkward cases live here and both are worth
 * pinning: a first run has nothing to come FROM, and two layouts rarely agree on
 * node or point counts. Those cross-fade (alpha `t`, target geometry) instead of
 * morphing mismatched arrays into nonsense.
 */

import type { DrawList } from "./drawList.ts";
import { interpolatePath, lerpPositions } from "./transition.ts";

export interface Frame {
  /** World x per node, for THIS frame. */
  readonly x: Float32Array;
  /** World y per node, for this frame. */
  readonly y: Float32Array;
  /** 1 normally; `t` while a mismatched pair cross-fades. */
  readonly alpha: number;
  /** Per-edge interpolated point run, or `null` to draw the edge from the node
   *  positions (every `Line`, and any path that could not be matched). */
  readonly paths: readonly (Float32Array | null)[];
  /** True when the node positions really are interpolated (same node count). */
  readonly morph: boolean;
}

/** Node positions of a list, copied: a `DrawList` is immutable, and the frame
 *  buffers are written into every animation frame. */
function positionsOf(list: DrawList): { x: Float32Array; y: Float32Array } {
  const x = new Float32Array(list.nodes.length);
  const y = new Float32Array(list.nodes.length);
  for (const node of list.nodes) {
    x[node.index] = node.x;
    y[node.index] = node.y;
  }
  return { x, y };
}

/** Interpolated points for one edge, or `null` to fall back to the endpoints.
 *  A kind change needs no check of its own: a `Line` carries no points at all
 *  (the contract's `EdgeOffsets`/`EdgePts` columns are absent for it), so the
 *  point-count guard in {@link interpolatePath} already refuses to morph a
 *  straight line into a routed one. */
function pathFor(to: DrawList, from: DrawList | null, index: number, t: number): Float32Array | null {
  if (from === null || to.edgeKind === "Line") return null;
  const target = to.edges[index];
  const previous = from.edges[index];
  if (target === undefined || previous === undefined) return null;
  return interpolatePath(previous.pts, target.pts, t);
}

/** A `Polyline`/`Curve` edge's own point run, which is its geometry rather than
 *  an interpolation result — a first run draws it as it stands. A `Line` carries
 *  no points at all, so it stays `null` and is drawn from its two endpoints. */
function ownPath(edge: DrawList["edges"][number] | undefined): Float32Array | null {
  if (edge === undefined || edge.pts.length < 4) return null;
  return edge.pts;
}

export function frameFor(to: DrawList, from: DrawList | null, t: number): Frame {
  const target = positionsOf(to);
  if (from === null) {
    return { ...target, alpha: 1, paths: to.edges.map(ownPath), morph: false };
  }
  const same = from.nodes.length === to.nodes.length;
  const alpha = same ? 1 : t;
  const x = same ? lerpPositions(positionsOf(from).x, target.x, t) : target.x;
  const y = same ? lerpPositions(positionsOf(from).y, target.y, t) : target.y;
  return { x, y, alpha, paths: to.edges.map((_, i) => pathFor(to, from, i, t)), morph: same };
}
