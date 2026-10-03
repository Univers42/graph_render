/**
 * Whether the user has taken the camera over. The safe area a fit draws into is the other half of
 * the same question — "is this camera still the view's own?" — and lives in `canvas2d/limits.ts`,
 * next to the limits it builds. The studio's chrome resizes for reasons that have nothing to do
 * with the drawing. A selection filling the Inspector is the one that mattered: re-fitting the camera
 * on each of those took whatever the user was pointing at out from under the pointer, so a box
 * select ended up measuring a screen-space box the camera had already left
 * (`canvas2d/controller.ts` `setSafeArea`).
 */
import type { Controller } from "./canvas2d/controller.ts";

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

