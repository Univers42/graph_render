/**
 * What the view is showing: a frame, a style, and everything derived from the pair.
 * Derived state is rebuilt when its input changes and never per painted frame.
 */
import { type Adjacency, adjacencyOf } from "./adjacency.ts";
import type { Bounds } from "./camera.ts";
import type { Frame } from "./frame.ts";
import { type Grid, type Positions, gridOf, pickNode } from "./grid.ts";
import { type Style, plainStyle } from "./style.ts";

export interface Scene {
  readonly frame: Frame;
  readonly style: Style;
  readonly adjacency: Adjacency;
  readonly grid: Grid;
  /** World distance from a node's centre to its edge. */
  readonly extent: Float32Array;
  readonly reach: number;
  /** The frame's bounds grown by the largest node, so a fit shows whole nodes. */
  readonly bounds: Bounds | null;
}

export const EMPTY_FRAME: Frame = {
  nodeKind: "Point", edgeKind: "Line", nodeCount: 0, edgeCount: 0,
  x: new Float32Array(0), y: new Float32Array(0), z: null, r: null, w: null, h: null,
  source: new Uint32Array(0), target: new Uint32Array(0),
  curveDegree: 0, offsets: null, pts: null, bounds: null, factor: 1,
};

function extentOf(frame: Frame, style: Style): Float32Array {
  if (frame.r !== null) return frame.r;
  const { w, h } = frame;
  if (w === null || h === null) return style.radius;
  const extent = new Float32Array(frame.nodeCount);
  for (let i = 0; i < extent.length; i += 1) extent[i] = Math.max(w[i] ?? 0, h[i] ?? 0) / 2;
  return extent;
}

function grown(bounds: Bounds | null, by: number): Bounds | null {
  if (bounds === null) return null;
  return { minX: bounds.minX - by, minY: bounds.minY - by, maxX: bounds.maxX + by, maxY: bounds.maxY + by };
}

/**
 * The bounds a fit should use for these positions: the extent of the nodes themselves, grown
 * by `reach`. A fit over the bare positions crops the outermost nodes by their own radius,
 * which is the one thing a fit is supposed not to do.
 */
export function boundsOf(positions: Positions, reach: number): Bounds | null {
  const { x, y } = positions;
  if (x.length === 0) return null;
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (let i = 0; i < x.length; i += 1) {
    minX = Math.min(minX, x[i] ?? 0);
    maxX = Math.max(maxX, x[i] ?? 0);
    minY = Math.min(minY, y[i] ?? 0);
    maxY = Math.max(maxY, y[i] ?? 0);
  }
  return grown({ minX, minY, maxX, maxY }, reach);
}

/** A style made for another node count is replaced by the plain one, never half-read. */
export function sceneOf(frame: Frame, style: Style, previous: Scene | null): Scene {
  const fitting = style.nodeCount === frame.nodeCount ? style : plainStyle(frame.nodeCount);
  const extent = extentOf(frame, fitting);
  let reach = 0;
  for (let i = 0; i < extent.length; i += 1) reach = Math.max(reach, extent[i] ?? 0);
  const kept = previous !== null && previous.frame === frame ? previous : null;
  return {
    frame,
    style: fitting,
    adjacency: kept?.adjacency ?? adjacencyOf(frame.nodeCount, frame),
    grid: kept?.grid ?? gridOf(frame, frame.bounds),
    extent,
    reach,
    bounds: grown(frame.bounds, reach),
  };
}

function boxDistance(scene: Scene, node: number, dx: number, dy: number): number {
  const outX = Math.abs(dx) - (scene.frame.w?.[node] ?? 0) / 2;
  const outY = Math.abs(dy) - (scene.frame.h?.[node] ?? 0) / 2;
  if (outX <= 0 && outY <= 0) return Math.max(outX, outY);
  return Math.hypot(Math.max(outX, 0), Math.max(outY, 0));
}

export interface WorldQuery {
  readonly x: number;
  readonly y: number;
  readonly tolerance: number;
  /** World radius below which a disc is drawn, and so picked, at the floor size. */
  readonly floor: number;
}

export function pickIn(scene: Scene, query: WorldQuery): number {
  const { frame, style, extent } = scene;
  const boxes = frame.nodeKind === "Box";
  return pickNode(scene.grid, {
    x: query.x,
    y: query.y,
    tolerance: query.tolerance,
    reach: Math.max(scene.reach, query.floor),
    distance: (node, px, py) => {
      if (style.hidden?.[node] === 1) return Infinity;
      const dx = px - (frame.x[node] ?? 0);
      const dy = py - (frame.y[node] ?? 0);
      if (boxes) return boxDistance(scene, node, dx, dy);
      return Math.hypot(dx, dy) - Math.max(extent[node] ?? 0, query.floor);
    },
  });
}

/**
 * The layout switch in flight: the two columns the nodes are easing between, and how far along
 * it is. The same four columns and the same fraction the node shader mixes (`webgl2/layer.ts`,
 * `Tween`), so what is picked here is what is drawn there.
 */
export interface EasedPose {
  readonly fromX: Float32Array;
  readonly fromY: Float32Array;
  readonly toX: Float32Array;
  readonly toY: Float32Array;
  /** `easeInOutCubic` of the tween's clock: 0 at the `from` columns, 1 at the `to` columns. */
  readonly eased: number;
}

/** One node's eased coordinate, mixed in the order `transition.blend` mixes it. */
function easedAt(pose: EasedPose, from: Float32Array, to: Float32Array, node: number): number {
  const start = from[node] ?? 0;
  return start + ((to[node] ?? 0) - start) * pose.eased;
}

/**
 * The node under the point while a tween is in flight, by one linear scan over the eased pose.
 *
 * The grid cannot answer this: it is built from the target frame, and every node is somewhere
 * else until the tween ends. A scan is the honest cost here — a hover runs it at most once a
 * frame, and it is 4 reads and 4 flops a node with no column allocated and nothing to free.
 *
 * The tie rule is `pickNode`'s: a tie goes to the later index, the node painted last is on top.
 */
export function pickEased(scene: Scene, query: WorldQuery, pose: EasedPose): number {
  const { frame, style, extent } = scene;
  const boxes = frame.nodeKind === "Box";
  // Past this the node cannot be a hit whatever its other axis says: the widest disc in the
  // frame plus the tolerance, which is the same `span` `pickNode` spans its cells with. One
  // subtraction rejects almost every node before the second column is read at all, and at
  // 200 000 nodes it is most of what the scan costs (tests/bench-tween.ts).
  const span = query.tolerance + Math.max(scene.reach, query.floor);
  let best = -1;
  let bestDistance = query.tolerance;
  for (let node = 0; node < frame.nodeCount; node += 1) {
    const dx = query.x - easedAt(pose, pose.fromX, pose.toX, node);
    if (dx > span || dx < -span) continue;
    if (style.hidden?.[node] === 1) continue;
    const dy = query.y - easedAt(pose, pose.fromY, pose.toY, node);
    const distance = boxes
      ? boxDistance(scene, node, dx, dy)
      : Math.hypot(dx, dy) - Math.max(extent[node] ?? 0, query.floor);
    if (distance < bestDistance || (distance === bestDistance && node > best)) {
      best = node;
      bestDistance = distance;
    }
  }
  return best;
}
