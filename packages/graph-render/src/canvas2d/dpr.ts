/**
 * Device pixel ratio watcher: re-measures the view when the browser's reported DPR changes.
 *
 * A window moved to a screen of another density, or a browser zoom, changes
 * `devicePixelRatio` without a resize event. The renderer allocates its backing store
 * from `measure()`, which is called on `ResizeObserver`; that does not fire for a DPR
 * change alone. This module listens to `matchMedia(\`(resolution: \${dpr}dppx)\`)` and
 * re-measures when it fires.
 *
 * The listener is armed once per view, and re-arms itself after each change so a
 * sequence of density changes (1 → 1.5 → 2 → 1) is all caught.
 *
 * Ponytail: the `matchMedia` change event is the browser's signal that the DPR changed,
 * but the spec does not guarantee it fires for every DPR change (e.g., some embedded
 * views, or a browser that batches density changes). If it misses one, the backing store
 * stays at the old DPR until a resize or the next caught change. The query is built at
 * arm time from the current `devicePixelRatio`, so a rapid 1→2→1 sequence could leave
 * the media query matching the wrong value between arms; the re-arm is scheduled after
 * the callback, so the window is narrow. A full fix would poll `devicePixelRatio` on a
 * timer as a fallback.
 * Ponytail: the re-measure logic here duplicates `measure()` from controller.ts because
 * that function is not exported. If `measure()` changes (e.g., MAX_DPR, sprite reset),
 * this copy must be updated in lockstep.
 */
import type { Controller } from "./controller.ts";
import { invalidate } from "./loop.ts";

const armed = new WeakMap<Controller, true>();

function arm(controller: Controller): void {
  if (armed.has(controller)) return;
  armed.set(controller, true);

  const remeasure = (): void => {
    armed.delete(controller);
    const { canvas, state } = controller;
    const box = canvas.getBoundingClientRect();
    const nextDpr = Math.min(2, globalThis.devicePixelRatio || 1);
    if (nextDpr !== state.dpr || box.width !== state.viewport.width || box.height !== state.viewport.height) {
      // Re-measure will be called by the ResizeObserver if the box changed, but for a
      // pure DPR change we must call it here. The controller's measure() is not
      // exported, so we replicate the logic that matters: update dpr, viewport, canvas
      // size, sprite cache, and invalidate.
      state.dpr = nextDpr;
      state.viewport = { width: Math.max(1, box.width), height: Math.max(1, box.height) };
      canvas.width = Math.round(state.viewport.width * state.dpr);
      canvas.height = Math.round(state.viewport.height * state.dpr);
      state.sprites.reset(state.theme, state.dpr);
    }
    invalidate(controller.state);
    // Re-arm for the next DPR change
    arm(controller);
  };

  // The media query that matches the current DPR. When it stops matching, the DPR
  // has changed. We query at the current DPR and listen for the change.
  const query = `(resolution: ${globalThis.devicePixelRatio || 1}dppx)`;
  const mql = globalThis.matchMedia(query);
  mql.addEventListener("change", remeasure, { once: true });
}

/** Arms the DPR watcher for the given controller. Idempotent. */
export function watchDpr(controller: Controller): void {
  arm(controller);
}