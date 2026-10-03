/**
 * The two halves of the studio's chrome: whether the user has taken the camera over, and how much
 * of the canvas a fit may draw into.
 *
 * They live together because they are the same question asked twice — "is this camera still the
 * view's own?" — and because the studio's chrome resizes for reasons that have nothing to do with
 * the drawing. A selection filling the Inspector is the one that mattered: re-fitting the camera
 * on each of those took whatever the user was pointing at out from under the pointer, so a box
 * select ended up measuring a screen-space box the camera had already left
 * (`canvas2d/controller.ts` `setSafeArea`).
 */
import type { FitArea, Viewport } from "./camera.ts";
import type { Controller } from "./canvas2d/controller.ts";
import type { LoopState } from "./canvas2d/loop.ts";

/**
 * Wraps a pointer handler so the gesture is recorded before it runs, and keeps whatever the
 * handler returns: `press` hands back the gesture the drag will finish with, and dropping it
 * would turn every node drag into a pan.
 */
export function taken<A extends unknown[], R>(controller: Controller, run: (...args: A) => R): (...args: A) => R {
  return (...args: A): R => {
    controller.gestured = true;
    return run(...args);
  };
}

/**
 * The part of the canvas a fit draws into: what the host declared as free of chrome, clamped to
 * the canvas itself and to at least a third of it. A host whose safe area has not been measured
 * yet says nothing, and `null` is the whole canvas.
 */
export function safeOf(state: LoopState, viewport: Viewport): FitArea | null {
  const wanted = state.safe;
  if (wanted === null) return null;
  const width = Math.max(1, Math.min(wanted.width, viewport.width));
  const height = Math.max(1, Math.min(wanted.height, viewport.height));
  return { x: wanted.x, y: wanted.y, width, height };
}