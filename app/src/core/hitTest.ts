/**
 * The two pure queries behind the pointer: which node is under the cursor, and
 * which nodes are one edge away from the one that was clicked.
 *
 * Both walk dense indices, never an id-keyed Map, so the answer for a given
 * snapshot is the same on every machine (D3/D4's spirit, applied to the studio):
 * the topmost node is the LAST index drawn, and a neighbour set is ascending.
 */

import { DrawListError, type DrawList, type NodeDraw } from "./drawList.ts";
import type { NodeGeometryKind } from "../../../crates/graph-sdk-js/src/index.ts";

/** A world-space point. */
export interface Point {
  readonly x: number;
  readonly y: number;
}

/** How forgiving the hit test is: `tolerancePx` is SCREEN pixels, converted
 *  through the camera scale so the grab area stays the same size on screen at
 *  any zoom. */
export interface HitOptions {
  readonly scale: number;
  readonly tolerancePx: number;
}

/** Inside a `Box` node's rectangle? Anywhere else, inside its radius. The KIND
 *  decides, not the extent: a Circle carries `w === h === 2r` too, and testing
 *  the extent alone would make every circle a rectangle. */
function contains(kind: NodeGeometryKind, node: NodeDraw, x: number, y: number, slack: number): boolean {
  if (kind === "Box") {
    return Math.abs(x - node.x) <= node.w / 2 + slack && Math.abs(y - node.y) <= node.h / 2 + slack;
  }
  const dx = x - node.x;
  const dy = y - node.y;
  const reach = node.r + slack;
  return dx * dx + dy * dy <= reach * reach;
}

/** The dense index of the node under a world point, or -1. The LAST hit wins:
 *  that is the node drawn on top, which is the one the user aimed at. */
export function hitTestNode(list: DrawList, world: Point, options: HitOptions): number {
  if (list.nodes.length === 0) throw new DrawListError("nothing to hit-test: the run has no nodes");
  const slack = options.tolerancePx / options.scale;
  for (let i = list.nodes.length - 1; i >= 0; i -= 1) {
    if (contains(list.nodeKind, list.nodes[i], world.x, world.y, slack)) return i;
  }
  return -1;
}

/** The dense indices one edge away from `index`, ascending. A self edge does
 *  not make a node its own neighbour. */
export function neighborsOf(list: DrawList, index: number): Set<number> {
  const found = new Set<number>();
  for (const edge of list.edges) {
    if (edge.source === index) found.add(edge.target);
    else if (edge.target === index) found.add(edge.source);
  }
  found.delete(index);
  return new Set([...found].sort((a, b) => a - b));
}

/** The `which`-th neighbour in ascending order, or `null` past the end. */
export function pickNeighbour(neighbors: ReadonlySet<number>, which: number): number | null {
  if (which < 0) return null;
  const sorted = [...neighbors].sort((a, b) => a - b);
  return sorted[which] ?? null;
}
