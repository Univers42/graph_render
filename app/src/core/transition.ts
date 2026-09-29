/**
 * The layout-to-layout transition: ~600 ms of eased interpolation from the
 * positions on screen to the ones the motor just produced, so switching layouts
 * reads as one graph moving rather than two graphs swapping.
 *
 * Pure on purpose. The renderer owns the clock; these functions own the maths, so
 * "how far along is the transition" is a value a test can pin instead of a frame
 * someone eyeballed.
 */

/** The transition's length. Pinned: the whole point of the eased move is that it
 *  is short enough not to be waited on and long enough to be followed. */
export const TRANSITION_MS = 600;

export function clamp01(value: number): number {
  if (Number.isNaN(value)) return 0;
  return value < 0 ? 0 : value > 1 ? 1 : value;
}

/** Symmetric ease-in-out cubic: 4t³ on the way in, 1 - (-2t + 2)³/2 on the way
 *  out. Accelerates away from the old layout, decelerates into the new one. */
export function easeInOutCubic(t: number): number {
  const u = clamp01(t);
  return u < 0.5 ? 4 * u * u * u : 1 - Math.pow(-2 * u + 2, 3) / 2;
}

/** 0 at the start of the transition, 1 from `durationMs` on (and for a
 *  non-positive duration, immediately). */
export function transitionProgress(elapsedMs: number, durationMs = TRANSITION_MS): number {
  if (durationMs <= 0) return 1;
  return clamp01(elapsedMs / durationMs);
}

/** `from + (to - from) * t`, element-wise, into `out` when given. Buffers of
 *  different lengths are refused rather than silently truncated — a short `to`
 *  would leave the tail of the graph behind at its old position. */
export function lerpPositions(
  from: Float32Array,
  to: Float32Array,
  t: number,
  out: Float32Array = new Float32Array(from.length),
): Float32Array {
  if (from.length !== to.length || to.length !== out.length) {
    throw new Error(`lerpPositions: buffers must be the same length (${from.length}, ${to.length}, ${out.length})`);
  }
  for (let i = 0; i < out.length; i += 1) out[i] = from[i] + (to[i] - from[i]) * t;
  return out;
}

/** The interpolated point run for one edge, or `null` when the two paths cannot
 *  be matched point for point — a rerouted arc from a layered layout has a
 *  different vertex count than the straight line it replaces, and the renderer
 *  cross-fades those instead of morphing nonsense. */
export function interpolatePath(from: Float32Array, to: Float32Array, t: number): Float32Array | null {
  if (from.length === 0 || from.length !== to.length) return null;
  return lerpPositions(from, to, t);
}
