//! The pass loop: three passes over the ranks, and the best order any of them found.
//!
//! **Three passes, not one.** The first two rebuild the order from scratch — one downwards
//! from the sources, one upwards from the sinks — and are allowed only a few sweeps each,
//! because a full sweep is expensive and a source-ordered graph is usually already good.
//! The third pass is where the work happens: it keeps the best order found so far and runs
//! up to 24 median/transpose sweeps against it.
//!
//! **The order that survives is the best one seen, not the last one.** Every sweep is
//! counted, and the best count is remembered with the order that produced it; a sweep that
//! makes things worse is thrown away by restoring the remembered order. So the pass cannot
//! return an order worse than the one it started the pass with, however many bad sweeps it
//! takes to get there.
//!
//! **When to stop.** A sweep that does not improve on the best count counts as a try, and
//! eight tries in a row ends the pass: that is the plateau. An improvement too small to be
//! worth a try — less than half a percent — resets the count instead, because a pass that is
//! still making progress has not plateaued. A count of zero ends everything at once, since
//! nothing can beat it.
//!
//! **The last transverse pass.** After the loops the best order is restored if the last sweep
//! left something worse, and then one final transverse pass runs — without the reverse
//! rule, so it only ever takes strict improvements. The count the pass returns is the one
//! *after* that pass, not the best count the loop recorded, so the number a caller compares
//! is the number the order in hand actually has.
//!
//! Determinism: the loop bounds are constants, the convergence test is one floating-point
//! comparison in the reference's own arithmetic, and nothing here reads a clock, a hash
//! order or a random number (`prompt.md` §6 D1-D10).

use super::super::fast::Fast;
use super::median::Sweep;
use super::ranks::Ranks;

/// The last pass index: 0 and 1 rebuild the order, 2 is the long one.
const LAST_PASS: i32 = 2;
/// How many sweeps a rebuilding pass gets, `min(4, MAX_SWEEPS)`.
const FIRST_ROUNDS: i32 = 4;
/// How many sweeps the long pass gets.
const MAX_SWEEPS: i32 = 24;
/// How many sweeps in a row may fail to improve before the pass gives up.
const MIN_QUIT: i32 = 8;
/// The improvement, as a fraction of the best count, that resets the give-up counter.
const CONVERGENCE: f64 = 0.995;

/// The two totals the loop carries from one sweep to the next: what the order in hand
/// costs, and the best any order has cost.
struct Counts {
    /// What the order in hand costs, as last counted.
    cur: i64,
    /// The best count seen, with the order that produced it remembered.
    best: i64,
}

/// Order the component, from pass `startpass` onwards, and report how many crossings the
/// order left behind has.
pub fn mincross(g: &mut Fast, ranks: &mut Ranks, startpass: i32) -> i64 {
    let mut counts = Counts::start(g, ranks, startpass);
    for pass in startpass..=LAST_PASS {
        counts = start_pass(g, ranks, pass, counts);
        counts = sweep(g, ranks, rounds_for(pass), counts);
        if counts.cur == 0 {
            break;
        }
    }
    if counts.cur > counts.best {
        ranks.restore_best(g);
    }
    if counts.best > 0 {
        super::transpose::transpose(g, ranks, false);
        counts.best = super::crossings::ncross(g, ranks);
    }
    counts.best
}

/// How many sweeps this pass is allowed.
fn rounds_for(pass: i32) -> i32 {
    if pass <= 1 { FIRST_ROUNDS.min(MAX_SWEEPS) } else { MAX_SWEEPS }
}

impl Counts {
    /// Nothing has been counted yet on a first run, so both totals start above any count a
    /// real order could reach and the first count is always an improvement on them. A run
    /// that starts part-way through the passes has an order in hand already, and that is
    /// what it must beat.
    fn start(g: &mut Fast, ranks: &mut Ranks, startpass: i32) -> Self {
        if startpass > 1 {
            let count = super::crossings::ncross(g, ranks);
            ranks.save_best(g);
            Self { cur: count, best: count }
        } else {
            Self { cur: i64::MAX, best: i64::MAX }
        }
    }
}

/// Get a pass ready: the first two rebuild the order from the sources or the sinks and
/// count what that cost; the third goes back to the best order found so far.
fn start_pass(g: &mut Fast, ranks: &mut Ranks, pass: i32, mut counts: Counts) -> Counts {
    if pass > 1 {
        if counts.cur > counts.best {
            ranks.restore_best(g);
        }
        counts.cur = counts.best;
        return counts;
    }
    super::build::build_ranks(g, ranks, pass as usize);
    counts.cur = super::crossings::ncross(g, ranks);
    if counts.cur <= counts.best {
        ranks.save_best(g);
        counts.best = counts.cur;
    }
    counts
}

/// Up to `rounds` median/transpose sweeps, stopping on a plateau or on a perfect order.
fn sweep(g: &mut Fast, ranks: &mut Ranks, rounds: i32, mut counts: Counts) -> Counts {
    let mut tries = 0;
    for iteration in 0..rounds {
        if tries >= MIN_QUIT || counts.cur == 0 {
            break;
        }
        tries += 1;
        one_step(g, ranks, iteration);
        counts.cur = super::crossings::ncross(g, ranks);
        if counts.cur <= counts.best {
            ranks.save_best(g);
            if (counts.cur as f64) < CONVERGENCE * counts.best as f64 {
                tries = 0;
            }
            counts.best = counts.cur;
        }
    }
    counts
}

/// One median/transpose sweep: every rank is moved towards the median of its neighbours,
/// downwards or upwards, and then every adjacent pair in every rank is reconsidered.
///
/// The direction alternates with the sweep index, and the pair sweep runs the other way
/// round from the median pass — on the odd sweeps, so that the two never settle into the
/// same arrangement and undo each other.
fn one_step(g: &mut Fast, ranks: &mut Ranks, iteration: i32) {
    let reverse = iteration % 4 < 2;
    let step = if iteration % 2 == 0 { 1 } else { -1 };
    let max = i32::try_from(ranks.max()).expect("a rank index fits i32");
    // A downward sweep starts one rank below the top, because the top rank's order was just
    // decided by the walk; an upward sweep ends one rank above the bottom, because the
    // bottom rank's order is the one the walk decided and is not its own to change.
    let visited: Vec<i32> = if step > 0 { (1..=max).collect() } else { (0..max).rev().collect() };
    for r in visited {
        let fixed = super::median::medians(g, ranks, r as usize, (r - step) as usize);
        super::median::reorder(g, ranks, r as usize, &Sweep { reverse, fixed });
    }
    super::transpose::transpose(g, ranks, !reverse);
}