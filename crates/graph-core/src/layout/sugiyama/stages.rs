//! The pipeline's own stage outputs, as one test-only snapshot: the acyclic arcs, the
//! ordering graph's layer index, each layer's order, the crossing count and X. This is the
//! motor half of the SciGraphs stage diff; the reference half is a dump of
//! `hierarchical.py`'s own private functions, and nothing but the ignored test in
//! [`dump`] reads any of this.
//!
//! **One snapshot per fixture, written to `target/sugiyama-stages.json`.** The diff is a
//! human read — "first stage that differs, first node that differs in it" — so the numbers
//! are dumped whole rather than compared here: a comparison inside graph-core could only
//! report that *something* moved, not which stage.

use super::acyclic::{Acyclic, Arcs};
use super::coords::Coords;
use super::layering::{DUMMY_BUDGET, Layering, assign_layers};
use super::ordering::Ordering;
use crate::index::Topology;

/// One run of the pipeline, stage by stage, dense-indexed like [`Topology`].
pub(crate) struct Stages {
    /// The ordering graph's arcs: every distinct non-loop `(tail, head)` pair, ascending,
    /// which is what the reference's `_acyclic_arcs` returns and what the pipeline is built
    /// over.
    pub(crate) arcs: Vec<(u32, u32)>,
    /// How many **edges** the cycle breaker reversed. Not a stage output the reference has —
    /// it draws no edges — but the count is what tells a reader whether an arc was oriented
    /// against its edge's own spelling, which is where a stage difference usually starts.
    pub(crate) reversed: u32,
    /// Real nodes then dummies, as the reference's `layer_of`.
    pub(crate) layer_of: Vec<u32>,
    /// Dummy vertices the long spans needed.
    pub(crate) num_dummies: u32,
    /// Each layer's left-to-right vertices.
    pub(crate) order: Vec<Vec<u32>>,
    /// The crossing count the sweep settled on.
    pub(crate) crossings: u64,
    /// Every vertex's X, dummies included, in the priority method's own units.
    pub(crate) x: Vec<f64>,
}

/// The pipeline over `topology`, captured after every stage. The same six calls
/// [`layered`](super::layered) and [`Coords::build`] make, kept here rather than reached
/// through them because two of the intermediate values they drop are the ones the diff is
/// about.
pub(crate) fn stages(topology: &Topology) -> Stages {
    let acyclic = Acyclic::of(topology);
    let list = Arcs::new(topology, &acyclic).grouped();
    let layer = assign_layers(&list);
    let layering = Layering::build(&list, &layer, DUMMY_BUDGET);
    let num_layers = layering.layer_of.iter().copied().max().map_or(0, |m| m + 1);
    let ordering = Ordering::build(&layering, num_layers);
    let coords = Coords::build(&ordering, &layering, topology.node_count());
    Stages {
        arcs: list.pairs(),
        reversed: acyclic.reversed.iter().filter(|r| **r).count() as u32,
        num_dummies: (layering.layer_of.len() - topology.node_count() as usize) as u32,
        layer_of: layering.layer_of,
        order: ordering.layers,
        crossings: ordering.crossings,
        x: coords.0,
    }
}

#[cfg(test)]
mod dump;
