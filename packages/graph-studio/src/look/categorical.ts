/**
 * Categorical colouring: one palette entry per distinct key in first-seen order.
 *
 * Ponytail: past MAX_KEYS distinct keys the extra keys share one MUTED slot, so a
 * drawing with more keys than that shows fewer colours than keys. Reduce the key
 * space or colour by a metric instead.
 */
import { GROUP_PALETTE, MUTED } from "./palette.ts";

export const MAX_KEYS = 512;

export interface Categorical {
  readonly colours: Uint16Array;
  readonly palette: readonly string[];
  readonly names: (slot: number) => string;
}

export interface CategoryLabels {
  /** What the empty key is called; the empty key is drawn MUTED. */
  readonly empty: string;
  readonly overflow: string;
}

/** One slot per distinct key of `keys`, in first-seen order. */
export function categoricalOf(keys: readonly string[], labels: CategoryLabels): Categorical {
  const slotOf = new Map<string, number>();
  const names: string[] = [];
  const colours = new Uint16Array(keys.length);
  keys.forEach((key, node) => {
    let slot = slotOf.get(key);
    if (slot === undefined) {
      slot = Math.min(names.length, MAX_KEYS);
      if (slot < MAX_KEYS) {
        slotOf.set(key, slot);
        names.push(key);
      }
    }
    colours[node] = slot;
  });
  const palette = names.map((name, i) => (name === "" ? MUTED : (GROUP_PALETTE[i % GROUP_PALETTE.length] ?? MUTED)));
  if (names.length >= MAX_KEYS) palette.push(MUTED);
  const nameOf = (slot: number): string => {
    const name = names[slot];
    if (name === undefined) return labels.overflow;
    return name === "" ? labels.empty : name;
  };
  return { colours, palette, names: nameOf };
}
