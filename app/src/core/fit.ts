/**
 * Fit-to-view. The camera every run opens with, and the only place the studio
 * invents a number the engine did not produce.
 *
 * It reuses the engine's zoom clamps and its `clamp` (`src/core/camera/transform.ts`,
 * `src/core/math.ts` — both dependency-free, so they load under a plain
 * `node --test`) but frames the box itself rather than importing
 * `camera/controls.ts`: that module reaches its sibling through an EXTENSIONLESS
 * specifier, which Vite resolves and Node's ESM resolver does not, and these unit
 * tests run under plain `node --test --experimental-strip-types` with no loader.
 * The framing arithmetic is the engine's `fitBounds`, restated.
 */

import { clamp } from "../../../src/core/math.ts";
import {
  IDENTITY,
  MAX_ZOOM,
  MIN_ZOOM,
  type Camera,
  type WorldBounds,
} from "../../../src/core/camera/transform.ts";
import type { NodeDraw } from "./drawList.ts";

export type { NodeDraw };

export { MAX_ZOOM, MIN_ZOOM };

/** Screen-space margin left around the framed box, in CSS pixels. */
export const FIT_PADDING = 64;

/** The tight world box over every node, grown by half each node's extent so a
 *  Box is framed whole. `null` for no nodes — an empty box at the origin would
 *  frame nothing and read as "the graph is at 0,0". */
export function boundsOf(nodes: readonly NodeDraw[]): WorldBounds | null {
  if (nodes.length === 0) return null;
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const node of nodes) {
    minX = Math.min(minX, node.x - node.w / 2);
    minY = Math.min(minY, node.y - node.h / 2);
    maxX = Math.max(maxX, node.x + node.w / 2);
    maxY = Math.max(maxY, node.y + node.h / 2);
  }
  return { minX, minY, maxX, maxY };
}

/** The camera that frames `bounds` in a `width`×`height` viewport, or the
 *  identity camera when there is nothing to frame. A box with no extent (one
 *  node, or every node a Point) is framed at the zoom ceiling rather than
 *  dividing by zero. */
export function fitCamera(
  bounds: WorldBounds | null,
  width: number,
  height: number,
  padding = FIT_PADDING,
): Camera {
  if (bounds === null) return { ...IDENTITY };
  const worldW = Math.max(1, bounds.maxX - bounds.minX);
  const worldH = Math.max(1, bounds.maxY - bounds.minY);
  const scale = clamp(
    Math.min((width - padding * 2) / worldW, (height - padding * 2) / worldH),
    MIN_ZOOM,
    MAX_ZOOM,
  );
  const centerX = (bounds.minX + bounds.maxX) / 2;
  const centerY = (bounds.minY + bounds.maxY) / 2;
  return { scale, x: width / 2 - centerX * scale, y: height / 2 - centerY * scale };
}
