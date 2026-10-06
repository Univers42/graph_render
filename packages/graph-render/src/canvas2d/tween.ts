/**
 * The layout tween of the 2D view: the eased pose between two frames, and the node budget over
 * which the 2D painter snaps instead of easing.
 */
import type { Camera } from "../camera.ts";
import { TRANSITION_MS, blend, easeInOutCubic, markTween } from "../transition.ts";
import type { PaintCounts } from "./input.ts";
import type { LoopState } from "./loop.ts";

/**
 * The node count from which the 2D painter stops easing a layout switch and snaps to it.
 *
 * Measured, not guessed (docs/measurements/perf-transition.md, `canvas2d`, a tween of the same
 * graph under `layout.random` -> `layout.grid`, 600 ms, 1920x1080 at DPR 1): 20 000 nodes held
 * 16.7 ms frames and drew 31 of the 36 the tween wanted; 32 768 drew 20 and its frame gaps were
 * already 33 ms; 65 536 drew 13; 131 072 drew 6 and 200 000 drew 4, at 183 ms a frame. Past this
 * count the tween is a slideshow, and the snap shows the new layout on the first frame instead.
 *
 * Ponytail: the GPU layer mixes the same two columns in its vertex shader for one float a frame,
 * so this budget is the 2D painter's alone — a tween over it on WebGL2 still runs, and `counts`
 * is what says which painter has the frame. Failing input: a host that put a node budget like
 * this one above 32 768 on purpose gets the snap, which is cheaper and less smooth, not wrong.
 * Direction: snap above the budget. Escape hatch: set `TWEEN_BUDGET` above the scene's node count.
 */
export const TWEEN_BUDGET = 32_768;

/** The frame is where the nodes are now, and the layer has nothing left to mix. */
function arrive(state: LoopState): void {
  state.transitionStart = -1;
  state.fromFrame = null;
  state.x = state.scene.frame.x;
  state.y = state.scene.frame.y;
  state.bulk.tween = null;
}

/**
 * True when the 2D painter, not the GPU layer, is the one that would have to ease this tween:
 * `bulk` is the draw count of the last painted frame, so it is 0 exactly when the layer drew
 * nothing (hook.ts). Exported, and narrow on purpose, so the budget is a number a test reads.
 */
export function overBudget(counts: Pick<PaintCounts, "bulk">, nodeCount: number): boolean {
  return counts.bulk === 0 && nodeCount > TWEEN_BUDGET;
}

/**
 * Moves the nodes towards the frame; true while they are still on their way.
 *
 * Exported, and narrow on purpose, so a test can step a tween to a given fraction without a
 * browser's animation frames — node has no `requestAnimationFrame`, so the loop cannot run
 * there at all. Nothing but the frame loop calls it.
 */
export function advance(state: LoopState, now: number): boolean {
  if (state.transitionStart < 0) return false;
  const t = (now - state.transitionStart) / TRANSITION_MS;
  // Compared on the clock, not on t: (start + T) - start can round below T, so a frame stamped
  // exactly at the end left the tween one frame short (focus-tween test, start 825.880174).
  const ended = now >= state.transitionStart + TRANSITION_MS;
  const snapped = overBudget(state.counts, state.scene.frame.nodeCount);
  markTween(state, state.transitionStart, ended || snapped);
  if (ended || snapped) {
    arrive(state);
    return false;
  }
  const eased = easeInOutCubic(t);
  state.eased = eased;
  const { frame } = state.scene;
  blend(state.fromX, frame.x, eased, state.x);
  blend(state.fromY, frame.y, eased, state.y);
  state.bulk.placed += 1;
  // The layer mixes these four columns on the GPU; `state.x`/`state.y` stay the eased pose for
  // the labels, the edges and the hit test, which are all on this side of the fence.
  state.bulk.tween = { fromX: state.fromX, fromY: state.fromY, toX: frame.x, toY: frame.y, eased };
  return true;
}

/**
 * Re-expresses the start of a move in the camera the new frame was fitted to, so the first frame
 * of the move is the picture that was on screen: `fromX`/`fromY` are in the world `before` drew,
 * and each node keeps its screen point under the new camera. A node with no start (NaN, one the
 * old drawing lacked) starts where it ends.
 */
export function placeStart(state: LoopState, before: Camera): void {
  const { camera, fromX, fromY } = state;
  const { frame } = state.scene;
  const ratio = before.scale / camera.scale;
  const dx = (before.x - camera.x) / camera.scale;
  const dy = (before.y - camera.y) / camera.scale;
  for (let node = 0; node < fromX.length; node += 1) {
    const x = fromX[node] ?? Number.NaN;
    const y = fromY[node] ?? Number.NaN;
    const known = Number.isFinite(x) && Number.isFinite(y);
    fromX[node] = known ? x * ratio + dx : (frame.x[node] ?? 0);
    fromY[node] = known ? y * ratio + dy : (frame.y[node] ?? 0);
  }
  state.x.set(fromX);
  state.y.set(fromY);
}
