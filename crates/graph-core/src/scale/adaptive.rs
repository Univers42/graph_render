//! Adaptive iteration budgets (`prompts/phase-09-scale-bench.md` §4): how many ticks a
//! layout earns at a given graph size, so a big graph returns in bounded work instead of
//! running to a fixed tick count regardless of `n`.
//!
//! **The budget is a pure function of `(n, m)`. Never of elapsed time.** The reference
//! (`SciGraphs/engine/scigraphs_engine/adaptive.py`) adapts at *render* time from measured
//! crowding and a camera; a motor that did the same from a clock would stop at a
//! different tick on a loaded host, and its output would stop being reproducible — D8
//! says no wall-clock in the motor, and the phase's gate greps `graph-core` for exactly
//! `Instant::now|SystemTime|elapsed` to keep it that way. This is therefore a deliberate
//! divergence from the reference, and the only kind of adaptation that can live inside a
//! hashed pipeline: the same graph always gets the same number of ticks, on every target,
//! in every run.
//!
//! The shape is the obvious one. A tick costs `O(n log n + m)` (the gather kernels plus
//! the quadtree rebuild), so the *work* a settle may spend is bounded by
//! [`TICK_WORK`] node-edge units, and the budget is that work divided by one tick's
//! cost — quantised **down to a power of two** so the ladder reads as a ladder (8, 16,
//! 32, 64, 112) instead of a scatter of arbitrary integers, and clamped to
//! `MIN_TICKS..=MAX_TICKS`.
//!
//! Ponytail: a large graph gets fewer ticks and therefore a **less settled** layout —
//! the same picture with the tails still moving. Direction: cosmetic. Escape hatch: the
//! caller passes [`tick_budget_with`] an explicit tick count, which this module honours
//! verbatim; the budget is a default, never an override the caller cannot take back.

/// Total node-edge units one settle may spend, in ticks × per-tick work.
pub const TICK_WORK: f64 = 6.0e6;

/// The floor: below this a layout is not worth starting, so this is also what a huge
/// graph gets rather than a fraction of a tick.
pub const MIN_TICKS: u32 = 8;

/// The ceiling: the full `alphaDecay(0.06)` settle to `alphaMin` 0.001
/// (`layout::force::params::TICKS`), which anything at or under [`FULL_TICKS_N`] earns.
pub const MAX_TICKS: u32 = 112;

/// The largest power of two at or below [`MAX_TICKS`]: the top of the quantised ladder.
/// 64 and 112 are within a factor of two of each other, so a share between them lands on
/// 64 — the quantisation is downward, never to a value the ladder does not contain.
pub const TOP_POWER_OF_TWO: u32 = 64;

/// The ticks a graph of `n` nodes and `m` edges earns, from [`tick_budget_with`]'s
/// default. `n == 0` earns the floor: there is nothing to lay out and no work to bound.
pub fn tick_budget(n: u32, m: u32) -> u32 {
    quantise(work_share(n, m))
}

/// The same budget, with the caller's explicit `override` honoured verbatim when it is
/// `Some` — the escape hatch the Ponytail marker names. `None` is the size-derived
/// default; an override of `0` is honoured as `0`, because "run no ticks" is a decision
/// the caller is entitled to make.
pub fn tick_budget_with(n: u32, m: u32, override_ticks: Option<u32>) -> u32 {
    override_ticks.unwrap_or_else(|| tick_budget(n, m))
}

/// The share of [`TICK_WORK`] one tick at this size is worth: `TICK_WORK / (n log2(n+1) + m)`,
/// or `0.0` for an empty graph — nothing to lay out, so the floor is the honest answer and
/// the ceiling would be a claim about work that does not exist.
fn work_share(n: u32, m: u32) -> f64 {
    let n = f64::from(n);
    if n <= 0.0 {
        return 0.0;
    }
    let work = n * libm::log2(n + 1.0) + f64::from(m);
    TICK_WORK / work
}

/// `work_share` quantised down to a power of two and clamped to `MIN_TICKS..=MAX_TICKS`.
fn quantise(share: f64) -> u32 {
    if share.is_nan() || share >= f64::from(MAX_TICKS) {
        return MAX_TICKS;
    }
    if share <= f64::from(MIN_TICKS) {
        return MIN_TICKS;
    }
    // The largest power of two at or below `share`, by repeated halving from the top of
    // the ladder. Downward only: a share is never rounded up into a settle it cannot pay for.
    let mut ticks = TOP_POWER_OF_TWO;
    while f64::from(ticks) > share {
        ticks /= 2;
    }
    ticks.clamp(MIN_TICKS, TOP_POWER_OF_TWO)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ladder the reports quote. Pinned because every crossover and settle number in
    /// `docs/measurements/` is read against it: a changed budget changes those.
    #[test]
    fn the_ladder_is_pinned_at_the_phase_sizes() {
        let cases = [
            (0, 0, MIN_TICKS),
            (220, 327, MAX_TICKS),
            (10_000, 15_497, 32),
            (100_000, 154_978, 8),
            (1_000_000, 1_549_780, MIN_TICKS),
        ];
        for (n, m, want) in cases {
            assert_eq!(tick_budget(n, m), want, "n={n} m={m}");
        }
    }

    #[test]
    fn more_nodes_or_more_edges_never_earns_a_longer_settle() {
        let base = tick_budget(2_000, 3_000);
        for n in [2_001, 4_000, 20_000, 200_000] {
            assert!(
                tick_budget(n, 3_000) <= base,
                "n={n} earned more than n=2000"
            );
        }
        for m in [3_001, 30_000, 300_000] {
            assert!(
                tick_budget(2_000, m) <= base,
                "m={m} earned more than m=3000"
            );
        }
    }

    #[test]
    fn the_budget_is_always_within_the_clamp_and_a_power_of_two_or_the_maximum() {
        for n in (0..2_000).step_by(7) {
            for m in [0, 1, 100, 10_000] {
                let ticks = tick_budget(n, m);
                assert!(
                    (MIN_TICKS..=MAX_TICKS).contains(&ticks),
                    "n={n} m={m}: {ticks}"
                );
                assert!(
                    ticks == MAX_TICKS || ticks.is_power_of_two(),
                    "n={n} m={m}: {ticks}"
                );
            }
        }
    }

    #[test]
    fn a_quantised_budget_is_the_largest_power_of_two_at_or_below_its_share() {
        assert_eq!(quantise(112.0), 112);
        assert_eq!(quantise(111.9), 64);
        assert_eq!(quantise(64.0), 64);
        assert_eq!(quantise(63.9), 32);
        assert_eq!(quantise(8.0), 8);
        assert_eq!(quantise(7.9), MIN_TICKS);
        assert_eq!(quantise(f64::INFINITY), MAX_TICKS);
        const { assert!(TOP_POWER_OF_TWO <= MAX_TICKS && TOP_POWER_OF_TWO * 2 > MAX_TICKS) };
    }

    /// The escape hatch, in both directions: an override is honoured verbatim, and
    /// `None` is exactly the size-derived budget.
    #[test]
    fn an_explicit_override_is_honoured_verbatim_including_zero() {
        for n in [0, 220, 100_000] {
            assert_eq!(tick_budget_with(n, 1_000, None), tick_budget(n, 1_000));
            assert_eq!(tick_budget_with(n, 1_000, Some(3)), 3);
            assert_eq!(tick_budget_with(n, 1_000, Some(0)), 0);
            assert_eq!(tick_budget_with(n, 1_000, Some(9_000)), 9_000);
        }
    }

    /// D8, the property the gate greps for, asserted here as well: the budget is a pure
    /// function, so calling it again with the same arguments cannot differ, and nothing
    /// in this module can read a clock (there is no `Instant`, no `SystemTime`, no I/O).
    #[test]
    fn the_budget_is_a_pure_function_of_its_two_arguments() {
        for n in [2, 220, 10_000] {
            for m in [0, 5, 15_497] {
                assert_eq!(tick_budget(n, m), tick_budget(n, m));
            }
        }
        assert_eq!(work_share(0, 0), 0.0);
        let want = TICK_WORK / (220.0 * libm::log2(221.0) + 327.0);
        assert!((work_share(220, 327) - want).abs() < 1e-9);
    }
}
