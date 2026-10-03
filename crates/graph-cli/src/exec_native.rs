//! The native thread executor, **outside** graph-core (`compute-tiers.md` rule 2): a
//! [`graph_core::exec::Runner`] over `graph_core::exec::partition`'s fixed slices, with one
//! barrier per step and double-buffered state.
//!
//! What makes it legal rather than merely parallel: the kernel it drives is a
//! [`StepRange`], so each worker reads only start-of-step state and writes only its own
//! span of `out`. No two workers touch one element's accumulator, no sum is split, and no
//! term is reordered — so the work count changes and the output bytes do not. That claim
//! is not this module's opinion: the tests below compare the threaded bytes with the
//! [`Serial`] ones over odd worker counts, and `partition.rs`'s own negative control shows
//! the comparison catches a split sum.
//!
//! **Measured once, on a loaded host, and not a promotion.** One interleaved pair of Barnes-Hut
//! bench runs at 100 000 nodes put this executor, with the workers writing straight into the
//! caller's column, at 5 650 ms against 6 547 ms for the per-worker buffers it replaced
//! (7 workers, 112 ticks, −13.7 %) — and the unchanged scalar arm of the same pair drifted
//! 3.4 % on the same host, which is the number a reader should hold the rest of the table to.
//! That is evidence the two copies of the column were worth removing, not a sweep carrying the
//! losing sizes a threshold needs (`docs/decisions/tier-thresholds.md:46-58`), so
//! `Thresholds::MEASURED` still promotes nothing. Commands, host load, and the 1M pair the
//! scalar control leaves inconclusive: `docs/measurements/perf-p3-split.md`.

use graph_core::exec::{Runner, Serial, StepRange, partition};
#[cfg(test)]
use std::ops::Range;

/// Runs a [`StepRange`] kernel on the `workers` `std::thread`s its `run` call names.
///
/// A unit struct on purpose: the worker count arrives with the call, because it is a
/// property of *this step* — the gate runs 1, 2, 3, 4 and 7, the SDK runs whatever the
/// tier selected — and a runner that carried its own count would be a second place for
/// the number to live and disagree with itself.
///
/// One worker is [`graph_core::exec::Serial`]'s work, not a one-thread `scope`: below two
/// there is nothing to overlap, and the single-threaded path is the one every
/// byte-identity claim is stated against, so a one-worker `Threads` that took a different
/// route would be the arm most likely to disagree.
#[derive(Debug, Clone, Copy, Default)]
pub struct Threads;

impl Runner for Threads {
    fn run<O: StepRange>(&self, kernel: &O, workers: u32, out: &mut Vec<O::Out>) {
        let workers = workers.max(1);
        if workers < 2 {
            Serial.run(kernel, 1, out);
            return;
        }
        // Sized once, before the call: the kernel writes `out[range]` at range-relative
        // indices, so the buffer has to be the kernel's own length when it starts. Only a
        // grown tail is written here; each worker clears its own span.
        out.resize(kernel.len() as usize, O::Out::default());
        // The disjoint borrow is `partition`'s contract, not a hope: its ranges are
        // ascending, contiguous and cover `0..n` exactly, so peeling `range.len()` off
        // the tail each time hands worker `i` a span starting exactly where its range
        // does, and the last peel is empty. One column, one allocation, no assembly.
        std::thread::scope(|scope| {
            let mut rest = &mut out[..];
            for range in partition(kernel.len(), workers) {
                let (span, tail) = std::mem::take(&mut rest).split_at_mut(range.len());
                rest = tail;
                scope.spawn(move || {
                    span.fill(O::Out::default());
                    kernel.step_range(range, span);
                });
            }
            // The plan's lengths add up to the column's length, so nothing is left over.
            // A `partition` that ever stopped covering `0..n` would fail this, not the
            // equality tests: the leftovers would be the previous pass's values.
            debug_assert!(rest.is_empty());
        });
    }
}

/// The ranges [`Threads`] would use, exposed so a caller (or a test) can read the plan
/// without running it.
#[cfg(test)]
pub fn plan(n: u32, workers: u32) -> Vec<Range<u32>> {
    partition(n, workers)
}

#[cfg(test)]
mod tests;
