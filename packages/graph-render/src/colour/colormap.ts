/**
 * sampleColormap walks the generated 32-stop ramp; coloursOf turns normalised values
 * into a palette the painter can index directly.
 *   SciGraphs/core/scigraphs_core/coloring/colormaps.py:329-335 (32 stops from linspace)
 *   SciGraphs/ui/coloring/functions.py:250-270 (stops at i/(count-1)), 406-410 (LINEAR)
 *   SciGraphs/core/scigraphs_core/coloring/colormaps.py:558-582 (clip, then the missing colour)
 */
import type { ColormapName } from "./tables.ts";
import { COLORMAP_STOPS } from "./tables.ts";
import { cssOf } from "./srgb.ts";
import type { Rgb } from "./srgb.ts";

/** Stops sit at x_k = k/31, so a t lands between two of them. */
const STOPS = 32;

/** Past this many distinct values the ramp has fewer colours than the sample has nodes. */
const MAX_DISTINCT = 1024;

/** What a non-finite sample becomes (colormaps.py:563). */
export const MISSING: Rgb = [0.3, 0.3, 0.3];

export interface NodeColours {
  /** One cssOf entry per distinct colour, in first-seen order. */
  readonly palette: readonly string[];
  /** slots[i] is the palette entry of node i. */
  readonly slots: Uint16Array;
}

function channelAt(name: ColormapName, stop: number, channel: number): number {
  const flat = COLORMAP_STOPS[name];
  const value = flat[stop * 3 + channel];
  if (value === undefined) throw new Error(`${name} has no stop ${stop}`);
  return value;
}

function mix(a: number, b: number, f: number): number {
  return a + (b - a) * f;
}

/** Where a clamped t falls on the ramp: the stop below it and the fraction into the next. */
function locate(t: number): { stop: number; fraction: number } {
  const scaled = t * (STOPS - 1);
  const stop = Math.min(STOPS - 1, Math.floor(scaled));
  return { stop, fraction: scaled - stop };
}

/** Linear interpolation across the 32 stops. A non-finite t is the missing colour. */
export function sampleColormap(name: ColormapName, t: number): Rgb {
  if (!Number.isFinite(t)) return MISSING;
  const { stop, fraction } = locate(Math.min(1, Math.max(0, t)));
  const next = Math.min(STOPS - 1, stop + 1);
  return [
    mix(channelAt(name, stop, 0), channelAt(name, next, 0), fraction),
    mix(channelAt(name, stop, 1), channelAt(name, next, 1), fraction),
    mix(channelAt(name, stop, 2), channelAt(name, next, 2), fraction),
  ];
}

/** How many distinct normalised values the sample carries, NaN counting as one. */
function distinctCount(norm: Float64Array): number {
  const seen = new Set<number>();
  // -1 stands in for NaN: normalise() clips into 0..1, so no real value can be -1.
  for (const value of norm) seen.add(Number.isNaN(value) ? -1 : value);
  return seen.size;
}

/**
 * Ponytail: past 1024 distinct values each t is snapped to the nearest 1/1023, so a node
 * whose t falls between two grid points is drawn up to 1/2046 off in t, about half a
 * palette step. The colour count is wrong in the direction of fewer entries, and the
 * escape hatch is to pass values already quantised or to shrink the node count.
 */
function quantise(t: number): number {
  return Math.round(t * (MAX_DISTINCT - 1)) / (MAX_DISTINCT - 1);
}

/**
 * One palette entry per distinct colour, slots[i] the entry of node i. The lookup
 * key is the CSS string, so two t values that encode to the same bytes share one
 * entry; the palette order is the order the colours are first seen in the sample,
 * so nothing depends on iterating the map.
 */
export function coloursOf(norm: Float64Array, name: ColormapName): NodeColours {
  const quantised = distinctCount(norm) > MAX_DISTINCT;
  const palette: string[] = [];
  const index = new Map<string, number>();
  const slots = new Uint16Array(norm.length);
  for (let i = 0; i < norm.length; i += 1) {
    const value = norm[i] ?? Number.NaN;
    const t = quantised && Number.isFinite(value) ? quantise(value) : value;
    const css = cssOf(sampleColormap(name, t));
    let slot = index.get(css);
    if (slot === undefined) {
      slot = palette.length;
      index.set(css, slot);
      palette.push(css);
    }
    slots[i] = slot;
  }
  return { palette, slots };
}
