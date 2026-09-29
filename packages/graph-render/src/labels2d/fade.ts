/**
 * The text-fade slider as a zoom factor. The slider runs -3 (labels appear early) to 3
 * (late); each step multiplies the zoom at which a label starts to show by FADE_BASE.
 *
 * Ponytail: the base is a taste, not a measurement. Obsidian's own slider has no published
 * curve, so this one is ours: 1.5 puts the ends at about 0.3x and 3.4x the default
 * threshold. The factor is one number for the whole graph; it does not look at how dense
 * the labels are.
 */
export const FADE_BASE = 1.5;

export function fadeFactor(fade: number): number {
  return Number.isFinite(fade) ? FADE_BASE ** fade : 1;
}
