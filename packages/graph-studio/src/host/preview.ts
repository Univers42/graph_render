/** What a host's `resolve` answered, checked and cut to size before anything keeps or shows it. */
import type { NodePreview } from "./contract.ts";

export interface PreviewLimits {
  readonly title: number;
  readonly text: number;
  readonly icon: number;
}

/** In code points, not UTF-16 units: a cut never splits a surrogate pair. */
export const PREVIEW_LIMITS: PreviewLimits = Object.freeze({ title: 256, text: 4096, icon: 16 });
/** Caveat: a title over 256, a text over 4096 or an icon over 16 code points loses its tail unmarked and reads short; a wider `limits` on `createPreviews` is the way out. */

/**
 * `text` cut to `max` code points. O(max): a string of a gigabyte is walked only as far as the
 * cut, and one no longer than `max` UTF-16 units is returned without a walk, because it cannot
 * hold more code points than units.
 */
export function clip(text: string, max: number): string {
  if (text.length <= max) return text;
  let units = 0;
  let points = 0;
  for (const point of text) {
    if (points === max) return text.slice(0, units);
    units += point.length;
    points += 1;
  }
  return text;
}

function field(value: unknown, name: string): unknown {
  return typeof value === "object" && value !== null ? Reflect.get(value, name) : undefined;
}

function checked(value: unknown, limits: PreviewLimits): NodePreview | null {
  const title = field(value, "title");
  if (typeof title !== "string") return null;
  const text = field(value, "text");
  const icon = field(value, "icon");
  return Object.freeze({
    title: clip(title, limits.title),
    ...(typeof text === "string" ? { text: clip(text, limits.text) } : {}),
    ...(typeof icon === "string" ? { icon: clip(icon, limits.icon) } : {}),
  });
}

/**
 * A frozen preview built from the fields that have the right type, or null when `title` is not
 * a string. Anything else the host put in the object, a `url` included, is not read.
 *
 * WHY the try: the value is the host's, and a getter or a proxy can throw; that reads as a
 * rejection, which is never cached.
 */
export function previewOf(value: unknown, limits: PreviewLimits = PREVIEW_LIMITS): NodePreview | null {
  try {
    return checked(value, limits);
  } catch {
    return null;
  }
}
