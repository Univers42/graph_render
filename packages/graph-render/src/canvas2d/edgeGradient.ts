/**
 * Which edges of one frame take a gradient, and which take the mean of their two colours.
 *
 * An edge whose endpoints share a palette entry is drawn in that entry's colour, batched
 * with every other edge of it — one stroke per CHUNK, as the flat pass does. An edge whose
 * endpoints do not share one is "mixed": live, it gets its own `createLinearGradient` from
 * source to target; past MIXED_EDGE_BUDGET of them in a frame, or while the view moves, it
 * is drawn in the linear mean of its two colours, batched per colour pair.
 *
 * Caveat: MIXED_EDGE_BUDGET gives the gradient up on a large or a moving graph — past it, or
 * while the view moves, a mixed edge comes out in the mean of its two colours instead of
 * running from one to the other, and the error is always in that flat direction; the escape
 * hatch is to raise the budget, at one `createLinearGradient` and one stroke per mixed edge.
 * A second limit with no constant: a curved or routed edge's gradient follows the chord
 * between its ends, not its path, so the colour at a point on the bend is the ramp's at the
 * chord's own t.
 */
import { type Rgb, cssOf, linearOf } from "../colour/srgb.ts";
import { meanLinear } from "../colour/blend.ts";
import type { PaintInput } from "./input.ts";

/** Past this many mixed edges in one frame they are all drawn in their mean colour. */
export const MIXED_EDGE_BUDGET = 512;

/** What an edge is drawn in when a slot has no colour behind it (colormap.ts:20 MISSING). */
export const FALLBACK_COLOUR: Rgb = [0.3, 0.3, 0.3];

export interface ColourPair {
  /** The two palette slots, lower first: the mean of a pair does not depend on the order. */
  readonly from: number;
  readonly to: number;
  /** The mixed edges of that pair, in frame order. */
  readonly edges: number[];
}

export interface EdgePlan {
  /** True when the mixed edges get a gradient rather than the mean of their two colours. */
  readonly live: boolean;
  /** The palette as linear triples, so a blend is taken in linear light. */
  readonly palette: readonly Rgb[];
  /** The same-colour edges, grouped by palette slot; slot `s` owns `start[s]..start[s+1]`. */
  readonly start: Uint32Array;
  readonly same: Uint32Array;
  /** The mixed edges in frame order, and the slot each of their two ends wears. */
  readonly mixedEdges: Uint32Array;
  readonly mixedFrom: Uint32Array;
  readonly mixedTo: Uint32Array;
  /** The mixed edges by colour pair, in the order the first edge of each pair was seen. */
  readonly pairs: readonly ColourPair[];
}

/** The palette as linear triples; null when an entry is not a colour this can read. */
function linearPalette(input: PaintInput): readonly Rgb[] | null {
  const { palette } = input.style;
  const triples: Rgb[] = [];
  for (const entry of palette) {
    const triple = linearOf(entry);
    if (triple === null) return null;
    triples.push(triple);
  }
  return triples;
}

/** The palette slot a node wears; the gradient mode reads colours through this. */
export function slotOf(input: PaintInput, node: number): number {
  return input.style.colours[node] ?? 0;
}

/** The mixed edges by colour pair, in first-seen order: a Map keyed on the slot pair. */
function pairsOf(
  paletteSize: number,
  mixedEdges: Uint32Array,
  from: Uint32Array,
  to: Uint32Array,
): ColourPair[] {
  const pairs: ColourPair[] = [];
  const seen = new Map<number, number>();
  for (let m = 0; m < mixedEdges.length; m += 1) {
    const a = from[m] ?? 0;
    const b = to[m] ?? 0;
    const low = Math.min(a, b);
    const high = Math.max(a, b);
    const key = low * paletteSize + high;
    let at = seen.get(key);
    if (at === undefined) {
      at = pairs.length;
      seen.set(key, at);
      pairs.push({ from: low, to: high, edges: [] });
    }
    (pairs[at]?.edges ?? []).push(mixedEdges[m] ?? 0);
  }
  return pairs;
}

/** The slot each end of every edge wears, and the prefix sums that bucket the same ones. */
interface Slots {
  readonly start: Uint32Array;
  readonly source: Uint32Array;
  readonly target: Uint32Array;
  readonly mixed: number;
}

function slotsOf(input: PaintInput, size: number): Slots {
  const { frame } = input;
  const count = frame.edgeCount;
  const start = new Uint32Array(size + 1);
  const source = new Uint32Array(count);
  const target = new Uint32Array(count);
  let mixed = 0;
  for (let edge = 0; edge < count; edge += 1) {
    const s = slotOf(input, frame.source[edge] ?? 0);
    const t = slotOf(input, frame.target[edge] ?? 0);
    source[edge] = s;
    target[edge] = t;
    if (s === t) start[s + 1] = (start[s + 1] ?? 0) + 1;
    else mixed += 1;
  }
  for (let slot = 0; slot < size; slot += 1) start[slot + 1] = (start[slot + 1] ?? 0) + (start[slot] ?? 0);
  return { start, source, target, mixed };
}

/** The split itself: the same-colour edges into their slots, the mixed ones in frame order. */
function splitOf(slots: Slots): Pick<EdgePlan, "same" | "mixedEdges" | "mixedFrom" | "mixedTo"> {
  const { source, target, mixed, start } = slots;
  const next = start.slice(0, start.length - 1);
  const same = new Uint32Array(source.length - mixed);
  const mixedEdges = new Uint32Array(mixed);
  const mixedFrom = new Uint32Array(mixed);
  const mixedTo = new Uint32Array(mixed);
  let at = 0;
  for (let edge = 0; edge < source.length; edge += 1) {
    const s = source[edge] ?? 0;
    if (s === (target[edge] ?? 0)) {
      same[next[s] ?? 0] = edge;
      next[s] = (next[s] ?? 0) + 1;
    } else {
      mixedEdges[at] = edge;
      mixedFrom[at] = s;
      mixedTo[at] = target[edge] ?? 0;
      at += 1;
    }
  }
  return { same, mixedEdges, mixedFrom, mixedTo };
}

/**
 * The frame's split into same-colour and mixed edges, or null when the style draws every
 * edge in one flat colour (the mode the renderer shipped in, and the one the settings
 * default to) or when a palette entry is not a colour the blend can read.
 */
export function planOf(input: PaintInput): EdgePlan | null {
  if (input.style.edgeColour !== "gradient") return null;
  const palette = linearPalette(input);
  if (palette === null) return null;
  const slots = slotsOf(input, palette.length);
  const split = splitOf(slots);
  const live = !input.moving && slots.mixed <= MIXED_EDGE_BUDGET;
  return {
    live,
    palette,
    start: slots.start,
    ...split,
    pairs: live ? [] : pairsOf(palette.length, split.mixedEdges, split.mixedFrom, split.mixedTo),
  };
}

/** The linear mean of one pair's two palette entries, encoded once. */
export function meanCss(plan: EdgePlan, from: number, to: number): string {
  const a = plan.palette[from] ?? FALLBACK_COLOUR;
  const b = plan.palette[to] ?? FALLBACK_COLOUR;
  return cssOf(meanLinear(a, b));
}