/**
 * An analysis face → what the studio shows with it.
 *
 * Two projections, chosen by the face's OWN `kind` and not by the id: an `f64`
 * face is a magnitude and is ramped across the run's own value domain, a `u32`
 * face is a labelling and is coloured by group. That rule is the whole reason
 * this file exists — deciding it per id would mean a capability registered after
 * this was written got no overlay, and deciding it wrong (ramping a community
 * id, binning a centrality) produces a picture that looks like data and is not.
 *
 * Every function here is pure and total: an empty face, a one-value face and a
 * face of negative centralities all produce fills, never a NaN and never a
 * division by zero.
 */

import type { AnalysisResult } from "../../../crates/graph-sdk-js/src/index.ts";
import { groupColour, rampColour } from "../render/analysisColours.ts";

/** One row of the panel's readout: a name and the value to print for it. */
export interface AnalysisRow {
  readonly label: string;
  readonly value: string;
}

/** One entry of the legend: what the colour means, and the colour itself. */
export interface LegendEntry {
  readonly label: string;
  readonly fill: string;
}

/** A legend longer than this collapses to its first entries and a count: a
 *  Louvain run on 120 nodes can name 36 communities and the panel is 340 px. */
const LEGEND_LIMIT = 8;

/** Magnitudes print to this many decimals — enough for a centrality, short
 *  enough that a `f64` face does not wrap the panel. */
const SCORE_DECIMALS = 6;

/** `value`'s place in `lo..hi` as 0..1. A flat domain has no direction, so it
 *  answers exactly 0.5: the middle of the ramp, rather than the cold end, which
 *  would read as "every node scored zero". */
export function normalise(value: number, lo: number, hi: number): number {
  if (!(hi > lo)) return 0.5;
  return (value - lo) / (hi - lo);
}

function minMax(values: readonly number[]): { lo: number; hi: number } {
  if (values.length === 0) return { lo: 0, hi: 0 };
  let lo = Infinity;
  let hi = -Infinity;
  for (const value of values) {
    if (value < lo) lo = value;
    if (value > hi) hi = value;
  }
  return { lo, hi };
}

/** The distinct group ids of a labelling, ascending — dense by construction, but
 *  sorted here rather than assumed, so a face that skipped an id still ranks its
 *  groups the same way. */
function distinctSorted(values: readonly number[]): number[] {
  return [...new Set(values)].sort((a, b) => a - b);
}

/** One fill per node, an `f64` face ramped over its own domain. */
export function rampFills(result: AnalysisResult): string[] {
  const { lo, hi } = minMax(result.values);
  return result.values.map((value) => rampColour(normalise(value, lo, hi)));
}

/** One fill per node, a `u32` face coloured by group RANK: group `k` of the
 *  ascending distinct groups takes palette entry `k`, so a Louvain run that
 *  numbered its communities 5, 9 and 17 colours exactly like one that numbered
 *  them 0, 1 and 2. */
export function groupPalette(result: AnalysisResult): string[] {
  const ranks = new Map(distinctSorted(result.values).map((group, rank) => [group, rank]));
  return result.values.map((value) => groupColour(ranks.get(value) ?? 0));
}

/** The fills for a face, by kind. This is the one place the two projections
 *  meet, so the panel, the canvas and the tooltip cannot disagree about which
 *  one a face got. */
export function fillsFor(result: AnalysisResult): string[] {
  return result.kind === "u32" ? groupPalette(result) : rampFills(result);
}

/** The node's own value, or `null` for an index the face does not cover — a run
 *  over a different document, or a hover past the end. */
export function valueAt(result: AnalysisResult, index: number): number | null {
  if (index < 0 || index >= result.values.length) return null;
  return result.values[index];
}

/** One node's fill, or `null` when there is no overlay or no such node: the
 *  caller falls back to the ingest-derived style rather than to grey. */
export function fillFor(fills: readonly string[] | null, index: number): string | null {
  if (fills === null || index < 0 || index >= fills.length) return null;
  return fills[index];
}

/** Whether a face describes THIS graph: one value per node, and its own
 *  `nodeCount` agreeing with the run's. A face that does not would colour the
 *  wrong nodes, so the panel reports it instead of drawing it — the motor never
 *  produces one, which is why this is a check rather than a normal path. */
export function coversGraph(result: AnalysisResult, nodeCount: number): boolean {
  return result.nodeCount === nodeCount && result.values.length === nodeCount;
}

/**
 * The hover line for one node under an applied analysis: the value the face holds
 * for it, and nothing else — no id, no kind, because the panel already names
 * both and a tooltip that repeated them would only be wider.
 *
 * `null` for an index the face does not cover (an overlay left over from another
 * document), so the tooltip drops the value instead of showing the wrong node's.
 */
export function hoverValue(result: AnalysisResult, index: number): string | null {
  const value = valueAt(result, index);
  return value === null ? null : formatValue(result, value);
}

/** A score, printed to {@link SCORE_DECIMALS}. Used for `modularity` and for an
 *  `f64` face's extremes, both of which are `f64` whatever the face's `kind`
 *  says — a Louvain partition's quality is not an integer because its community
 *  ids are. */
export function formatScore(value: number): string {
  return value.toFixed(SCORE_DECIMALS);
}

/** How a face's per-node values are printed. A labelling is an integer the motor
 *  said was one, so it prints as the motor wrote it — no rounding, because a
 *  rounding here would print a number the engine never produced. An `f64` is a
 *  widened `f32` and prints through {@link formatScore}. */
export function formatValue(result: AnalysisResult, value: number): string {
  return result.kind === "u32" ? String(value) : formatScore(value);
}

/** The panel's readout: the face's own scalars, in a fixed order, with the three
 *  optional members present exactly when the analysis handed one back. The
 *  `min`/`max` pair is what the ramp spans, and a `u32` face gets a group count
 *  instead — a "min community id" would be a number nobody can act on. */
export function describeAnalysis(result: AnalysisResult): AnalysisRow[] {
  const rows: AnalysisRow[] = [
    { label: "id", value: result.id },
    { label: "kind", value: result.kind },
    { label: "nodes", value: String(result.nodeCount) },
  ];
  if (result.values.length === 0) return [...rows, { label: "empty", value: "no values" }];
  if (result.kind === "u32") {
    rows.push({ label: "groups", value: String(distinctSorted(result.values).length) });
  } else {
    const { lo, hi } = minMax(result.values);
    rows.push({ label: "min", value: formatScore(lo) }, { label: "max", value: formatScore(hi) });
  }
  if (result.converged !== undefined) rows.push({ label: "converged", value: String(result.converged) });
  if (result.modularity !== undefined) rows.push({ label: "modularity", value: formatScore(result.modularity) });
  if (result.max !== undefined) rows.push({ label: "max level", value: String(result.max) });
  return rows;
}

/** What the colours mean. A ramp is named by its two ends, because that is all
 *  the legend can honestly say about a continuous scale; a grouping is named by
 *  every group, up to {@link LEGEND_LIMIT}, then counted. */
export function analysisLegend(result: AnalysisResult): LegendEntry[] {
  if (result.kind === "u32") {
    const groups = distinctSorted(result.values);
    const shown = groups.slice(0, LEGEND_LIMIT);
    const entries = shown.map((group, rank) => ({ label: `group ${group}`, fill: groupColour(rank) }));
    if (groups.length > shown.length) {
      entries.push({ label: `${groups.length - shown.length} more groups`, fill: "transparent" });
    }
    return entries;
  }
  const { lo, hi } = minMax(result.values);
  // A flat face paints one colour, so a two-ended legend would name two colours
  // the canvas never uses. It gets the one it does.
  if (hi <= lo) return [{ label: formatScore(lo), fill: rampColour(0.5) }];
  return [
    { label: formatScore(lo), fill: rampColour(0) },
    { label: formatScore(hi), fill: rampColour(1) },
  ];
}
