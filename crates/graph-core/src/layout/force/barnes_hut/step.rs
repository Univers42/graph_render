//! The many-body pass as a range kernel: the phase-11 [`StepRange`], and the first real
//! kernel in the motor.
//!
//! **Why the charge pass and not the whole layout.** A Barnes-Hut run is `TICKS` ticks,
//! and tick `t + 1` reads the positions tick `t` wrote, so the tick loop cannot be
//! partitioned at all — it is one thread's straight-line code above the kernel, and the
//! barrier lands at the end of the pass, once per tick. What *is* partitionable is the
//! per-node gather inside each pass, and that is what this file holds: one kernel per
//! gathered pass, each with the same shape.
//!
//! **What makes it legal.** A pass reads only start-of-step state — the built quadtrees,
//! the aggregate masses and centres, the positions, `alpha`, the frozen per-edge link
//! geometry — and writes only element `i`'s own velocity delta into `out`. Nothing
//! accumulates into another element, no term is reordered, and a node's walk visits its
//! subtree in the quadtree's own `visit` order regardless of which slice of the node
//! range the node fell into. So `partition(n, workers)` is a scheduling decision and
//! nothing else, and a tier is byte-identical rather than close (D3, D10).
//!
//! | kernel | partitions by | the scatter it replaces |
//! |---|---|---|
//! | [`Pass`] (charge) | node | none — it was already a gather |
//! | [`CollidePass`] | node | none — the same gather shape |
//! | [`LinkPass`] | node | one edge writing both endpoints — see `link-gather.md` |
//!
//! All three share one output type — a per-node `(dvx, dvy)` — so they share the tick's
//! single scratch buffer and the one merge each ends with.
//!
//! The tree builds and the aggregate stay **single-threaded and ahead of the ranges**,
//! which is `phase-11-compute-tiers.md` step 2's own prescription: the tree is a shared,
//! start-of-step read, and the aggregate is a bottom-up pass whose every arena node reads
//! its children's already-final values, so it could be partitioned by arena node but buys
//! nothing next to the queries it feeds. `docs/measurements/phase11-threads.md` is where a
//! measurement that says otherwise would land.
//!
//! **Not the only kernels, and not claimed to be.** `post::fdeb`'s iteration partitions by
//! point and gets its own kernel when it gets a tier; none of them reorders a sum to get
//! one.

use super::sim::Sim;
use crate::exec::StepRange;
use std::ops::Range;

/// The per-node many-body gather over an already-built and already-aggregated [`Sim`].
///
/// Borrows the state read-only, which is the whole contract: `&self`, no `&mut`, so
/// several workers can hold one of these at once and each writes only its own slots. The
/// `Sync` the [`StepRange`] supertrait asks for is what the compiler checks that against.
pub struct Pass<'a> {
    sim: &'a Sim,
}

impl<'a> Pass<'a> {
    /// The pass over `sim`, which the caller has already built the tree and aggregated.
    pub fn of(sim: &'a Sim) -> Pass<'a> {
        Pass { sim }
    }
}

impl StepRange for Pass<'_> {
    /// `(dvx, dvy)`: one node's own velocity delta, both columns in one output.
    type Out = (f64, f64);

    fn len(&self) -> u32 {
        self.sim.x.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [(f64, f64)]) {
        // One stack for the whole range, reused across its nodes: the walk is iterative
        // precisely so this buffer can be borrowed rather than owned, which is what lets
        // several workers hold the same `&Sim` at once. Allocated per *range*, not per
        // node, so the threaded tier's allocation count is `workers` per pass.
        let mut stack = Vec::new();
        // `zip`, not indexing: `out` is this range's own sub-column, so the two iterate
        // together by construction and a mismatch is a length error at the pairing rather
        // than a silent write past the end.
        for (slot, i) in out.iter_mut().zip(range) {
            *slot = self.sim.node_delta(i, &mut stack);
        }
    }
}

/// The per-node collide gather over an already-projected, already-built [`Sim`].
pub struct CollidePass<'a> {
    sim: &'a Sim,
}

impl<'a> CollidePass<'a> {
    /// The pass over `sim`, which the caller has already projected and built.
    pub fn of(sim: &'a Sim) -> CollidePass<'a> {
        CollidePass { sim }
    }
}

impl StepRange for CollidePass<'_> {
    type Out = (f64, f64);

    fn len(&self) -> u32 {
        self.sim.px.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [(f64, f64)]) {
        let reach = super::collide::reach_squared(self.sim);
        let mut stack = Vec::new();
        for (slot, i) in out.iter_mut().zip(range) {
            *slot = super::collide::node_delta(self.sim, i, reach, &mut stack);
        }
    }
}

/// The per-node link gather: node `i`'s own share of every edge incident to it, summed in
/// its CSR row's order.
///
/// The gather half of `docs/decisions/link-gather.md`: the row order is the simple-edge
/// index order the old single loop visited, so the per-node sum sees its terms in the
/// order it saw them when the whole column was accumulated by one thread.
pub struct LinkPass<'a> {
    sim: &'a Sim,
}

impl<'a> LinkPass<'a> {
    /// The pass over `sim`, whose edges carry their frozen geometry already.
    pub fn of(sim: &'a Sim) -> LinkPass<'a> {
        LinkPass { sim }
    }
}

impl StepRange for LinkPass<'_> {
    type Out = (f64, f64);

    fn len(&self) -> u32 {
        self.sim.x.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [(f64, f64)]) {
        for (slot, i) in out.iter_mut().zip(range) {
            *slot = self.node_share(i);
        }
    }
}

impl LinkPass<'_> {
    /// Node `i`'s own share of every simple edge incident to it, in its row's order.
    fn node_share(&self, node: u32) -> (f64, f64) {
        let (mut dvx, mut dvy) = (0.0, 0.0);
        for &e in self.sim.graph.rows.row(node) {
            let ((lox, loy), (hix, hiy)) = super::link::halves(self.sim, e as usize);
            let share = if self.sim.graph.hi[e as usize] == node {
                (hix, hiy)
            } else {
                (lox, loy)
            };
            dvx += share.0;
            dvy += share.1;
        }
        (dvx, dvy)
    }
}
