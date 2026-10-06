/**
 * `bounds-link.ts` — what the link arm's numbers are held to, and its `PassReport`.
 *
 * Two kinds of number, as in `bounds.ts`, and only the comparator is shared with it.
 *
 * - The **guard** is *derived*: `rmsRel ≤ k_measured · 5 · 2⁻²³` (condition 8). A breach is a
 *   stop to report with both numbers, never a re-tune.
 * - A **ceiling** is *measured*, per `(arm, n, state)`, the measured value rounded up to two
 *   significant digits. A missing row means the guard alone, which is how the arm ran once
 *   before anything was measured.
 *
 * ## The guard's arithmetic
 *
 * The pass is a pure gather: no atomics, no transform, only `f32` rounding. Each term of a
 * node's sum costs one subtraction (0.5 ULP), one `sqrt` (2 ULP) and one `x/y` (2.5 ULP), the
 * record's figures for WGSL (`gpu-force-tier.md:98-100`): **5 ULP per term**. An `f32` ULP is
 * **`2⁻²³`**; `2⁻²⁴` is the unit roundoff, half of it. A node sums `k` terms, so the guard is
 * `k · 5 · 2⁻²³`, with `k = max degree + 1` read from the fixture's own `edgeLo`/`edgeHi`.
 * The plan's fixed `k` and the ceiling it printed were both wrong (condition 8).
 *
 * Caveat: the guard is only as good as the fixture's measured `k`. The synthetic model's
 * `earlier` draw is preferential attachment, whose degree tail has no closed form, so `k` is
 * read from the edges every run and never assumed — a graph with a heavier hub gets a looser
 * guard, and a reader comparing two runs must compare their `k` first. It is applied to
 * `rmsRel`, an aggregate, so a single node whose terms cancel can exceed it while the
 * aggregate does not; `maxAbs` is held only to its measured ceiling.
 */

import { compare, components } from "./bounds.ts";
import type { Arm, Ceiling } from "./bounds.ts";
import type { Fixture } from "./fixture.ts";
import type { PassReport } from "./pass-report.ts";

/** One measured row, and the `k_measured` of the fixture it was measured on. */
export interface LinkCeiling extends Ceiling {
  readonly k: number;
}

/** Per-term cost in ULP: subtraction 0.5, `sqrt` 2, `x/y` 2.5 (`gpu-force-tier.md:98-100`). */
const ULP_PER_TERM = 5;

/** One `f32` ULP at 1: `2⁻²³`. `2⁻²⁴` is the unit roundoff, which is half of it. */
const F32_ULP = 2 ** -23;

/**
 * The measured ceilings, keyed by `${arm}:${n}:${state}`: hardware is `amd/rdna-2`, software
 * `google/swiftshader`, both measured 2026-10-06 (`docs/measurements/gpu-g1.md`, G1c). `k` is
 * the fixture's `measuredK`, the same for both states of one `n`.
 */
const CEILINGS: Readonly<Record<string, LinkCeiling>> = {
  "hardware:1000:0": { rmsRel: 8.6e-8, maxAbs: 7.7e-6, k: 40 },
  "hardware:1000:1": { rmsRel: 1.1e-7, maxAbs: 1.2e-5, k: 40 },
  "hardware:10000:0": { rmsRel: 7.6e-8, maxAbs: 4.6e-5, k: 91 },
  "hardware:10000:1": { rmsRel: 9.9e-8, maxAbs: 2.3e-5, k: 91 },
  "hardware:50000:0": { rmsRel: 7.5e-8, maxAbs: 1.3e-4, k: 114 },
  "hardware:50000:1": { rmsRel: 1.1e-7, maxAbs: 1.4e-4, k: 114 },
  "software:1000:0": { rmsRel: 8.2e-8, maxAbs: 7.7e-6, k: 40 },
  "software:1000:1": { rmsRel: 9.1e-8, maxAbs: 7.7e-6, k: 40 },
  "software:10000:0": { rmsRel: 7.3e-8, maxAbs: 4.6e-5, k: 91 },
  "software:10000:1": { rmsRel: 9.1e-8, maxAbs: 2.3e-5, k: 91 },
  "software:50000:0": { rmsRel: 7.2e-8, maxAbs: 1.3e-4, k: 114 },
  "software:50000:1": { rmsRel: 9.5e-8, maxAbs: 1.4e-4, k: 114 },
};

/** This arm's ceiling for one fixture, or `undefined` when none has been measured. */
export function linkCeilingFor(arm: Arm, n: number, state: number): LinkCeiling | undefined {
  return CEILINGS[`${arm}:${n}:${state}`];
}

/** Every ceiling row, for the ordering test. */
export function linkCeilings(): Readonly<Record<string, LinkCeiling>> {
  return CEILINGS;
}

/** `max degree + 1` over the fixture's simple edges: the `k` the guard is built from. */
export function measuredK(fixture: Pick<Fixture, "n" | "edgeLo" | "edgeHi">): number {
  const degree = new Uint32Array(fixture.n);
  let most = 0;
  for (const ends of [fixture.edgeLo, fixture.edgeHi]) {
    for (const node of ends) {
      const now = (degree[node] ?? 0) + 1;
      degree[node] = now;
      most = Math.max(most, now);
    }
  }
  return most + 1;
}

/** The derived guard on `rmsRel`: `k · 5 · 2⁻²³`. */
export function linkGuard(k: number): number {
  return k * ULP_PER_TERM * F32_ULP;
}

/** What the link verdict reads. */
export interface LinkCase {
  readonly n: number;
  readonly state: number;
  readonly arm: Arm;
  readonly k: number;
  readonly rmsRel: number;
  readonly maxAbs: number;
  readonly repeatEqual: boolean;
  /** Where the two runs' bytes first differed, when `repeatEqual` is false. */
  readonly repeatDetail: string;
}

/**
 * The verdict: every breached check, by one of four words — `guard`, `rms`, `max`, `repeat` —
 * and all of them, never the first alone.
 */
export function linkVerdict(input: LinkCase): { readonly pass: boolean; readonly failures: readonly string[] } {
  const failures: string[] = [];
  const where = `(n=${input.n}, state=${input.state})`;
  const guard = linkGuard(input.k);
  if (!(input.rmsRel <= guard)) {
    failures.push(`guard ${where}: rmsRel ${input.rmsRel} over k=${input.k} · 5 · 2⁻²³ = ${guard}`);
  }
  const ceiling = linkCeilingFor(input.arm, input.n, input.state);
  if (ceiling && !(input.rmsRel <= ceiling.rmsRel)) {
    failures.push(`rms ${where}: ${input.rmsRel} over this arm's ceiling ${ceiling.rmsRel}`);
  }
  if (ceiling && !(input.maxAbs <= ceiling.maxAbs)) {
    failures.push(`max ${where}: ${input.maxAbs} over this arm's ceiling ${ceiling.maxAbs}`);
  }
  if (!input.repeatEqual) {
    failures.push(`repeat ${where}: ${input.repeatDetail}`);
  }
  return { pass: failures.length === 0, failures };
}

/** The two runs' columns, and what the browser said about the device. */
export interface LinkRuns {
  readonly first: Float32Array;
  readonly second: Float32Array;
  readonly arm: Arm;
  readonly marks: string;
  readonly fallback: boolean;
}

/** The link `PassReport`: `bounds.ts`'s comparator over `delta_link`, then the verdict. */
export function linkReport(fixture: Fixture, runs: LinkRuns): PassReport {
  const reference = components(fixture.delta.link.x, fixture.delta.link.y);
  const measured = compare({ got: runs.second, reference });
  const differs = firstDifference(runs.first, runs.second);
  const verdicted = linkVerdict({
    n: fixture.n,
    state: fixture.state,
    arm: runs.arm,
    k: measuredK(fixture),
    rmsRel: measured.rmsRel,
    maxAbs: measured.maxAbs,
    repeatEqual: differs < 0,
    repeatDetail: `the second run's delta differs from the first at component ${differs}`,
  });
  return {
    kind: "link",
    n: fixture.n,
    state: fixture.state,
    ...measured,
    repeatEqual: differs < 0,
    exact: {},
    pass: verdicted.pass,
    failures: verdicted.failures,
    marks: runs.marks,
    fallback: runs.fallback,
  };
}

/** The first component whose bits differ between two runs, or `-1`: bytes, not numbers. */
function firstDifference(left: Float32Array, right: Float32Array): number {
  const a = new Uint32Array(left.buffer, left.byteOffset, left.length);
  const b = new Uint32Array(right.buffer, right.byteOffset, right.length);
  if (a.length !== b.length) {
    return 0;
  }
  return a.findIndex((word, k) => word !== b[k]);
}
