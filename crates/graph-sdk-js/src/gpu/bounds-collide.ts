/**
 * `bounds-collide.ts` — what the collide arm's numbers are held to, its one exactness check,
 * and the report the harness prints.
 *
 * ## The guard and the ceilings
 *
 * The **guard** is `rmsRel ≤ 1e-4`, the plan's table (`:1286-1290`), checked against the
 * deposit quantum 2⁻¹¹ and not the mesh's 3.6e-3. A breach is a stop to report with both
 * numbers, never a re-tune. The **ceilings** are measured, per `(arm, n, state)` as charge's
 * are, rounded **up** to two significant digits; a missing row means guard only. The
 * comparator is `bounds.ts`'s `compare`, the same `f64` fold over the `2n` components with the
 * reference narrowed by `Math.fround` once.
 *
 * Caveat: the ceilings are one device's numbers on one driver stack. A driver update
 * re-measures them; it does not widen them.
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
import type { Fixture } from "./fixture.ts";
import type { PassReport } from "./pass-report.ts";

/** The `rmsRel` guard every collide case is held to, row or no row. */
export const COLLIDE_RMS_REL_GUARD = 1e-4;

/** The measured ceilings, keyed by `${arm}:${n}:${state}`. Empty until the first run. */
const CEILINGS: Readonly<Record<string, Ceiling>> = {};

/** Every ceiling row, for the guard test. */
export function collideCeilings(): Readonly<Record<string, Ceiling>> {
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
  if (input.rmsRel > COLLIDE_RMS_REL_GUARD) {
    failures.push(`guard ${where}: rmsRel ${input.rmsRel} over the ${COLLIDE_RMS_REL_GUARD} guard`);
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
  const verdict = collideVerdict({
    n: fixture.n,
    state: fixture.state,
    arm: ran.arm,
    rmsRel: measured.rmsRel,
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
