//! `layout.bipartite`: two vertical columns, one node set per column, edges straight
//! lines. Reference: networkx 3.6 `bipartite_layout`
//! (`networkx/drawing/layout.py:322`) at `align = "vertical"`, `aspect_ratio = 4/3`,
//! `scale = 1`: the first set on `x = 0`, the second on `x = 4/3`, each spread over
//! `y in [0, 1]` by `linspace` in set order, the whole cloud centred and rescaled.
//!
//! The two sets come from [`partition`], SciGraphs' rule (`_bipartite_parts`, with its
//! greedy maximum cut for a graph that does not two-colour). networkx would raise on a
//! disconnected graph, so it is given SciGraphs' sets through its `nodes=` argument.
//!
//! A graph that is not bipartite still gets a drawing: the cut splits it and some edges
//! then run inside a column (see `partition`'s Ponytail). It never panics.

mod partition;

use super::Geometry;
use super::adjacency::neighbours;
use super::coords::{point_geometry, rescale};
use crate::index::Topology;
use crate::stage::StageError;
use partition::partition;

/// The layout's capability id, which is also its hash-gate stage.
pub const ID: &str = "layout.bipartite";

/// networkx's default `aspect_ratio` over a unit height.
const WIDTH: f64 = 4.0 / 3.0;

/// The two node sets, in SciGraphs' order, shared with [`basic_3d::bipartite_3d`].
///
/// **Published rather than re-ported, because the two layouts need the same answer.** They
/// differ in placement and in nothing else — two columns against two rings — so a second
/// copy of `_bipartite_parts` could only ever drift from this one. See
/// `layout/basic_3d/bipartite_3d.rs` for the visiting order this preserves.
pub(in crate::layout) fn node_sets(topology: &Topology) -> (Vec<u32>, Vec<u32>) {
    partition(&neighbours(topology))
}

/// Runs the bipartite layout; never refuses.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let count = topology.node_count() as usize;
    let (first, second) = node_sets(topology);
    let mut x = vec![0.0; count];
    let mut y = vec![0.0; count];
    for (nodes, column) in [(&first, 0.0), (&second, WIDTH)] {
        let last = nodes.len().saturating_sub(1).max(1) as f64;
        for (slot, &node) in nodes.iter().enumerate() {
            x[node as usize] = column - WIDTH / 2.0;
            y[node as usize] = slot as f64 * (1.0 / last) - 0.5;
        }
    }
    rescale(&mut x, &mut y);
    Ok(point_geometry(&x, &y))
}

#[cfg(test)]
mod tests;
