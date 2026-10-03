/** Moving from one layout to the next: positions blended in place, no allocation per frame. */

export const TRANSITION_MS = 600;

/** The marks a layout-switch measurement reads between the request and the settled frame. */
export const MOVED_MARK = "gm:transition:moved";
export const SETTLED_MARK = "gm:transition:settled";

/** The tween a view is already marked under, so each mark fires once per tween. */
const tween = new WeakMap<object, number>();

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
 * Ponytail: keyed by a `performance.now()` value, so two switches inside one millisecond share
 * a mark. Failing input: a view that never settles leaves the entry, and the next tween marks
 * again anyway. Direction: one mark per tween, never cleared. Escape hatch: the console's
 * `performance.getEntriesByName`.
 */
export function markTween(state: object, start: number, settled: boolean): void {
  if (tween.get(state) === start && !settled) return;
  if (settled) {
    tween.delete(state);
    performance.mark(SETTLED_MARK);
    return;
  }
  tween.set(state, start);
  performance.mark(MOVED_MARK);
}
