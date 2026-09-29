/**
 * normalise() mirrors SciGraphs' normalize_values step for step, because the studio's
 * colours have to land where SciGraphs' would for the same metric:
 * percentile clip (colormaps.py:423-435), vmin/vmax override and the degenerate-range
 * nudge (:437-440), clip to the bounds (:517), the per-mode transform (:520-538),
 * clip to 0..1 (:540), then gamma as norm^(1/gamma) (:542-543).
 *
 * SciGraphs/core/scigraphs_core/coloring/colormaps.py:411-555
 */

/** The modes of colormaps.py:338-366, in the source's own order. */
export const NORM_MODES = ["LINEAR", "LOG", "RANK", "QUANTILE"] as const;

export type NormMode = (typeof NORM_MODES)[number];

export interface NormOptions {
  readonly mode: NormMode;
  /** Applied as norm^(1/gamma); anything <= 0 or non-finite is 1 (colormaps.py:492-493). */
  readonly gamma?: number;
  /** Percentile clip bounds; the pass runs only when one moved (colormaps.py:425). */
  readonly clipLowPct?: number;
  readonly clipHighPct?: number;
  readonly vmin?: number;
  readonly vmax?: number;
}

/** LOG epsilon constants of colormaps.py:382-385. */
const LOG_EPS_DECADES = 1e-3;
const LOG_EPS_FLOOR = 1e-30;
const LOG_ZERO_RAMP_FRACTION = 0.1;
const LOG_MIN_GAP_DECADES = 0.05;

/** A degenerate lo/hi is widened by this much rather than dividing by zero (:440). */
const DEGENERATE = 1e-9;

/** Indexed reads go through this so a wrong index is a throw, not a silent NaN. */
function at(values: Float64Array, index: number): number {
  const value = values[index];
  if (value === undefined) throw new RangeError(`index ${index} out of range`);
  return value;
}

function slot(values: Uint32Array, index: number): number {
  const value = values[index];
  if (value === undefined) throw new RangeError(`index ${index} out of range`);
  return value;
}

/** numpy's linear-interpolation percentile, on an ascending sample (colormaps.py:431-432). */
function percentileOf(sorted: readonly number[], q: number): number {
  const first = sorted[0];
  if (first === undefined) return Number.NaN;
  if (sorted.length === 1) return first;
  const pos = (q / 100) * (sorted.length - 1);
  const low = sorted[Math.floor(pos)];
  const high = sorted[Math.ceil(pos)];
  if (low === undefined || high === undefined) return Number.NaN;
  // numpy's _lerp switches to b - (b-a)*(1-t) at t >= 0.5, and the two forms are
  // not the same double, so the bounds move by an ulp against the source.
  const span = high - low;
  const f = pos - Math.floor(pos);
  return f >= 0.5 ? high - span * (1 - f) : low + span * f;
}

/** One pass for the extrema; a spread of the sample would overflow the call stack. */
function extentOf(values: readonly number[]): readonly [number, number] {
  let lo = Number.POSITIVE_INFINITY;
  let hi = Number.NEGATIVE_INFINITY;
  for (const value of values) {
    if (value < lo) lo = value;
    if (value > hi) hi = value;
  }
  return [lo, hi];
}

function clampPct(q: number): number {
  return Math.min(Math.max(q, 0), 100);
}

/** The percentile pass, bounds clamped to 0..100 and swapped when reversed (colormaps.py:425-432). */
function percentileBounds(finite: readonly number[], options: NormOptions): readonly [number, number] {
  const low = clampPct(options.clipLowPct ?? 0);
  const high = clampPct(options.clipHighPct ?? 100);
  const [from, to] = high < low ? [high, low] : [low, high];
  const sorted = [...finite].sort((a, b) => a - b);
  return [percentileOf(sorted, from), percentileOf(sorted, to)];
}

/** lo/hi after the optional percentile pass and the vmin/vmax override (colormaps.py:411-441). */
function effectiveBounds(finite: readonly number[], options: NormOptions): readonly [number, number] {
  const low = options.clipLowPct ?? 0;
  const high = options.clipHighPct ?? 100;
  const auto: readonly [number, number] = low > 0 || high < 100
    ? percentileBounds(finite, options)
    : extentOf(finite);
  const lo = options.vmin ?? auto[0];
  const hi = options.vmax ?? auto[1];
  return [lo, hi === lo ? lo + DEGENERATE : hi];
}

/** The floor applied before log10 (colormaps.py:444-459). */
function logEpsilon(work: readonly number[], upper: number): number {
  let posMin = Number.POSITIVE_INFINITY;
  let positives = 0;
  let floored = false;
  for (const value of work) {
    if (value > 0) {
      positives += 1;
      if (value < posMin) posMin = value;
    } else {
      floored = true;
    }
  }
  // Nothing positive to sit under, so the floor stands alone (colormaps.py:446-448).
  if (positives === 0) return LOG_EPS_FLOOR;
  if (!floored) {
    // Nothing to floor: eps only has to stay under the data, never binding.
    return Math.max(posMin * LOG_EPS_DECADES, LOG_EPS_FLOOR);
  }
  const hi = Math.max(upper, posMin);
  const span = Math.log10(hi) - Math.log10(posMin);
  const fraction = 1 - LOG_ZERO_RAMP_FRACTION;
  const gap = Math.max((span * LOG_ZERO_RAMP_FRACTION) / fraction, LOG_MIN_GAP_DECADES);
  return Math.max(posMin * 10 ** -gap, LOG_EPS_FLOOR);
}

/**
 * The input indices sorted by value, ascending, ties broken by position. The rank passes
 * need ties grouped, so the order is fixed rather than left to the sort alone.
 */
function ascending(work: readonly number[]): Uint32Array {
  const order = Array.from({ length: work.length }, (_, i) => i);
  order.sort((a, b) => {
    const delta = (work[a] ?? 0) - (work[b] ?? 0);
    return delta === 0 ? a - b : delta;
  });
  return Uint32Array.from(order);
}

/** The end of the tie group starting at `start`, exclusive (colormaps.py:462-469). */
function groupEnd(work: readonly number[], order: Uint32Array, start: number): number {
  const value = work[slot(order, start)];
  let end = start + 1;
  while (end < order.length && work[slot(order, end)] === value) end += 1;
  return end;
}

/** Mid-rank of every sample, ties sharing their average rank (colormaps.py:462-469). */
function midRanks(work: readonly number[], out: Float64Array): void {
  const order = ascending(work);
  let start = 0;
  while (start < order.length) {
    const end = groupEnd(work, order, start);
    const mid = (start + end - 1) * 0.5;
    for (let i = start; i < end; i += 1) out[slot(order, i)] = mid;
    start = end;
  }
}

/** Rank among distinct values, so a block of identical samples does not eat the ramp (:531-535). */
function quantileIndices(work: readonly number[], out: Float64Array): number {
  const order = ascending(work);
  let start = 0;
  let distinct = 0;
  while (start < order.length) {
    const end = groupEnd(work, order, start);
    for (let i = start; i < end; i += 1) out[slot(order, i)] = distinct;
    distinct += 1;
    start = end;
  }
  return distinct;
}

/** Mid-rank over (n-1), so a sample of one does not divide by zero (colormaps.py:527-530). */
function stageRanks(work: readonly number[], out: Float64Array): void {
  midRanks(work, out);
  const denom = work.length > 1 ? work.length - 1 : 1;
  for (let i = 0; i < out.length; i += 1) out[i] = at(out, i) / denom;
}

/** Rank over (distinct-1), the same guard for a single distinct value (colormaps.py:531-535). */
function stageQuantiles(work: readonly number[], out: Float64Array): void {
  const distinct = quantileIndices(work, out);
  const denom = distinct > 1 ? distinct - 1 : 1;
  for (let i = 0; i < out.length; i += 1) out[i] = at(out, i) / denom;
}

/** The per-mode transform, in the source's order (colormaps.py:520-538). */
function transform(
  work: readonly number[],
  bounds: readonly [number, number],
  mode: NormMode,
): Float64Array {
  const staged = new Float64Array(work.length);
  if (mode === "LOG") logNormalise(work, bounds, staged);
  if (mode === "RANK") stageRanks(work, staged);
  if (mode === "QUANTILE") stageQuantiles(work, staged);
  if (mode === "LINEAR") {
    const denom = bounds[1] - bounds[0];
    for (let i = 0; i < work.length; i += 1) staged[i] = ((work[i] ?? 0) - bounds[0]) / denom;
  }
  return staged;
}

function logNormalise(
  work: readonly number[],
  bounds: readonly [number, number],
  out: Float64Array,
): void {
  const eps = logEpsilon(work, bounds[1]);
  const planLo = Math.log10(Math.max(bounds[0], eps));
  let planHi = Math.log10(Math.max(bounds[1], eps));
  if (planHi === planLo) planHi = planLo + DEGENERATE;
  for (let i = 0; i < work.length; i += 1) {
    out[i] = (Math.log10(Math.max(work[i] ?? 0, eps)) - planLo) / (planHi - planLo);
  }
}

/** Place the transformed values back at their input indices, clip to 0..1, then gamma (:540-543). */
function scatter(
  out: Float64Array,
  plan: { indices: Uint32Array; work: readonly number[]; bounds: readonly [number, number]; mode: NormMode; gamma: number },
): Float64Array {
  const staged = transform(plan.work, plan.bounds, plan.mode);
  for (let i = 0; i < staged.length; i += 1) {
    const clipped = Math.min(1, Math.max(0, at(staged, i)));
    const gamma = plan.gamma === 1 ? clipped : clipped ** (1 / plan.gamma);
    out[slot(plan.indices, i)] = gamma;
  }
  return out;
}

/** The indices of the finite samples and their values, in input order (colormaps.py:504). */
function finiteSamples(arr: readonly number[]): { indices: Uint32Array; values: number[] } {
  const indices: number[] = [];
  const values: number[] = [];
  for (let i = 0; i < arr.length; i += 1) {
    const v = arr[i] ?? Number.NaN;
    if (Number.isFinite(v)) {
      indices.push(i);
      values.push(v);
    }
  }
  return { indices: Uint32Array.from(indices), values };
}

/**
 * Normalise values into 0..1. Non-finite inputs come back as NaN, as the source's
 * finite mask (colormaps.py:504) and its NaN fill (:516) do.
 */
export function normalise(
  values: Float64Array | Uint32Array | readonly number[],
  options: NormOptions,
): Float64Array {
  const arr = Array.from(values);
  // Non-finite inputs keep the NaN the source's finite mask gives them (:516).
  const out = new Float64Array(arr.length).fill(Number.NaN);
  if (arr.length === 0) return out;
  const { indices, values: finite } = finiteSamples(arr);
  if (finite.length === 0) return out;
  const gamma = options.gamma === undefined || !(options.gamma > 0) || !Number.isFinite(options.gamma)
    ? 1
    : options.gamma;
  const bounds = effectiveBounds(finite, options);
  const work = finite.map((v) => Math.min(bounds[1], Math.max(bounds[0], v)));
  const mode: NormMode = NORM_MODES.includes(options.mode) ? options.mode : "LINEAR";
  return scatter(out, { indices, work, bounds, mode, gamma });
}
