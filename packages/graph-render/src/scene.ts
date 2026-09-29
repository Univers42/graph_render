/**
 * What the view is showing: a frame, a style, and everything derived from the pair.
 * Derived state is rebuilt when its input changes and never per painted frame.
 */
import { type Adjacency, adjacencyOf } from "./adjacency.ts";
import type { Bounds } from "./camera.ts";
import type { Frame } from "./frame.ts";
import { type Grid, gridOf, pickNode } from "./grid.ts";
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
  x: new Float32Array(0), y: new Float32Array(0), r: null, w: null, h: null,
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
