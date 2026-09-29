/**
 * From what the graph is (meta), what was measured on it (an analysis) and what the user
 * asked for (appearance, filter) to the renderer's style input. Pure: same in, same out.
 */
import type { StyleInput } from "../../../graph-render/src/style.ts";
import { NODE_KINDS } from "../source/ingest.ts";
import type { GraphMeta } from "../source/meta.ts";
import type { Appearance, Filter } from "../state/settings.ts";
import { withReveal } from "./reveal.ts";
import { GROUP_PALETTE, MUTED, RAMP } from "./palette.ts";

export interface AnalysisValues {
  readonly id: string;
  readonly kind: "f64" | "u32";
  readonly values: Float64Array | Uint32Array;
}

export interface LookInput {
  readonly meta: GraphMeta;
  readonly appearance: Appearance;
  readonly filter: Filter;
  readonly analysis: AnalysisValues | null;
  /** How many nodes an animation has shown so far, in ingest order; absent or null is all. */
  readonly reveal?: number | null;
}

export interface LegendEntry {
  readonly colour: string;
  readonly label: string;
  readonly count: number;
}

interface Colouring {
  readonly colours: Uint16Array;
  readonly palette: readonly string[];
  /** What palette entry `i` stands for. */
  readonly names: (slot: number) => string;
}

const BASE_RADIUS = 4;
const GAIN = 2.5;
/** Ponytail: fixed pixel bounds picked by eye, not derived from the viewport; a huge graph wants a smaller max. */
const MIN_RADIUS = 0.5;
const MAX_RADIUS = 120;
const LEGEND_ROWS = 12;

/**
 * Ponytail: whether an analysis names groups or measures something is read off its id
 * (`analysis.communities.*`, `analysis.components.*`). A labelling registered under any
 * other family is drawn as a score: a ramp over label numbers, which means nothing. The
 * motor's capability descriptors will carry this; until then, colour by group instead.
 */
export function isLabelling(analysisId: string): boolean {
  return analysisId.startsWith("analysis.communities.") || analysisId.startsWith("analysis.components.");
}

function spanOf(values: Float64Array | Uint32Array): { readonly min: number; readonly span: number } {
  let min = Infinity;
  let max = -Infinity;
  for (const value of values) {
    if (value < min) min = value;
    if (value > max) max = value;
  }
  return values.length === 0 ? { min: 0, span: 0 } : { min, span: max - min };
}

/** Each value's place between the smallest and the largest, 0 when they are all equal. */
function normalised(values: Float64Array | Uint32Array): Float32Array {
  const { min, span } = spanOf(values);
  const out = new Float32Array(values.length);
  if (span > 0) for (let i = 0; i < values.length; i += 1) out[i] = ((values[i] ?? min) - min) / span;
  return out;
}

function number(value: number): string {
  return String(Number(value.toPrecision(3)));
}

function byScore(analysis: AnalysisValues): Colouring {
  const { min, span } = spanOf(analysis.values);
  const last = RAMP.length - 1;
  const colours = Uint16Array.from(normalised(analysis.values), (t) => Math.round(t * last));
  return { colours, palette: RAMP, names: (slot) => number(min + (span * slot) / last) };
}

function byLabel(analysis: AnalysisValues): Colouring {
  const colours = Uint16Array.from(analysis.values, (value) => value % GROUP_PALETTE.length);
  return { colours, palette: GROUP_PALETTE, names: (slot) => `#${slot}` };
}

function byGroup(meta: GraphMeta): Colouring {
  const colours = Uint16Array.from(meta.group, (group) => group % GROUP_PALETTE.length);
  return { colours, palette: GROUP_PALETTE, names: (slot) => meta.groups[slot] ?? `#${slot}` };
}

function byKind(meta: GraphMeta): Colouring {
  const colours = Uint16Array.from(meta.kinds, (kind) => Math.max(0, NODE_KINDS.indexOf(kind)));
  return { colours, palette: GROUP_PALETTE, names: (slot) => NODE_KINDS[slot] ?? `#${slot}` };
}

function colouringOf(input: LookInput): Colouring {
  const { meta, analysis } = input;
  const by = input.appearance.colourBy;
  if (by === "group") return byGroup(meta);
  if (by === "kind") return byKind(meta);
  if (by === "analysis" && analysis !== null && analysis.values.length === meta.nodeCount) {
    return isLabelling(analysis.id) ? byLabel(analysis) : byScore(analysis);
  }
  return { colours: new Uint16Array(meta.nodeCount), palette: [MUTED], names: () => "nodes" };
}

function weightsOf(input: LookInput): Float32Array {
  const { meta, analysis } = input;
  const by = input.appearance.sizeBy;
  if (by === "degree") return Float32Array.from(meta.degree, (degree) => (meta.maxDegree === 0 ? 0 : degree / meta.maxDegree));
  const scored = by === "analysis" && analysis !== null && !isLabelling(analysis.id);
  return scored && analysis.values.length === meta.nodeCount ? normalised(analysis.values) : meta.weight;
}

/** 1 per hidden node, or `null` when the filter hides nothing. */
export function hiddenOf(meta: GraphMeta, filter: Filter): Uint8Array | null {
  const text = filter.text.trim().toLowerCase();
  const groups = new Set(filter.hiddenGroups);
  if (text === "" && groups.size === 0 && filter.minDegree === 0) return null;
  const hidden = new Uint8Array(meta.nodeCount);
  for (let i = 0; i < meta.nodeCount; i += 1) {
    const unnamed = text !== "" && !(meta.labels[i] ?? "").toLowerCase().includes(text);
    const grouped = groups.has(meta.groups[meta.group[i] ?? 0] ?? "");
    if (unnamed || grouped || (meta.degree[i] ?? 0) < filter.minDegree) hidden[i] = 1;
  }
  return hidden;
}

export function styleInputOf(input: LookInput): StyleInput {
  const { colours, palette } = colouringOf(input);
  const { nodeScale, sizeBy, linkThickness, edgeStyle, arrows, glow, glowStrength } = input.appearance;
  return {
    labels: input.meta.labels,
    weights: weightsOf(input),
    colours,
    palette,
    sizing: { base: BASE_RADIUS * nodeScale, gain: sizeBy === "uniform" ? 0 : GAIN, min: MIN_RADIUS, max: MAX_RADIUS },
    edges: { scale: linkThickness, curve: edgeStyle === "curve", arrows },
    glow: glow ? glowStrength : 0,
    hidden: withReveal(hiddenOf(input.meta, input.filter), input.reveal ?? null, input.meta.nodeCount),
  };
}

function countsOf(colouring: Colouring): Map<number, number> {
  const counts = new Map<number, number>();
  for (const slot of colouring.colours) counts.set(slot, (counts.get(slot) ?? 0) + 1);
  return counts;
}

/** What each colour on screen stands for, in palette order, the first LEGEND_ROWS of them. */
export function legendOf(input: LookInput): readonly LegendEntry[] {
  const colouring = colouringOf(input);
  const counts = countsOf(colouring);
  const scored = colouring.palette === RAMP;
  const slots = scored ? [0, RAMP.length - 1] : [...counts.keys()].sort((a, b) => a - b).slice(0, LEGEND_ROWS);
  return slots
    .filter((slot) => counts.has(slot))
    .map((slot) => ({ colour: colouring.palette[slot] ?? MUTED, label: colouring.names(slot), count: counts.get(slot) ?? 0 }));
}
