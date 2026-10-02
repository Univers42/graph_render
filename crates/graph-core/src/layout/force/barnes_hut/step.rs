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
//! geometry — and writes only output `k`'s own velocity delta into `out`. Nothing
//! accumulates into another element, no term is reordered, and a node's walk visits its
//! subtree in the quadtree's own `visit` order regardless of which slice of the output
//! range the node fell into. So `partition(n, workers)` is a scheduling decision and
//! nothing else, and a tier is byte-identical rather than close (D3, D10).
//!
//! **Outputs in tree order.** The two tree walks number their outputs by the tree's own
//! point order (`Quadtree::order`): output `k` is node `order[k]`, so consecutive queries
//! sit in the same leaf or the next one and open the same cells. At 1M nodes the walk is
//! memory-bound, and a query order with no spatial locality pays a cache miss on most
//! cells it reads (`docs/measurements/perf-p1-baseline.md`). [`merge`] puts each delta
//! back on its own node, one addition per node, so the order moves no byte.
//!
//! | kernel | partitions by | the scatter it replaces |
//! |---|---|---|
//! | [`Pass`] (charge) | node | none — it was already a gather |
//! | [`CollidePass`] | node | none — the same gather shape |
//! | [`LinkForces`] | edge | none — each output is its own edge's force |
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
        let ctx = super::charge::Ctx::of(self.sim);
        let order = &self.sim.charge_tree.order()[range.start as usize..range.end as usize];
        // `zip`, not indexing: `out` is this range's own sub-column, so the two iterate
        // together by construction and a mismatch is a length error at the pairing rather
        // than a silent write past the end.
        for (slot, &i) in out.iter_mut().zip(order) {
            *slot = super::charge::node_delta(&ctx, i);
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
        let order = &self.sim.collide_tree.order()[range.start as usize..range.end as usize];
        for (slot, &i) in out.iter_mut().zip(order) {
            *slot = super::collide::node_delta(self.sim, i, reach);
        }
    }
}

/// Each simple edge's force, `(fx, fy)`, one output per edge: the square root and the
/// division run once per edge here, where reading them from both endpoints would run them
/// twice. Every output is its own edge's, so the pass partitions by edge.
pub struct LinkForces<'a> {
    sim: &'a Sim,
}

impl<'a> LinkForces<'a> {
    /// The pass over `sim`'s simple edges, which carry their frozen geometry already.
    pub fn of(sim: &'a Sim) -> LinkForces<'a> {
        LinkForces { sim }
    }
}

impl StepRange for LinkForces<'_> {
    type Out = (f64, f64);

    fn len(&self) -> u32 {
        self.sim.graph.lo.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [(f64, f64)]) {
        for (slot, e) in out.iter_mut().zip(range) {
            *slot = super::link::force(self.sim, e as usize);
        }
    }
}

/// The per-node link gather: node `i`'s own share of every edge incident to it, summed in
/// its CSR row's order, each edge's force read from [`LinkForces`]'s output.
///
/// The gather half of `docs/decisions/link-gather.md`: the row order is the simple-edge
/// index order the old single loop visited, so the per-node sum sees its terms in the
/// order it saw them when the whole column was accumulated by one thread.
pub struct LinkPass<'a> {
    sim: &'a Sim,
    forces: &'a [(f64, f64)],
}

impl<'a> LinkPass<'a> {
    /// The pass over `sim`, with `forces` the [`LinkForces`] output of the same state.
    pub fn of(sim: &'a Sim, forces: &'a [(f64, f64)]) -> LinkPass<'a> {
        LinkPass { sim, forces }
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
            let e = e as usize;
            let hi = self.sim.graph.hi[e] == node;
            let share = super::link::share(self.forces[e], self.sim.link_bias[e], hi);
            dvx += share.0;
            dvy += share.1;
        }
        (dvx, dvy)
    }
}

/// `v[i] += deltas[k]` for every output `k`, where node `i` is `order[k]` (or `k` itself
/// when `order` is `None`, the link pass's node order) — and, under the control, output
/// `k + 1`'s delta as well, the shape a wrong partition of the outputs would take.
///
/// Every gathered pass ends here, so a partition mistake shows up in one place for all
/// three. Each node receives exactly one addition, its own delta, which is why the order
/// the outputs are laid out in moves no byte and why the control has to *steal* a
/// neighbour's term to move anything.
pub(in crate::layout::force) fn merge(
    v: (&mut [f64], &mut [f64]),
    order: Option<&[u32]>,
    deltas: &[(f64, f64)],
    split: bool,
) {
    for (k, (dvx, dvy)) in deltas.iter().enumerate() {
        let stolen = if split {
            deltas.get(k + 1).copied().unwrap_or((0.0, 0.0))
        } else {
            (0.0, 0.0)
        };
        let i = order.map_or(k, |order| order[k] as usize);
        v.0[i] += dvx + stolen.0;
        v.1[i] += dvy + stolen.1;
    }
}
