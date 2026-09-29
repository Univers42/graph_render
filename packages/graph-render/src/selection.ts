/** The pure part of selecting: which nodes a box holds, and how a shift-click adds. */
import type { Bounds } from "./camera.ts";
import type { Positions } from "./grid.ts";

export function boxOf(a: { readonly x: number; readonly y: number }, b: { readonly x: number; readonly y: number }): Bounds {
  return { minX: Math.min(a.x, b.x), minY: Math.min(a.y, b.y), maxX: Math.max(a.x, b.x), maxY: Math.max(a.y, b.y) };
}

/**
 * Node centres on or inside the box, hidden nodes left out.
 * Ponytail: a centre test, so a big node that overlaps the box without its centre in it is
 * not selected; and a scan of every node, once per box, not per pointer move.
 */
export function nodesInBox(positions: Positions, hidden: Uint8Array | null, box: Bounds): number[] {
  const inside: number[] = [];
  for (let node = 0; node < positions.x.length; node += 1) {
    if (hidden?.[node] === 1) continue;
    const x = positions.x[node] ?? 0;
    const y = positions.y[node] ?? 0;
    if (x >= box.minX && x <= box.maxX && y >= box.minY && y <= box.maxY) inside.push(node);
  }
  return inside;
}

export function addTo(selection: readonly number[], node: number): number[] {
  if (node < 0 || selection.includes(node)) return [...selection];
  return [...selection, node];
}
