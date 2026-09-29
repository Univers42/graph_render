/**
 * A node the user has dragged: a view-only override of its position. The columns the view
 * draws from are copied, so the frame the motor handed over is never written to; a new
 * layout replaces the whole scene and the override with it.
 *
 * Ponytail: the pick grid is rebuilt over all nodes, O(n), once when the drag ends and not
 * per pointer move (nothing is picked while a drag holds the pointer). The bounds used for
 * fitting are the layout's, so a fit after a drag ignores the moved node.
 */
import type { Bounds, Point } from "./camera.ts";
import { type Positions, gridOf } from "./grid.ts";
import type { Gesture } from "./pointer.ts";
import type { Scene } from "./scene.ts";

function boundsAround(positions: Positions): Bounds | null {
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
  return { minX, minY, maxX, maxY };
}

/** The scene with these columns as the node positions; its grid is rebuilt to match. */
export function movedScene(scene: Scene, positions: Positions): Scene {
  return {
    ...scene,
    frame: { ...scene.frame, x: positions.x, y: positions.y },
    grid: gridOf(positions, boundsAround(positions)),
  };
}

/** The motor's live session as the view sees it: positions are world coordinates. */
export interface LiveDrag {
  /** False while no session exists; the view-only drag then applies. */
  enabled(): boolean;
  drag(node: number, at: Point): void;
  release(node: number): void;
}

const held = new WeakSet<LiveDrag>();

/**
 * A drag that pins `node` in the motor under the pointer. Null when the port is disabled or
 * already holds a node, so a second pointer never takes a second pin.
 *
 * Ponytail: one pin per port; a multi-touch drag of two nodes is not supported.
 */
export function liveGesture(port: LiveDrag, node: number, world: (screen: Point) => Point, from: Point): (Gesture & { cancel(): void }) | null {
  if (!port.enabled() || held.has(port)) return null;
  held.add(port);
  let open = true;
  port.drag(node, world(from));
  const stop = (): void => {
    if (!open) return;
    open = false;
    held.delete(port);
    port.release(node);
  };
  return {
    move: (to) => {
      if (open) port.drag(node, world(to));
    },
    end: stop,
    cancel: stop,
  };
}
