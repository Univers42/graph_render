/**
 * A node the user has dragged: a view-only override of its position. The columns the view
 * draws from are copied, so the frame the motor handed over is never written to; a new
 * layout replaces the whole scene and the override with it.
 *
 * Ponytail: the pick grid is never rebuilt until something picks, so a pointer move that
 * nobody picks in and nobody fits costs O(1); the first pick pays one O(n) pass over the
 * moved columns and every later pick on that scene reuses it. The bounds grow by the same
 * reach `sceneOf` uses, so a fit of a moved drawing still shows whole nodes.
 */
import type { Point } from "./camera.ts";
import { type Positions } from "./grid.ts";
import type { Gesture } from "./pointer.ts";
import type { Scene } from "./scene.ts";
import { deferredScene } from "./lazy.ts";

/**
 * The scene with these columns as the node positions; its bounds and grid are read on
 * demand, so a per-frame move does not scan the frame.
 */
export function movedScene(scene: Scene, positions: Positions): Scene {
  return deferredScene(scene, positions);
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
 * WHY the pin waits for the first move: a press that never travels is a click, and the click
 * path never calls `end` (it selects instead), so a pin taken on press would outlive the press
 * and leave the node nailed where the pointer first touched it — with the settle still running
 * around it, which moves every other node out from under the pointer too. The port is reserved
 * on press all the same, so a second pointer cannot take the node while this one is undecided.
 *
 * Ponytail: one pin per port; a multi-touch drag of two nodes is not supported.
 */
export function liveGesture(port: LiveDrag, node: number, world: (screen: Point) => Point): (Gesture & { cancel(): void }) | null {
  if (!port.enabled() || held.has(port)) return null;
  held.add(port);
  let open = true;
  let dragging = false;
  const stop = (): void => {
    if (!open) return;
    open = false;
    held.delete(port);
    if (dragging) port.release(node);
  };
  return {
    move: (to) => {
      if (!open) return;
      dragging = true;
      port.drag(node, world(to));
    },
    end: stop,
    cancel: stop,
  };
}
