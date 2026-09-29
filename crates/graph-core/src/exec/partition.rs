//! Where the slices are, what a slice is allowed to touch, and who runs it.
//! `partition(n, workers)` is a **pure function of its two arguments** (`prompt.md` §6 D3,
//! `phase-11-compute-tiers.md` step 2), so the same `(n, workers)` gives the same ranges on
//! every run, on every target, in every tier — nothing about the host, its core count or
//! the clock enters here.
//!
//! The rule is stated rather than computed, because a partition is a convention: `k =
//! min(n, workers)` slices, the first `n % k` of them one element longer than the rest, in
//! ascending output order. The remainder goes to the **front**, not to the busiest slice,
//! so the rule stays one integer division and one comparison with no search. Odd worker
//! counts (the gate runs 1, 2, 3, 4 and 7) leave uneven slices on purpose: an even split
//! hides a boundary bug, and an uneven one shows it.
//!
//! [`Runner`] is the other half of the contract and the reason graph-core stays pure: a
//! runner is *supplied* — [`Serial`] here, `std::thread::scope` in graph-cli, Web Workers
//! in the SDK — and graph-core never spawns one itself, so it still builds for
//! `wasm32-unknown-unknown` with no dependency beyond its own (`compute-tiers.md` rule 2).
//!
//! No marker is owed on this file. Everything here is exact.

use std::ops::Range;

/// One per-step kernel that can be cut into ranges (`phase-11-compute-tiers.md` step 2).
///
/// The contract is the whole reason a tier can exist without changing a byte: element `i`
/// reads only **start-of-step** state, writes only `out[i]`, and accumulates its own terms
/// in the kernel's own fixed order. Nothing scatters into another element's accumulator, so
/// any division of `0..n` is a legal plan (D10) and no division splits one element's sum
/// (D3).
///
/// `&self` and no `&mut`: the workers share the start-of-step state and each owns a
/// disjoint span of `out`, which is exactly what `&self` plus disjoint ranges express.
/// The `Sync` supertrait is that sharing stated to the compiler: a kernel that had to be
/// mutated mid-step could not be handed to another thread, and refusing it here means the
/// refusal is a type error at the kernel, not a data race in an executor.
///
/// `Out` is an associated type rather than a fixed `f32` because the motor is not all
/// `f32`: the bundling points are (`post::fdeb`), the force simulation is `f64`
/// (`layout::force`). One contract, both widths — a second trait for `f64` would be the
/// same rule written twice, and the two copies would drift.
pub trait StepRange: Sync {
    /// The value one output is: a scalar column, or a pair of columns written together.
    type Out: Copy + Default + Send;

    /// How many outputs the whole kernel writes — the `n` [`partition`] divides.
    fn len(&self) -> u32;

    /// Whether the kernel writes nothing, so every range is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Computes the elements of `range` into `out`, and nothing else.
    ///
    /// `out.len()` is `range.len()`: it is **that range's own sub-column**, so element `i`
    /// of `range` is written at `out[i - range.start]`. A runner that hands a worker a
    /// fresh buffer per range therefore needs no shared borrow and no `unsafe`, and one
    /// that hands out disjoint spans of a single column is doing the same thing.
    fn step_range(&self, range: Range<u32>, out: &mut [Self::Out]);
}

/// Runs a [`StepRange`] kernel over some division of its outputs.
///
/// **Supplied, never chosen here** (`compute-tiers.md` rule 2). graph-core holds
/// [`Serial`]; the executors live with their hosts — `std::thread::scope` in graph-cli,
/// Web Workers in the SDK — and each one is a different answer to "how do these ranges run
/// at once", not a different answer to "what do they compute". That is the whole design:
/// the bytes are the kernel's, and the runner only decides who writes them and when.
///
/// The method is generic rather than `dyn`-dispatched on purpose, so a runner monomorphises
/// per output type and the call is a direct one.
pub trait Runner {
    /// Computes every one of `kernel`'s outputs, appending them to `out` in ascending
    /// order. `out` is cleared first, so the caller reuses one buffer and a steady-state
    /// step allocates nothing.
    ///
    /// `workers` is the host's own count, and the plan is [`partition`]'s whatever the
    /// runner does with it: a runner that runs every range on one thread is still correct,
    /// which is what makes [`Serial`] a reference rather than a special case.
    ///
    /// A `workers` below 1 is **one**, not zero. [`partition`] says `partition(n, 0)` is no
    /// slices, which is the right answer to "where are the slices"; it is the wrong answer
    /// to "compute the outputs", and a runner that forwarded it would hand back a column of
    /// zeros for every element a zero-worker host asked for. Zero workers is what a host
    /// with no threads reports (`Caps::workers == 0`), and its layout is still a layout.
    fn run<O: StepRange>(&self, kernel: &O, workers: u32, out: &mut Vec<O::Out>);
}

/// The one-thread runner: every range, in order, on the calling thread.
///
/// The reference every other runner is compared against, and a real tier rather than a
/// stand-in — [`Runner::run`] with `workers = 1` is the tier 1a path, and it runs the same
/// `partition` a seven-worker host runs, one range at a time. That is deliberate: a
/// reference that took a different code path would make "the threads changed nothing" a
/// claim about two different programs.
#[derive(Debug, Clone, Copy, Default)]
pub struct Serial;

impl Runner for Serial {
    fn run<O: StepRange>(&self, kernel: &O, workers: u32, out: &mut Vec<O::Out>) {
        out.clear();
        // The buffer is the kernel's own length before the first range runs: `step_range`
        // writes at range-relative indices, so a kernel given a short buffer would index
        // past its end rather than write somewhere wrong.
        out.resize(kernel.len() as usize, O::Out::default());
        for range in partition(kernel.len(), workers.max(1)) {
            let (start, end) = (range.start as usize, range.end as usize);
            kernel.step_range(range, &mut out[start..end]);
        }
    }
}

/// The output ranges `workers` workers take over `n` outputs, ascending and contiguous.
///
/// Empty when `n` is 0 or `workers` is 0: there is no work to divide, and a worker given
/// an empty range would be a worker that computes nothing and reports success.
pub fn partition(n: u32, workers: u32) -> Vec<Range<u32>> {
    let k = n.min(workers);
    if k == 0 {
        return Vec::new();
    }
    let base = n / k;
    let longer = n % k;
    let mut out = Vec::with_capacity(k as usize);
    let mut at = 0;
    for i in 0..k {
        let len = base + u32::from(i < longer);
        out.push(at..at + len);
        at += len;
    }
    out
}

#[cfg(test)]
mod tests;
