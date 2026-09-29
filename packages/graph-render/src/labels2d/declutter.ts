/**
 * The screen-space label pass: rank by importance, then greedily drop the labels whose
 * estimated box would collide with one already accepted, and only then cut the survivors.
 *   SciGraphs/core/visualization/text_overlay.py:302-326 (the declutter)
 *   SciGraphs/core/repro/executor.py:498-507 (rank, declutter, then the cut)
 */

/** One label as the declutter sees it: a screen point and the string to draw. */
export interface LabelBox {
  readonly x: number;
  readonly y: number;
  /** Character count only; the painter measures the real glyphs. */
  readonly text: string;
}

interface Extent {
  readonly left: number;
  readonly right: number;
  readonly top: number;
  readonly bottom: number;
}

/**
 * Python len() counts code points, so an astral glyph is one and not two. `Array.from`
 * walks the string iterator, which yields code points, and is what `[...text]` does without
 * the lint rule that cannot tell the two apart.
 */
function codePoints(text: string): number {
  return Array.from(text).length;
}

/** Half-width 0.30 of the font size per character, half-height 0.62 of it (:314,317). */
function extentOf(label: LabelBox, fontSize: number): Extent {
  const halfW = 0.3 * fontSize * Math.max(codePoints(label.text), 1);
  const halfH = 0.62 * Math.max(fontSize, 1);
  return { left: label.x - halfW, right: label.x + halfW, top: label.y - halfH, bottom: label.y + halfH };
}

/**
 * The overlap test is a strict < on both axes, exactly as in the source: a box whose
 * edge touches another's edge is not overlapping and both are kept.
 */
function overlaps(a: Extent, b: Extent): boolean {
  return a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom;
}

/**
 * The indices of the labels to draw, greedy in the order given, so the caller sorts by
 * importance first and the survivors are the important ones.
 *
 * Ponytail: the box is estimated from the character count and a fixed height rather than
 * measured from the font, which the source does too (text_overlay.py:309-313). The width
 * is wrong in both directions and the source claims it errs wide, but on DejaVu Sans at
 * 26 px it is too narrow for 7 of the 18 fig6 names rich in wide glyphs (MmePontmercy is
 * 204 px of text against 187 estimated), so two labels that overlap are both kept and the
 * result is the unreadable stack the pass exists to prevent; it is too wide for narrow
 * glyphs (i, l, t), so a label that would have fitted is dropped. The escape hatch is to
 * measure the real glyphs once the font is loaded and pre-filter the list.
 */
export function declutter(labels: readonly LabelBox[], fontSize: number): number[] {
  const kept: number[] = [];
  const accepted: Extent[] = [];
  for (const [i, label] of labels.entries()) {
    const box = extentOf(label, fontSize);
    if (accepted.some((other) => overlaps(box, other))) continue;
    accepted.push(box);
    kept.push(i);
  }
  return kept;
}

/** The sort key's first term: a value that is not there goes last (executor.py:500). */
function absent(score: number | undefined): number {
  return Number.isNaN(score ?? Number.NaN) ? 1 : 0;
}

/** Descending by score, and a pair of missing scores compares equal so ties stay in order. */
function descending(b: number | undefined, a: number | undefined): number {
  if (absent(a) === 1 || absent(b) === 1) return 0;
  return (b ?? 0) - (a ?? 0);
}

/**
 * Every node index, ordered by score descending, ties broken by index ascending so the
 * order is fixed and a missing score lands last (executor.py:498-501). The whole ranking
 * comes back: the source declutters it in full and only then cuts it (:503-507).
 */
export function rankedLabels(scores: Float64Array | Uint32Array): Uint32Array {
  const order = Array.from(scores.keys());
  // Array.prototype.sort is stable (ES2019) and `order` starts in index order, so
  // equal scores come out index ascending without a second comparison key.
  order.sort((a, b) => absent(scores[a]) - absent(scores[b]) || descending(scores[b], scores[a]));
  return Uint32Array.from(order);
}

/** The boxes of the ranked nodes, and the node index each of them came from. */
function rankedBoxes(
  order: Uint32Array,
  labels: readonly LabelBox[],
): { boxes: LabelBox[]; source: number[] } {
  const boxes: LabelBox[] = [];
  const source: number[] = [];
  for (const index of order) {
    const box = labels[index];
    if (box === undefined) continue;
    boxes.push(box);
    source.push(index);
  }
  return { boxes, source };
}

/**
 * The source's own order, label for label: rank, declutter the whole ranking, then cut to
 * max_count (executor.py:498-507). Cutting before the declutter would drop labels the
 * source keeps, because a stacked label is dropped by the declutter rather than by the
 * count. A max_count of 0 or below is not a cut at all (:506-507).
 */
export function selectLabels(
  scores: Float64Array | Uint32Array,
  labels: readonly LabelBox[],
  fontSize: number,
  limit: number,
): number[] {
  const { boxes, source } = rankedBoxes(rankedLabels(scores), labels);
  const kept = declutter(boxes, fontSize);
  const cut = limit > 0 ? kept.slice(0, limit) : kept;
  return cut.map((at) => source[at] ?? 0);
}
