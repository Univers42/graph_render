/**
 * From what the graph is (meta), what was measured on it (an analysis) and what the user
 * asked for (appearance, filter) to the renderer's style input. Pure: same in, same out.
 */
import type { StyleInput } from "../../../graph-render/src/style.ts";
import type { GraphMeta } from "../source/meta.ts";
import type { Appearance, Filter, Group } from "../state/settings.ts";
import { hiddenOf } from "./visibleOf.ts";
import { type Colouring, colouringOf as byColourBy } from "./colourBy.ts";
import { overlayGroups } from "./groupOverlay.ts";
import { GROUP_PALETTE, MUTED } from "./palette.ts";

export interface AnalysisValues {
  readonly id: string;
  readonly kind: "f64" | "u32";
  readonly values: Float64Array | Uint32Array;
}

export interface LookInput {
  readonly meta: GraphMeta;
  readonly appearance: Appearance;
  readonly filter: Filter;
  readonly groups: readonly Group[];
  readonly analysis: AnalysisValues | null;
}

export interface LegendEntry {
  readonly colour: string;
  readonly label: string;
  readonly count: number;
}


const BASE_RADIUS = 4;
const GAIN = 2.5;
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

function byLabel(analysis: AnalysisValues): Colouring {
  const colours = Uint16Array.from(analysis.values, (value) => value % GROUP_PALETTE.length);
  return { colours, palette: GROUP_PALETTE, names: (slot) => `#${slot}` };
}

/** One colour path: the labelling case is the only one drawn here, every other goes through colourBy. */
function colouringOf(input: LookInput): Colouring {
  const { meta, analysis, groups } = input;
  const by = input.appearance.colourBy;
  if (by === "analysis" && analysis !== null && isLabelling(analysis.id) && analysis.values.length === meta.nodeCount) {
    return overlayGroups(byLabel(analysis), meta, groups);
  }
  const values = analysis !== null && !isLabelling(analysis.id) ? analysis.values : null;
  return byColourBy({ meta, by, values, groups });
}

function weightsOf(input: LookInput): Float32Array {
  const { meta, analysis } = input;
  const by = input.appearance.sizeBy;
  if (by === "degree") return Float32Array.from(meta.degree, (degree) => (meta.maxDegree === 0 ? 0 : degree / meta.maxDegree));
  const scored = by === "analysis" && analysis !== null && !isLabelling(analysis.id);
  return scored && analysis.values.length === meta.nodeCount ? normalised(analysis.values) : meta.weight;
}

export function styleInputOf(input: LookInput): StyleInput {
  const { colours, palette } = colouringOf(input);
  const { nodeScale, sizeBy } = input.appearance;
  return {
    labels: input.meta.labels,
    weights: weightsOf(input),
    colours,
    palette,
    sizing: { base: BASE_RADIUS * nodeScale, gain: sizeBy === "uniform" ? 0 : GAIN },
    hidden: hiddenOf(input.meta, input.filter),
  };
}

function countsOf(colouring: Colouring): Map<number, number> {
  const counts = new Map<number, number>();
  for (const slot of colouring.colours) counts.set(slot, (counts.get(slot) ?? 0) + 1);
  return counts;
}

function extremes(values: Float64Array | Uint32Array): readonly number[] {
  let low = 0;
  let high = 0;
  values.forEach((value, node) => {
    if (value < (values[low] ?? value)) low = node;
    if (value > (values[high] ?? value)) high = node;
  });
  return values.length === 0 ? [] : [low, high];
}

/** The slots the legend names: a score shows the colours of its smallest and largest value. */
function legendSlots(input: LookInput, colouring: Colouring, counts: Map<number, number>): readonly number[] {
  const { analysis, groups, appearance } = input;
  const scored = appearance.colourBy === "analysis" && groups.length === 0 && analysis !== null
    && !isLabelling(analysis.id) && analysis.values.length === input.meta.nodeCount;
  if (scored) return [...new Set(extremes(analysis.values).map((node) => colouring.colours[node] ?? 0))];
  return [...counts.keys()].sort((a, b) => a - b).slice(0, LEGEND_ROWS);
}

/** What each colour on screen stands for, in palette order, the first LEGEND_ROWS of them. */
export function legendOf(input: LookInput): readonly LegendEntry[] {
  const colouring = colouringOf(input);
  const counts = countsOf(colouring);
  return legendSlots(input, colouring, counts)
    .filter((slot) => counts.has(slot))
    .map((slot) => ({ colour: colouring.palette[slot] ?? MUTED, label: colouring.names(slot), count: counts.get(slot) ?? 0 }));
}
