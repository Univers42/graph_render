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
//! **Not yet measured.** The executor exists and is proven equal; whether it is *faster*
//! at a given N is a measurement this branch has not taken
//! (`docs/decisions/tier-thresholds.md` and `docs/measurements/phase11-threads.md` are
//! where that lands), which is why `Thresholds::MEASURED` promotes nothing yet.

use graph_core::exec::{Runner, StepRange, partition};
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
        out.clear();
        if workers.max(1) < 2 {
            // No resize before the call: the kernel writes `out[range]` at range-relative
            // indices, so the buffer has to be the kernel's own length when it starts.
            out.resize(kernel.len() as usize, O::Out::default());
            kernel.step_range(0..kernel.len(), out);
            return;
        }
        // Each worker fills its own buffer rather than a disjoint borrow of the caller's
        // column: the ranges are ascending and contiguous (`partition`'s contract), so
        // laying the buffers end to end after the join reconstructs the column exactly,
        // with no `unsafe` and no overlapping borrow. The barrier is the scope's own join,
        // before the assembly — one per step, which is the whole synchronisation the
        // double-buffered state asks for.
        let parts: Vec<Vec<O::Out>> = std::thread::scope(|scope| {
            let handles: Vec<_> = partition(kernel.len(), workers)
                .into_iter()
                .map(|range| {
                    scope.spawn(move || {
                        let mut span = vec![O::Out::default(); range.len()];
                        kernel.step_range(range, &mut span);
                        span
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("a worker did not panic"))
                .collect()
        });
        out.extend_from_slice(&parts.concat());
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
