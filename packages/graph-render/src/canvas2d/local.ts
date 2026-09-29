/** The local graph: what the view does when it is asked to show a node's neighbourhood only. */
import { bfsMask } from "../adjacency.ts";
import { type LocalOptions, boundsOfVisible, idsOf, withLocalHidden } from "../local.ts";
import { sceneOf } from "../scene.ts";
import type { Style } from "../style.ts";
import { type Controller, fit, select } from "./controller.ts";
import { invalidate, relight } from "./loop.ts";

/** Rebuilds the scene's style from the host's style and the layer, and asks for a paint. */
export function applyLayer(controller: Controller): void {
  const { state, local } = controller;
  const base = local.base ?? state.scene.style;
  state.scene = sceneOf(state.scene.frame, withLocalHidden(base, local.visible), state.scene);
  relight(state);
  invalidate(state);
}

/** The host's style replaces the base; the layer keeps hiding what is outside the set. */
export function setBaseStyle(controller: Controller, style: Style): void {
  controller.local.base = style;
  applyLayer(controller);
}

/** Shows `node` and what a walk of `options` reaches, fitted; returns those nodes ascending. */
export function showLocal(controller: Controller, node: number, options: LocalOptions): readonly number[] {
  const { state, local } = controller;
  const visible = bfsMask(state.scene.adjacency, node, options.depth, options);
  const ids = idsOf(visible);
  if (ids.length === 0) return ids;
  local.base ??= state.scene.style;
  local.visible = visible;
  local.bounds = boundsOfVisible(state.scene.frame, visible, state.scene.extent);
  applyLayer(controller);
  select(controller, node);
  fit(controller);
  return ids;
}

/** Back to the whole graph, fitted. Does nothing when no local graph is shown. */
export function showAll(controller: Controller): void {
  const { local } = controller;
  if (local.visible === null) return;
  local.visible = null;
  local.bounds = null;
  applyLayer(controller);
  fit(controller);
}

/**
 * Called after a new frame is shown. A frame of another size drops the set (its indices mean
 * other nodes now); one of the same size keeps it and refits the bounds to the new positions.
 */
export function rebaseLocal(controller: Controller): void {
  const { local, state } = controller;
  if (local.visible === null) return;
  if (local.visible.length !== state.scene.frame.nodeCount) {
    local.visible = null;
    local.bounds = null;
    local.base = null;
    return;
  }
  local.bounds = boundsOfVisible(state.scene.frame, local.visible, state.scene.extent);
  applyLayer(controller);
}
