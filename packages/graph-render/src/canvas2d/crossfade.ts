/**
 * A change the nodes cannot move through — a restyle, a theme, another document — fades from the
 * picture that was on screen instead of cutting to the new one. `startCrossFade` copies the canvas
 * as it is; the next repaints draw the new scene and lay the copy over it, more transparent each
 * frame, until it is gone after `CROSSFADE_MS`.
 *
 * Caveat: the copy is screen pixels, so a pan, a zoom or a node move during the fade slides the new
 * drawing under a ghost that stays put, for at most `CROSSFADE_MS`; a resize drops the ghost at once.
 * Without `OffscreenCanvas` (an old engine, node) the change cuts, as it did before. Each fade frame
 * repaints the scene in full, so a scene whose frame costs more than 16 ms fades in fewer steps.
 */
import { easeInOutCubic } from "../transition.ts";
import type { LoopState } from "./loop.ts";

/** Long enough to read as a fade, short enough that a click still feels answered. */
export const CROSSFADE_MS = 220;

export interface CrossFade {
  readonly canvas: OffscreenCanvas;
  /** When the copy was first painted over the new scene, or -1 while no paint has run since the copy. */
  start: number;
  readonly ms: number;
}

/** Copies what is on screen to fade it out over the next `ms`; nothing when the engine cannot copy. */
export function startCrossFade(state: LoopState, ms: number = CROSSFADE_MS): void {
  // Two changes before one paint: the screen still shows the picture already copied.
  if (state.crossFade !== null && state.crossFade.start < 0) return;
  const source = state.ctx.canvas;
  if (typeof OffscreenCanvas === "undefined" || !(ms > 0) || source.width === 0 || source.height === 0) return;
  const canvas = new OffscreenCanvas(source.width, source.height);
  const ctx = canvas.getContext("2d");
  if (ctx === null) return;
  ctx.drawImage(source, 0, 0);
  state.crossFade = { canvas, start: -1, ms };
}

/** Lays the copy over the scene just painted, at the opacity the fade has reached; drops it once gone. */
export function paintCrossFade(state: LoopState, now: number): void {
  const ghost = state.crossFade;
  if (ghost === null) return;
  const { ctx } = state;
  if (ghost.canvas.width !== ctx.canvas.width || ghost.canvas.height !== ctx.canvas.height) {
    state.crossFade = null;
    return;
  }
  // The clock starts on the first paint, so the time the change itself took is not fade time.
  if (ghost.start < 0) ghost.start = now;
  const t = (now - ghost.start) / ghost.ms;
  if (t >= 1) {
    state.crossFade = null;
    return;
  }
  ctx.save();
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.globalAlpha = 1 - easeInOutCubic(t);
  ctx.drawImage(ghost.canvas, 0, 0);
  ctx.restore();
}

/** True while a copy is still to be faded out, so the loop asks for another frame. */
export function crossFading(state: Pick<LoopState, "crossFade">): boolean {
  return state.crossFade !== null;
}
