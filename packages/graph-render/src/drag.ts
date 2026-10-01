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
import { type Scene, boundsOf } from "./scene.ts";

/** The scene with these columns as the node positions; its grid is rebuilt to match. */
export function movedScene(scene: Scene, positions: Positions): Scene {
  // The bounds grow by the same reach `sceneOf` uses, so a fit of a moved drawing still
  // shows whole nodes: over the bare positions the outermost ones are cropped by their radius.
  const bounds: Bounds | null = boundsOf(positions, scene.reach);
  return {
    ...scene,
    frame: { ...scene.frame, x: positions.x, y: positions.y },
    grid: gridOf(positions, bounds),
    bounds,
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
