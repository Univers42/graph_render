/**
 * `bounds.ts` — what the charge arm's numbers are held to, and what the pass reports.
 *
 * Two kinds of number live here and they are not the same kind.
 *
 * - A **guard** is *derived*. It is a bound the arm must not cross, and a breach is a stop to
 *   report with both numbers, never a re-tune. There are two: `rmsRel ≤ 1e-4` at every
 *   fixture, and at the two 1M fixtures `maxAbs ≤ |charge·alpha| · (2⁻¹¹/√3) · ‖g‖₂ · 6`,
 *   with `‖g‖₂ = P · ‖spectrum‖₂` read from the fixture's own spectrum section.
 * - A **ceiling** is *measured*, per `(arm, n, state)`, and is the arm held to its own row. The
 *   table below was empty when the first run went out and was filled from what it printed; the
 *   derivation that produced the guards is condition 7's, corrected by Amendment 2
 *   (`docs/decisions/gpu-g1.md`), and restated here.
 *
 * ## The guards' arithmetic, spelled out
 *
 * `rmsRel ≤ 1e-4`. That is **1.5 orders of magnitude** under the mesh's own best error against
 * the exact all-pairs sum, `3.6e-3` rms (`docs/decisions/gpu-force-tier.md:27`, restated
 * `fixtures/gpu/README.md:148-149`). The plan's own text said "one order of magnitude", which
 * is wrong by a factor of 3.6 and condition 7 requires the corrected wording.
 *
 * `maxAbs ≤ |charge·alpha| · (2⁻¹¹/√3) · ‖g‖₂ · 6` at 1M. The deposit quantum at 1M is 2⁻¹¹ of
 * a unit charge; four CIC weights each rounded with independent uniform error over one quantum
 * give rms `2⁻¹¹/√12` apiece, and four in quadrature give **`2⁻¹¹/√3 = 2.8e-4`** of a unit per
 * occupied cell. That deposit term is right. Two steps after it were wrong, and Amendment 2
 * corrects them:
 *
 * 1. **The propagation factor.** The field is the convolution of the density error with the
 *    sampled kernel `g`, so the field error's rms is `δ_rms · ‖g‖₂`, not `δ_rms / h²`. The
 *    kernel `G = −r/l(r)` is a 1/r law, and `‖g‖₂ = P · ‖spectrum‖₂` comes from the fixture's
 *    own spectrum section — 0.2085 on the settled 1M fixture, 0.1712 on the start one. The old
 *    `1/h²` is about 100× smaller than `‖g‖₂`.
 * 2. **An rms read as a maximum.** The guard now carries a peak factor over the `2n` read
 *    components: `√(2·ln 2n)` = 5.39 at n = 1e6, rounded up to **6**.
 *
 * So the guard is `|charge·alpha| · (2⁻¹¹/√3) · ‖g‖₂ · 6`, which is 3.17e-2 settled and 2.61e-2
 * start. `‖spectrum‖₂` is computed from the fixture's spectrum section, as condition 8 reads
 * `k` from the fixture's own edges.
 *
 * Caveat: this is a statistical bound, not a strict one. It models the deposit error as white
 * noise and its peak as a Gaussian. The fixed-point weights sum to exactly one per node, which
 * makes the true error field slightly smaller than the model. The f32 transcription sits 7%
 * under the settled value. Another seed's 1M maximum could cross it, and that is a stop, not a
 * re-tune.
 *
 * **There is no `maxAbs` guard below 1M.** The deposit quantum is not the dominant error
 * there — at 1k the scale is 2²¹ and the quantum 2⁻²¹, thirty orders below the field — so a
 * guard derived from it would be a number 10¹³ under everything else and would certify
 * anything.
 *
 * The measured ceilings themselves — the f32-spacing floor, the dropped `the_ceilings_are_ordered`
 * test, and the one-device caveat below — are in `ceilings.ts`, split out to keep this file under
 * the house's 300-line limit.
 *
 * Caveat: **these are one device's numbers on one driver stack.** A driver update re-measures
 * them; it does not widen them. The per-arm rows are the decision: a software adapter's `f32`
 * is the same `f32`, but its reassociation and its transcendentals are not the hardware's, and
 * the record's claim is per-device repeatability, not cross-device equality
 * (`gpu-force-tier.md:56-58`). A ceiling widened until both arms passed would be a number
 * about nothing.
 */

import { scaleFor } from "./fixture.ts";
import { ceilingFor } from "./ceilings.ts";
import type { Arm } from "./ceilings.ts";

// The measured ceilings live in `ceilings.ts`, split out to keep this file under the house's
// 300-line limit. They are re-exported here so the tests and `readback.ts` keep one import path.
export { ceilingFor, ceilings, setCeiling } from "./ceilings.ts";
export type { Arm, Ceiling } from "./ceilings.ts";

/**
 * The `rmsRel` guard: 1.5 orders of magnitude under the mesh's own 3.6e-3 rms against the
 * exact sum.
 */
export const RMS_REL_GUARD = 1e-4;

/** The deposit quantum at 1M, `2⁻¹¹` of a unit charge (`fixtures/gpu/README.md:140`). */
const QUANTUM_AT_1M = 2 ** -11;

/**
 * The four rounded weights in quadrature: `2⁻¹¹/√3 = 2.8e-4` of a unit per occupied cell.
 *
 * Condition 7's correction. The plan's printed formula — two roots times the quantum times root
 * 1.25 — is 7.7e-4 and does not produce the record's figure; the derivation is four independent
 * uniform errors over one quantum, each `2⁻¹¹/√12`, summed in quadrature.
 */
const FOUR_WEIGHTS_IN_QUADRATURE = QUANTUM_AT_1M / Math.sqrt(3);

/** `params.charge · alpha` at `alpha = 1`, the value the fixture's deltas are at. */
const CHARGE_ALPHA = 90;

/**
 * The peak factor over the `2n` read components: `√(2·ln 2n)` = 5.39 at n = 1e6, rounded up.
 *
 * The deposit term is an rms; a maximum over 2n components sits several rms out. This converts
 * one to the other, and rounding up keeps the guard a bound rather than an estimate.
 */
const PEAK_FACTOR = 6;

/**
 * The kernel's 2-norm, `‖g‖₂ = P · ‖spectrum‖₂`, from the fixture's own spectrum section.
 *
 * The spectrum is pre-scaled by `1/P²` (`kernel.rs:66-76`), so the `P` here undoes the sampling
 * density and leaves the kernel's actual 2-norm. Computed in `f64` on the host, once, from the
 * fixture's `spectrum_re` / `spectrum_im` — the same way condition 8 reads `k` from the
 * fixture's own edges.
 */
export function kernelNorm(side: number, spectrumRe: Float64Array, spectrumIm: Float64Array): number {
  let sum = 0;
  for (let k = 0; k < spectrumRe.length; k += 1) {
    const re = spectrumRe[k] ?? 0;
    const im = spectrumIm[k] ?? 0;
    sum += re * re + im * im;
  }
  return side * Math.sqrt(sum);
}

/**
 * The derived `maxAbs` guard at 1M: `|charge·alpha| · (2⁻¹¹/√3) · ‖g‖₂ · 6` units per tick.
 *
 * `kernelNorm` is `‖g‖₂ = P · ‖spectrum‖₂`, computed from the fixture's own spectrum section.
 * The two 1M fixtures have different spectra — `‖g‖₂` is 0.2085 settled and 0.1712 start — so
 * the guard is computed per fixture and not from one of them.
 */
export function maxAbsGuard(kernelNorm: number): number {
  return CHARGE_ALPHA * FOUR_WEIGHTS_IN_QUADRATURE * kernelNorm * PEAK_FACTOR;
}

/** One arm's per-node velocity increment as it came back, and the reference it is against. */
export interface Measurement {
  /** The arm's own `delta_charge`, `2n` components, already narrowed to `f32`. */
  readonly got: Float32Array;
  /** The fixture's `delta_charge`, `2n` components, in row order `x₀y₀x₁y₁…`. */
  readonly reference: Float64Array;
}

/**
 * The comparator: `rmsAbs`, `rmsRef`, `rmsRel` and `maxAbs` over the `2n` axis components.
 *
 * The reference is narrowed with `Math.fround` **here**, once, and not before: an already-`f32`
 * reference could not show a 2⁻¹¹ difference, which is the whole reason the format stores
 * `f64` (`fixtures/gpu/README.md:50-53`). `rmsRel` is relative to `rmsRef`, the CPU's *own*
 * increment, and not to 1: the increment's magnitude scales with `charge·alpha`
 * (`charge.rs:44`), so an absolute bound would be a statement about the parameters rather than
 * about the kernel.
 *
 * The reduction is a fixed-order left fold over the components in index order. `f64` addition
 * is not associative, so the sum is one specific sum; the CPU's own value at these magnitudes
 * has enough headroom that a reassociation is far under the 1e-4 guard, but the fold is still
 * written once and in one order so the number is reproducible.
 */
export function compare(measurement: Measurement): {
  rmsAbs: number;
  rmsRef: number;
  rmsRel: number;
  maxAbs: number;
} {
  const { got, reference } = measurement;
  let sumAbs = 0;
  let sumRef = 0;
  let maxAbs = 0;
  for (let k = 0; k < got.length; k += 1) {
    const want = Math.fround(reference[k] ?? 0);
    const diff = Math.abs((got[k] ?? 0) - want);
    sumAbs += diff * diff;
    sumRef += want * want;
    if (diff > maxAbs) {
      maxAbs = diff;
    }
  }
  const count = got.length;
  const rmsAbs = Math.sqrt(sumAbs / count);
  const rmsRef = Math.sqrt(sumRef / count);
  return { rmsAbs, rmsRef, rmsRel: rmsRef === 0 ? 0 : rmsAbs / rmsRef, maxAbs };
}

/** One word per breached check. The comparator names a failure, it does not explain it. */
export type Failure = "rms" | "max" | "deposit" | "repeat" | "bounds" | "guard";

/**
 * The verdict: `pass` is false when any guard or ceiling is exceeded, or when one of the
 * exactness checks failed.
 *
 * `failures` names each breached check with one of six words, and **all** of them: a pass that
 * hides a second breach is how a green row reports a broken kernel, so the list is complete
 * rather than short-circuited at the first.
 */
export function verdict(input: {
  readonly n: number;
  readonly state: number;
  readonly arm: Arm;
  /** The kernel's 2-norm `‖g‖₂ = P · ‖spectrum‖₂`, for the 1M `maxAbs` guard. */
  readonly kernelNorm: number;
  readonly rmsRel: number;
  readonly maxAbs: number;
  readonly depositedUnits: number;
  readonly repeatEqual: boolean;
  /** Which of the two runs' columns differed, when `repeatEqual` is false. */
  readonly repeatDetail: string;
  readonly boundsExact: boolean;
}): { readonly pass: boolean; readonly failures: readonly string[] } {
  const failures: string[] = [];
  const where = `(n=${input.n}, state=${input.state})`;
  if (!input.boundsExact) {
    failures.push(`bounds ${where}: the device's min/max is not the host's, bit for bit`);
  }
  const want = expectedUnits(input.n);
  if (input.depositedUnits !== want) {
    failures.push(`deposit ${where}: ${input.depositedUnits} units, not ${want}`);
  }
  if (!input.repeatEqual) {
    failures.push(`repeat ${where}: ${input.repeatDetail}`);
  }
  if (input.rmsRel > RMS_REL_GUARD) {
    failures.push(`guard ${where}: rmsRel ${input.rmsRel} over the ${RMS_REL_GUARD} guard`);
  }
  const ceiling = ceilingFor(input.arm, input.n, input.state);
  if (ceiling && input.rmsRel > ceiling.rmsRel) {
    failures.push(`rms ${where}: ${input.rmsRel} over this arm's ceiling ${ceiling.rmsRel}`);
  }
  if (ceiling && input.maxAbs > ceiling.maxAbs) {
    failures.push(`max ${where}: ${input.maxAbs} over this arm's ceiling ${ceiling.maxAbs}`);
  }
  const guard = maxAbsGuard(input.kernelNorm);
  if (input.n === 1_000_000 && input.maxAbs > guard) {
    failures.push(`max ${where}: ${input.maxAbs} over the derived 1M guard ${guard}`);
  }
  return { pass: failures.length === 0, failures };
}

/**
 * The exact fixed-point total every fixture must deposit: `n · scaleFor(n)`.
 *
 * An equality with no tolerance, and the one check in the tier with none
 * (`gpu-force-tier.md:48-52`). It catches a deposit that is merely *close*, which every other
 * number here would accept. `scaleFor` is the loader's, so the two arms cannot disagree about
 * the scale by construction — and `negctl-weight` is what proves the scale is *read* from it.
 */
export function expectedUnits(n: number): number {
  const scale = scaleFor(n);
  if (scale === undefined) {
    throw new Error(`gpu: scaleFor(${n}) is undefined, and n·scale is what must be deposited`);
  }
  return n * scale;
}

/** The `2n` components a fixture's `delta_charge` compares over, `x` then `y` per node. */
export function components(x: Float64Array, y: Float64Array): Float64Array {
  const out = new Float64Array(x.length * 2);
  for (let k = 0; k < x.length; k += 1) {
    out[k * 2] = x[k] ?? 0;
    out[k * 2 + 1] = y[k] ?? 0;
  }
  return out;
}