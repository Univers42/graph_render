/**
 * The hover fade: everything outside the focus goes from opacity 1 to the theme's dim alpha
 * over FADE_MS. The level is 0..1 and the painters read it as a dim alpha, so they stay as
 * they are.
 *
 * Ponytail: only the fade to dim is animated. Leaving the focus restores full opacity in the
 * next frame, because the lit mask is cleared with the focus and the old one is not kept to
 * fade it back out.
 */
export const FADE_MS = 120;

/** `startedAt` is `performance.now()` when the focus appeared, or -1 for no focus. */
export function fadeLevel(startedAt: number, now: number): number {
  if (startedAt < 0) return 0;
  return Math.min(1, Math.max(0, (now - startedAt) / FADE_MS));
}

/** The opacity of a node outside the focus at this level of the fade. */
export function dimAt(dimAlpha: number, level: number): number {
  return 1 + (dimAlpha - 1) * level;
}
