/** Moving from one layout to the next: positions blended in place, no allocation per frame. */

export const TRANSITION_MS = 600;

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
