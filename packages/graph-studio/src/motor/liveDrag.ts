/**
 * The view's live-drag port over the worker: a pinned node becomes `force.drag` with the
 * node's stable id and its world position, a release becomes `force.release`.
 *
 * Ponytail: `enabled` is read at each press, so a session that appears mid-drag takes effect
 * on the next press, and one that vanishes mid-drag still gets its release dropped.
 */
import type { LiveDrag } from "../../../graph-render/src/drag.ts";
import type { ForceRequest } from "./protocol.ts";

export interface LiveDragDeps {
  /** Node ids in dense order, or null before a graph is loaded. */
  readonly ids: () => readonly string[] | null;
  /** Why the live simulation cannot run, or null. */
  readonly disabled: () => string | null;
  readonly send: (request: ForceRequest) => void;
}

export function createLiveDrag(deps: LiveDragDeps): LiveDrag {
  const idOf = (node: number): string | null => deps.ids()?.[node] ?? null;
  return {
    enabled: () => deps.disabled() === null,
    drag: (node, at) => {
      const id = idOf(node);
      if (deps.disabled() === null && id !== null) deps.send({ type: "force.drag", id, x: at.x, y: at.y });
    },
    release: (node) => {
      const id = idOf(node);
      if (deps.disabled() === null && id !== null) deps.send({ type: "force.release", id });
    },
  };
}
