/** View-only marks on nodes: pinned, and hidden. Neither reaches the motor or the frame. */
import type { Controller } from "./controller.ts";
import { invalidate, relight } from "./loop.ts";
import { type Scene, sceneOf } from "../scene.ts";
import { setSelection } from "./choose.ts";

/** The pinned list with `node` added, or removed if it was there; out-of-range nodes change nothing. */
export function toggled(pinned: readonly number[], node: number, count: number): readonly number[] {
  if (node < 0 || node >= count) return pinned;
  return pinned.includes(node) ? pinned.filter((at) => at !== node) : [...pinned, node];
}

/**
 * The scene with these nodes hidden. The style is copied, so the one the host set is not
 * written to; the frame and so the grid are kept.
 */
export function withHidden(scene: Scene, nodes: readonly number[]): Scene {
  const count = scene.frame.nodeCount;
  const hidden = (scene.style.hidden ?? new Uint8Array(count)).slice();
  for (const node of nodes) if (node >= 0 && node < count) hidden[node] = 1;
  return sceneOf(scene.frame, { ...scene.style, hidden }, scene);
}

export function togglePin(controller: Controller, node: number): void {
  const { state } = controller;
  state.pinned = toggled(state.pinned, node, state.scene.frame.nodeCount);
  invalidate(state);
}

/**
 * Takes nodes out of the drawing and out of picking.
 * Ponytail: a later `setStyle` from the host replaces the hidden column and brings the nodes
 * back, and a new frame of the same size keeps them hidden.
 */
export function hideNodes(controller: Controller, nodes: readonly number[]): void {
  const { state } = controller;
  state.scene = withHidden(state.scene, nodes);
  const hidden = state.scene.style.hidden;
  const gone = (node: number): boolean => hidden?.[node] === 1;
  state.pinned = state.pinned.filter((node) => !gone(node));
  if (gone(state.hovered)) state.hovered = -1;
  setSelection(controller, state.selection.filter((node) => !gone(node)));
  relight(state);
  invalidate(state);
}
