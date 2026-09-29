/**
 * The navigation keys, as the studio's own actions. A key press is a dispatch, so what the
 * keyboard does is a thing the console can be typed and the log records; nothing here knows
 * about a keyboard event's target, which the caller has already read.
 */
import type { Args } from "../actions/registry.ts";

/** What one key asks for: an action id and the values it takes, already typed. */
export interface NavKey {
  readonly id: string;
  readonly args: Args;
}

/** A key with a modifier held is the browser's or the host's: Ctrl+F finds in the page. */
export type Held = Pick<KeyboardEvent, "ctrlKey" | "metaKey" | "altKey">;

const NONE: Held = { ctrlKey: false, metaKey: false, altKey: false };

/** A press of + doubles the scale, and a press of - halves it. */
export const ZOOM_STEP = 2;
/** How far an arrow pans, in screen pixels. */
export const ARROW_PAN = 50;

function pan(dx: number, dy: number): NavKey {
  return { id: "view.pan", args: { dx, dy } };
}

function zoom(factor: number): NavKey {
  return { id: "view.zoom", args: { factor } };
}

/**
 * The keys that move the camera, and what each one asks for. `=` and `_` are the two
 * characters the + and - keys print on a layout where the unshifted key is not a symbol.
 */
export const KEYS: Readonly<Record<string, NavKey>> = {
  f: { id: "view.fit", args: {} },
  "0": { id: "view.reset", args: {} },
  "+": zoom(ZOOM_STEP),
  "=": zoom(ZOOM_STEP),
  "-": zoom(1 / ZOOM_STEP),
  _: zoom(1 / ZOOM_STEP),
  ArrowLeft: pan(-ARROW_PAN, 0),
  ArrowRight: pan(ARROW_PAN, 0),
  ArrowUp: pan(0, -ARROW_PAN),
  ArrowDown: pan(0, ARROW_PAN),
  Escape: { id: "view.clear", args: {} },
};

/** What `key` asks for, or null when it asks for nothing here. */
export function navKeyOf(key: string, held: Held = NONE): NavKey | null {
  if (held.ctrlKey || held.metaKey || held.altKey) return null;
  return KEYS[key] ?? null;
}
