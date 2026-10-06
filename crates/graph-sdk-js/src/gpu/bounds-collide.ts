/**
 * `bounds-collide.ts` — what the collide arm's numbers are held to, its one exactness check,
 * and the report the harness prints.
 *
 * ## The guard and the ceilings
 *
 * The **guard** is mixed, absolute plus relative (Amendment 1):
 *
 * `rmsAbs ≤ 1e-4 · rmsRef + k_c · 5 · 2⁻²³ · P`
 *
 * As a relative guard for `compare()`: `1e-4 + k_c · 5 · 2⁻²³ · P / rmsRef`.
 *
 * - `1e-4 · rmsRef` is the relative part: the plan's `1e-4`, a bound on the rounding of the
 *   *net* delta.
 * - `k_c · 5 · 2⁻²³ · P` is the absolute part: the Higham bound `γₖ·Σ|xᵢ|` for the sum of `k_c`
 *   pushes, each bounded by `P`, each rounded at `5 · 2⁻²³` (condition 8's per-term rounding —
 *   a product 0.5, `sqrt` 2, a division 2.5 ULP, an `f32` ULP being `2⁻²³`).
 * - `k_c` is the largest number of contacts of any one node, counted in `f64` on the host from
 *   the fixture's own positions, with the grid the device sorts with.
 * - `P` bounds one push: `(reach − dist) · 0.5` is at most `reach / 2 = collide_radius`
 *   (`collide.rs:254-256`). The `delta_collide` column carries no strength — `collide_pass`
 *   merges the push straight through `motion::merge` (`particle_mesh.rs:85-95`), and the only
 *   collide parameter is `collide_radius` — so `P = reach / 2`.
 *
 * The two parts are both needed because the error scales with the pushes, not with their net:
 * at equilibrium the pushes nearly cancel, so `rmsRef` is tiny while the rounding error is not,
 * and a relative-only guard is a bound on a near-zero denominator.
 *
 * The **ceilings** are measured, per `(arm, n, state)`, rounded **up** to two significant
 * digits; a missing row means guard only. The comparator is `bounds.ts`'s `compare`.
 *
 * Caveat: `k_c` is measured on the fixture's positions. A denser crowd raises it, so the guard
 * is only as good as the fixture's measured crowd, the same limit condition 8 states for link.
 * The ceilings are one device's numbers on one driver stack; a driver update re-measures them.
 *
 * ## `order`, the check no bound can make
 *
 * The resolve sums each node's candidates in the CPU's order, and that order is the member
 * lists' (`collide.rs:150-165`: a stable counting sort, so each bucket lists its nodes by
 * ascending index). A device that listed them in another order would sum the same terms in
 * another order: a rounding difference, under every `f32` bound (plan `:1333-1338`). So the
 * order is read back and checked as a fact, and `--break collide-order` is what proves the
 * check reads it. `--break collide-window` is the control a bound must catch: it drops a
 * contact, an O(1) error (condition 9).
 */

import { compare, components } from "./bounds.ts";
import type { Arm, Ceiling } from "./bounds.ts";
import { cellOf, gridFor } from "./collide.ts";
import type { Grid } from "./collide.ts";
import type { Fixture } from "./fixture.ts";
import type { PassReport } from "./pass-report.ts";

/** The relative part of the guard: the plan's `1e-4`, a bound on the net delta's rounding. */
export const COLLIDE_RMS_REL_GUARD = 1e-4;

/** Condition 8's per-term rounding: a product 0.5, `sqrt` 2, a division 2.5 ULP; an f32 ULP is 2⁻²³. */
const TERM_ROUNDING = 5 * 2 ** -23;

/**
 * The guard for one fixture: `1e-4 + k_c · 5 · 2⁻²³ · P / rmsRef`, with `P = reach / 2`.
 *
 * `k_c` is the fixture's largest crowd, `rmsRef` its reference's rms, `reach` the grid's.
 */
export function collideGuard(k_c: number, reach: number, rmsRef: number): number {
  const P = reach / 2;
  const absolute = k_c * TERM_ROUNDING * P;
  return rmsRef === 0 ? COLLIDE_RMS_REL_GUARD : COLLIDE_RMS_REL_GUARD + absolute / rmsRef;
}

/**
 * The largest number of contacts of any one node, in `f64`, over the fixture's positions.
 *
 * A contact is a pair closer than `reach` (the CPU's `d2` test, `collide.rs:242`), counted on the
 * host in `f64` from the fixture's own positions. The neighbourhood is the grid's — the same
 * `cellOf` and row table the device sorts with — so the count is the device's candidate count,
 * with the distance test the CPU's.
 *
 * Caveat: the count is the fixture's own crowd. A denser overlap raises it, so the guard is only
 * as good as the fixture's measured crowd, the same limit condition 8 states for link.
 */
export function maxContacts(posX: Float64Array, posY: Float64Array, grid: Grid): number {
  const n = posX.length;
  const cell = new Array(n);
  const bucket = new Uint32Array(n);
  for (let i = 0; i < n; i += 1) {
    const cx = cellOf(posX[i] ?? 0, grid.originX, grid.invSize);
    const cy = cellOf(posY[i] ?? 0, grid.originY, grid.invSize);
    cell[i] = [cx, cy];
    bucket[i] = bucketOf(cx, cy, grid);
  }
  const counts = new Uint32Array(grid.buckets);
  for (let i = 0; i < n; i += 1) counts[bucket[i] ?? 0] = (counts[bucket[i] ?? 0] ?? 0) + 1;
  const start = new Uint32Array(grid.buckets + 1);
  for (let b = 0; b < grid.buckets; b += 1) start[b + 1] = (start[b] ?? 0) + (counts[b] ?? 0);
  const order = new Uint32Array(n);
  const cursor = start.slice(0, grid.buckets);
  for (let i = 0; i < n; i += 1) {
    const b = bucket[i] ?? 0;
    order[cursor[b] ?? 0] = i;
    cursor[b] = (cursor[b] ?? 0) + 1;
  }
  let max = 0;
  for (let i = 0; i < n; i += 1) {
    const [cx, cy] = cell[i] ?? [0, 0];
    let contacts = 0;
    for (const b of readsBuckets(cx, cy, grid)) {
      for (let q = start[b] ?? 0; q < (start[b + 1] ?? 0); q += 1) {
        const j = order[q] ?? 0;
        if (j === i) continue;
        const dx = (posX[i] ?? 0) - (posX[j] ?? 0);
        const dy = (posY[i] ?? 0) - (posY[j] ?? 0);
        if (dx * dx + dy * dy < grid.d2) contacts += 1;
      }
    }
    if (contacts > max) max = contacts;
  }
  return max;
}

/** The bucket of cell `(cx, cy)`: `hash.rs:24-25` with the row table. */
function bucketOf(cx: number, cy: number, grid: Grid): number {
  const at = Math.min(Math.max(cy + 1, 0), grid.rows.length - 1);
  return ((grid.rows[at] ?? 0) + cx) & grid.mask;
}

/** The buckets a query from cell `(cx, cy)` reads, each once, in `Grid::reads` order. */
function readsBuckets(cx: number, cy: number, grid: Grid): number[] {
  const at: number[] = [];
  const firsts: number[] = [];
  for (let row = 0; row < 3; row += 1) {
    const first = bucketOf(cx - 1, cy - 1 + row, grid);
    for (let t = 0; t < 3; t += 1) {
      const b = (first + t) & grid.mask;
      let seen = false;
      for (let e = 0; e < row; e += 1) seen = seen || (((b - (firsts[e] ?? 0)) & grid.mask) < 3);
      if (!seen) at.push(b);
    }
    firsts[row] = first;
  }
  return at;
}

/** One measured ceiling row, with the fixture's own crowd and reference the guard is built from. */
export interface CollideCeiling extends Ceiling {
  /** The fixture's largest crowd, the guard's `k_c`. */
  readonly k_c: number;
  /** The fixture's reference rms, the guard's `rmsRef`. */
  readonly rmsRef: number;
}

/** The measured ceilings, keyed by `${arm}:${n}:${state}`. Empty until the first run. */
const CEILINGS: Readonly<Record<string, CollideCeiling>> = {
  "hardware:1000:0": { rmsRel: 4.1e-6, maxAbs: 3.3e-5, k_c: 8, rmsRef: 1.72612 },
  "hardware:1000:1": { rmsRel: 5.5e-4, maxAbs: 2.7e-5, k_c: 4, rmsRef: 0.007189 },
  "hardware:10000:0": { rmsRel: 2.2e-5, maxAbs: 1.3e-4, k_c: 8, rmsRef: 1.00834 },
  "hardware:10000:1": { rmsRel: 6.9e-6, maxAbs: 1.2e-4, k_c: 9, rmsRef: 2.6824 },
  "hardware:50000:0": { rmsRel: 8.1e-5, maxAbs: 3.9e-4, k_c: 8, rmsRef: 0.67192 },
  "hardware:50000:1": { rmsRel: 7.4e-6, maxAbs: 4.7e-3, k_c: 23, rmsRef: 9.91163 },
  "software:1000:0": { rmsRel: 4.1e-6, maxAbs: 3.2e-5, k_c: 8, rmsRef: 1.72612 },
  "software:1000:1": { rmsRel: 5.4e-4, maxAbs: 2.7e-5, k_c: 4, rmsRef: 0.007189 },
  "software:10000:0": { rmsRel: 2.2e-5, maxAbs: 1.3e-4, k_c: 8, rmsRef: 1.00834 },
  "software:10000:1": { rmsRel: 6.9e-6, maxAbs: 1.2e-4, k_c: 9, rmsRef: 2.6824 },
  "software:50000:0": { rmsRel: 8.1e-5, maxAbs: 3.9e-4, k_c: 8, rmsRef: 0.67192 },
  "software:50000:1": { rmsRel: 7.4e-6, maxAbs: 4.7e-3, k_c: 23, rmsRef: 9.91163 },
};

/** Every ceiling row, for the guard test. */
export function collideCeilings(): Readonly<Record<string, CollideCeiling>> {
  return CEILINGS;
}

/** One run's read-back: the per-node increment, the member lists and their spans. */
export interface CollideRan {
  readonly delta: Float32Array;
  readonly order: Uint32Array;
  readonly start: Uint32Array;
}

/** Which device ran, for the ceiling's key and the report's marks. */
export interface Ran {
  readonly arm: Arm;
  readonly marks: string;
  readonly fallback: boolean;
}

/**
 * `null` when every bucket's member list is strictly ascending and together they hold each of
 * the `n` nodes once; otherwise the first violation, in words, for the failure line.
 *
 * `start` is the scan's output, `buckets + 1` entries, bucket `b`'s span `start[b]..start[b + 1]`.
 */
export function orderCheck(order: Uint32Array, start: Uint32Array, n: number): string | null {
  const buckets = start.length - 1;
  if (start[0] !== 0 || start[buckets] !== n || order.length < n) {
    return `the spans run ${start[0]}..${start[buckets]}, not n = ${n}`;
  }
  const seen = new Uint8Array(n);
  for (let b = 0; b < buckets; b += 1) {
    const lo = start[b] ?? 0;
    const hi = start[b + 1] ?? 0;
    for (let q = lo; q < hi; q += 1) {
      const node = order[q] ?? n;
      if (node >= n || seen[node] === 1) {
        return `slot ${q} holds node ${node}, out of range or listed twice`;
      }
      seen[node] = 1;
      if (q > lo && node <= (order[q - 1] ?? 0)) {
        return `bucket ${b} is not ascending at slot ${q}: node ${order[q - 1]} then ${node}`;
      }
    }
  }
  return null;
}

/**
 * The verdict: one word per breached check — `order`, `repeat`, `guard`, `rms`, `max` — and
 * all of them, never only the first.
 */
export function collideVerdict(input: {
  readonly n: number;
  readonly state: number;
  readonly arm: Arm;
  readonly rmsRel: number;
  readonly rmsRef: number;
  readonly k_c: number;
  readonly reach: number;
  readonly maxAbs: number;
  readonly repeatEqual: boolean;
  /** `orderCheck`'s answer: `null` when the order held. */
  readonly order: string | null;
}): { readonly pass: boolean; readonly failures: readonly string[] } {
  const failures: string[] = [];
  const where = `(n=${input.n}, state=${input.state})`;
  if (input.order !== null) {
    failures.push(`order ${where}: ${input.order}`);
  }
  if (!input.repeatEqual) {
    failures.push(`repeat ${where}: the second run's bytes differ from the first's`);
  }
  const guard = collideGuard(input.k_c, input.reach, input.rmsRef);
  if (input.rmsRel > guard) {
    failures.push(`guard ${where}: rmsRel ${input.rmsRel} over 1e-4 + k_c=${input.k_c}·5·2⁻²³·${input.reach / 2}/${input.rmsRef} = ${guard}`);
  }
  const ceiling = CEILINGS[`${input.arm}:${input.n}:${input.state}`];
  if (ceiling && input.rmsRel > ceiling.rmsRel) {
    failures.push(`rms ${where}: ${input.rmsRel} over this arm's ceiling ${ceiling.rmsRel}`);
  }
  if (ceiling && input.maxAbs > ceiling.maxAbs) {
    failures.push(`max ${where}: ${input.maxAbs} over this arm's ceiling ${ceiling.maxAbs}`);
  }
  return { pass: failures.length === 0, failures };
}

/**
 * Two runs against the fixture's `delta_collide`: the comparator on the second, `order` on the
 * second's lists, and the two runs' delta and order compared byte for byte.
 */
export function collideReport(fixture: Fixture, runs: readonly [CollideRan, CollideRan], ran: Ran): PassReport {
  const [first, second] = runs;
  const reference = components(fixture.delta.collide.x, fixture.delta.collide.y);
  const measured = compare({ got: second.delta, reference });
  const repeatEqual = sameBytes(first.delta, second.delta) && sameBytes(first.order, second.order);
  const order = orderCheck(second.order, second.start, fixture.n);
  const grid = gridFor(fixture.posX, fixture.posY);
  const k_c = maxContacts(fixture.posX, fixture.posY, grid);
  const verdict = collideVerdict({
    n: fixture.n,
    state: fixture.state,
    arm: ran.arm,
    rmsRel: measured.rmsRel,
    rmsRef: measured.rmsRef,
    k_c,
    reach: grid.reach,
    maxAbs: measured.maxAbs,
    repeatEqual,
    order,
  });
  return {
    kind: "collide",
    n: fixture.n,
    state: fixture.state,
    ...measured,
    repeatEqual,
    exact: { order: order === null },
    pass: verdict.pass,
    failures: verdict.failures,
    marks: ran.marks,
    fallback: ran.fallback,
  };
}

/** Byte equality, not numeric: `-0` against `+0` is a difference. */
function sameBytes(left: Float32Array | Uint32Array, right: Float32Array | Uint32Array): boolean {
  const a = new Uint8Array(left.buffer, left.byteOffset, left.byteLength);
  const b = new Uint8Array(right.buffer, right.byteOffset, right.byteLength);
  return a.length === b.length && a.every((byte, k) => byte === b[k]);
}
