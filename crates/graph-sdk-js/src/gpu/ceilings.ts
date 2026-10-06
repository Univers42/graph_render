/**
 * `ceilings.ts` — the measured per-arm ceilings, split from `bounds.ts` to keep that file under
 * the house's 300-line limit.
 *
 * A **ceiling** is *measured*, per `(arm, n, state)`, and is the arm held to its own row. It is
 * not derived: the table below was filled from what the `charge-hardware` and `charge-software`
 * runs printed, and a later run of the same arm on the same device must sit at or under its own
 * row. The arm is held to its own row because a software adapter's reassociation and
 * transcendentals are not the hardware's.
 *
 * ## The f32-spacing floor, and why it is a Caveat and not a test
 *
 * A `maxAbs` ceiling below **2⁻¹¹ units at 1M is not a tighter gate, it is an unmeasurable
 * one**: positions are `f32`, the seed spiral reaches `12·√n = 12 000` units, and `f32`'s
 * spacing there is **2⁻¹⁰**, so a velocity error under 2⁻¹¹ units is lost when integrated
 * into a rim position at that magnitude (`gpu-force-tier.md:91-93`). A ceiling under the floor
 * gains nothing at 1M. It is *not* that the delta cannot be read — the delta itself comes back
 * as `f32` and is compared as such — it is that a number under the floor describes an
 * integration step the integrator cannot carry.
 *
 * **The plan's `the_ceilings_are_ordered` (`B_max ≥ 2⁻¹¹·h`) is dropped.** It contradicts its
 * own derivation: `2⁻¹¹ · 26.9 = 0.013`, which is thirteen thousand times the 3.7e-5 the plan
 * simultaneously derives for the same case. A test cannot hold both. The rule kept here is the
 * guard ordering in `bounds.ts`, which is checkable and not self-contradictory.
 */

/** The arms the ceiling table is keyed by. */
export type Arm = "hardware" | "software";

/**
 * One arm's measured numbers at one `(n, state)`.
 *
 * `rmsRel` and `maxAbs` are the measured values rounded **up** to two significant digits, so a
 * later run of the same arm on the same device sits at or under its own row rather than a
 * rounding step either side of it.
 */
export interface Ceiling {
  readonly rmsRel: number;
  readonly maxAbs: number;
}

/**
 * The measured ceilings, keyed by `${arm}:${n}:${state}`.
 *
 * Filled from this host's `charge-hardware` (all eight fixtures) and `charge-software` (1k, 10k,
 * 50k) runs. `rmsRel` and `maxAbs` are each rounded **up** to two significant digits, so a later
 * run of the same arm on the same device sits at or under its own row rather than a rounding step
 * either side of it. The 1M software rows were not run — see `docs/measurements/gpu-g1.md`.
 */
const CEILINGS: Record<string, Ceiling> = {
  "hardware:1000:0": { rmsRel: 1.5e-7, maxAbs: 7.7e-5 },
  "hardware:1000:1": { rmsRel: 3.2e-7, maxAbs: 3.5e-5 },
  "hardware:10000:0": { rmsRel: 4.5e-7, maxAbs: 1.6e-4 },
  "hardware:10000:1": { rmsRel: 6e-7, maxAbs: 1.1e-4 },
  "hardware:50000:0": { rmsRel: 2.4e-6, maxAbs: 5e-4 },
  "hardware:50000:1": { rmsRel: 2.3e-6, maxAbs: 6.3e-4 },
  "hardware:1000000:0": { rmsRel: 7.5e-5, maxAbs: 9e-3 },
  "hardware:1000000:1": { rmsRel: 3.7e-5, maxAbs: 2.4e-2 },
  "software:1000:0": { rmsRel: 1.9e-7, maxAbs: 7.7e-5 },
  "software:1000:1": { rmsRel: 3.9e-7, maxAbs: 4.6e-5 },
  "software:10000:0": { rmsRel: 4.6e-7, maxAbs: 1.7e-4 },
  "software:10000:1": { rmsRel: 6.1e-7, maxAbs: 1.3e-4 },
  "software:50000:0": { rmsRel: 2.4e-6, maxAbs: 5e-4 },
  "software:50000:1": { rmsRel: 2.3e-6, maxAbs: 6e-4 },
};

/** This arm's ceiling for one fixture, or `undefined` when none has been measured. */
export function ceilingFor(arm: Arm, n: number, state: number): Ceiling | undefined {
  return CEILINGS[`${arm}:${n}:${state}`];
}

/**
 * Records one measured ceiling row.
 *
 * A test hook, not a runtime path: the production table is filled by hand from a measurement
 * run, and this lets a test set a row without editing the literal. It writes the same key
 * `ceilingFor` reads, so a test can hold an arm to a ceiling the production table does not have.
 */
export function setCeiling(arm: Arm, n: number, state: number, ceiling: Ceiling): void {
  CEILINGS[`${arm}:${n}:${state}`] = ceiling;
}

/** Every ceiling row, for the ordering test and for a report that wants the whole table. */
export function ceilings(): Readonly<Record<string, Ceiling>> {
  return CEILINGS;
}
