/**
 * What the nodes look like, joined to the frame by dense index. A style outlives a layout
 * run and a frame outlives a restyle, so neither is rebuilt for the other.
 */
import type { Rgb } from "./colour/srgb.ts";

export interface StyleInput {
  /** One label per node; a missing one draws no label. */
  readonly labels: readonly string[];
  /** Importance in 0..1: radius, label rank and label threshold all read it. */
  readonly weights: Float32Array;
  /** Index into `palette` per node. */
  readonly colours: Uint16Array;
  readonly palette: readonly string[];
  readonly sizing?: Sizing;
  /** 1 hides the node and every edge touching it. */
  readonly hidden?: Uint8Array | null;
  /** Where a label sits on its node; `below` is the studio default (labels.ts:110). */
  readonly placement?: LabelPlacement;
  /** Stroke width of an edge in world units, or null for the zoom-driven one. */
  readonly edgeWidth?: number | null;
  /** One linear base colour per palette entry, for the impostor spheres; null for flat. */
  readonly spheres?: readonly Rgb[] | null;
  readonly edges?: EdgeLook;
  /** Halo strength around every node; 0 or absent draws none. */
  readonly glow?: number;
  /** Absent is `flat`: every edge in one stroke colour, as the renderer has always drawn. */
  readonly edgeColour?: EdgeColour;
}

/**
 * How an edge takes its colour: `flat` is one stroke in the theme's colour, `gradient` runs
 * from the source node's colour to the target's. The SciGraphs edge tubes carry the node
 * colour attribute and the renderer interpolates it along the edge in linear light
 * (SciGraphs/ui/coloring/properties.py:195-215).
 */
export type EdgeColour = "flat" | "gradient";

export const EDGE_COLOURS: readonly EdgeColour[] = ["flat", "gradient"];

/** How edges are drawn on top of the look's own width: the studio's display panel. */
export interface EdgeLook {
  /** Multiplies the stroke width; the head of an arrow follows it. */
  readonly scale: number;
  /** Bend a straight (Line) edge into a quadratic; routed edges keep their own path. */
  readonly curve: boolean;
  /** A head on every edge, pointing at its target. */
  readonly arrows: boolean;
}

export const PLAIN_EDGES: EdgeLook = { scale: 1, curve: false, arrows: false };

/** `below` puts a label under its node; `centred` is the SciGraphs overlay (text_overlay.py:231). */
export type LabelPlacement = "below" | "centred";

export interface Sizing {
  /** Radius of a weightless node, in world units. */
  readonly base: number;
  /** A node of weight 1 has radius `base · (1 + gain)`. */
  readonly gain: number;
  /** The smallest and largest radius, in world units; absent is unbounded. */
  readonly min?: number;
  readonly max?: number;
}

export interface Style {
  readonly nodeCount: number;
  readonly labels: readonly string[];
  readonly weights: Float32Array;
  readonly radius: Float32Array;
  readonly maxRadius: number;
  readonly palette: readonly string[];
  /** Index into `palette` per node, as it came in: the impostor path reads it. */
  readonly colours: Uint16Array;
  /** Nodes grouped by palette entry: bucket `c` is `bucketItems[bucketStart[c]..bucketStart[c+1]]`. */
  readonly bucketStart: Uint32Array;
  readonly bucketItems: Uint32Array;
  /** Node indices, heaviest first; ties keep index order. */
  readonly rank: Uint32Array;
  readonly hidden: Uint8Array | null;
  readonly placement: LabelPlacement;
  readonly edgeWidth: number | null;
  readonly spheres: readonly Rgb[] | null;
  readonly edges: EdgeLook;
  readonly glow: number;
  readonly edgeColour: EdgeColour;
}

export const DEFAULT_SIZING: Sizing = { base: 4, gain: 2.5 };

/** Square root, so area — what the eye compares — grows linearly with weight. */
export function radiusFor(weight: number, sizing: Sizing): number {
  const bounded = Math.min(1, Math.max(0, Number.isFinite(weight) ? weight : 0));
  return sizing.base * (1 + sizing.gain * Math.sqrt(bounded));
}

function clamped(radius: number, sizing: Sizing): number {
  return Math.min(sizing.max ?? Infinity, Math.max(sizing.min ?? 0, radius));
}

export function bucketsOf(colours: Uint16Array, paletteSize: number): Pick<Style, "bucketStart" | "bucketItems"> {
  const last = Math.max(0, paletteSize - 1);
  const bucketStart = new Uint32Array(paletteSize + 1);
  for (let i = 0; i < colours.length; i += 1) {
    const slot = Math.min(colours[i] ?? 0, last) + 1;
    bucketStart[slot] = (bucketStart[slot] ?? 0) + 1;
  }
  for (let c = 0; c < paletteSize; c += 1) bucketStart[c + 1] = (bucketStart[c + 1] ?? 0) + (bucketStart[c] ?? 0);
  const next = bucketStart.slice(0, paletteSize);
  const bucketItems = new Uint32Array(colours.length);
  for (let i = 0; i < colours.length; i += 1) {
    const colour = Math.min(colours[i] ?? 0, last);
    const at = next[colour] ?? 0;
    bucketItems[at] = i;
    next[colour] = at + 1;
  }
  return { bucketStart, bucketItems };
}

export function rankOf(weights: Float32Array): Uint32Array {
  const rank = new Uint32Array(weights.length);
  for (let i = 0; i < rank.length; i += 1) rank[i] = i;
  return rank.sort((a, b) => (weights[b] ?? 0) - (weights[a] ?? 0) || a - b);
}

/** Every node's drawn radius, and the largest of them. */
export function radiiOf(weights: Float32Array, sizing: Sizing): Pick<Style, "radius" | "maxRadius"> {
  const radius = new Float32Array(weights.length);
  let maxRadius = 0;
  for (let i = 0; i < radius.length; i += 1) {
    const value = clamped(radiusFor(weights[i] ?? 0, sizing), sizing);
    radius[i] = value;
    if (value > maxRadius) maxRadius = value;
  }
  return { radius, maxRadius };
}

export function styleFrom(input: StyleInput): Style {
  const palette = input.palette.length > 0 ? input.palette : ["#9a9a9a"];
  return {
    nodeCount: input.weights.length,
    labels: input.labels,
    weights: input.weights,
    ...radiiOf(input.weights, input.sizing ?? DEFAULT_SIZING),
    palette,
    colours: input.colours,
    ...bucketsOf(input.colours, palette.length),
    rank: rankOf(input.weights),
    hidden: input.hidden ?? null,
    placement: input.placement ?? "below",
    edgeWidth: input.edgeWidth ?? null,
    spheres: input.spheres ?? null,
    edges: input.edges ?? PLAIN_EDGES,
    glow: input.glow ?? 0,
    edgeColour: input.edgeColour ?? "flat",
  };
}

/** A style for a graph nothing is known about: equal weights, one colour, no labels. */
export function plainStyle(nodeCount: number): Style {
  return styleFrom({
    labels: [],
    weights: new Float32Array(nodeCount),
    colours: new Uint16Array(nodeCount),
    palette: [],
  });
}
