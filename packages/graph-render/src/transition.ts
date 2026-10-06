/** Moving from one layout to the next: positions blended in place, no allocation per frame. */

/**
 * 400 ms, down from 600: a layout switch read as slow at 600 (user report, 2026-10-06), and 400 is
 * still long enough for the eye to follow one node across the screen.
 */
export const TRANSITION_MS = 400;

/** The marks a layout-switch measurement reads between the request and the settled frame. */
export const MOVED_MARK = "gm:transition:moved";
export const SETTLED_MARK = "gm:transition:settled";

/** Where a view's marking has got to: which tween, and whether its end has been marked. */
interface Marking {
  readonly start: number;
  done: boolean;
}

const marking = new WeakMap<object, Marking>();

export function easeInOutCubic(t: number): number {
  const bounded = Math.min(1, Math.max(0, t));
  return bounded < 0.5 ? 4 * bounded ** 3 : 1 - (-2 * bounded + 2) ** 3 / 2;
}

/** `out[i] = from[i] + (to[i] - from[i]) · t`. A column shorter than `out` reads as `to`. */
export function blend(from: Float32Array, to: Float32Array, t: number, out: Float32Array): void {
  for (let i = 0; i < out.length; i += 1) {
    const end = to[i] ?? 0;
    const start = from[i] ?? end;
    out[i] = start + (end - start) * t;
  }
}

/**
 * Marks the tween `state` is in, one mark per tween: `moved` on its first moving frame,
 * `settled` on the frame that ends it. Keyed by the tween's own clock rather than by a flag on
 * the view, so the next switch re-arms both without the loop carrying a field for it.
 *
 * A tween the 2D painter snaps over its budget (`TWEEN_BUDGET` in canvas2d/tween.ts) has no
 * moving frame at all, and marks `settled` alone: the end of that switch is a real time, and a
 * probe waiting for it must not wait for a beginning that never comes.
 *
 * Ponytail: keyed by a `performance.now()` value, so two switches inside one millisecond share
 * a mark. Failing input: a view that never settles leaves the entry, and the next tween marks
 * again anyway. Direction: one mark per tween, never cleared. Escape hatch: the console's
 * `performance.getEntriesByName`.
 */
export function markTween(state: object, start: number, settled: boolean): void {
  const seen = marking.get(state);
  if (seen !== undefined && seen.start === start) {
    if (!settled || seen.done) return;
    seen.done = true;
    performance.mark(SETTLED_MARK);
    return;
  }
  marking.set(state, { start, done: settled });
  performance.mark(settled ? SETTLED_MARK : MOVED_MARK);
}
