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
use super::coords::{point_geometry, point_geometry_with, rescale};
use crate::index::Topology;
use crate::stage::StageError;
use partition::partition;

/// The layout's capability id, which is also its hash-gate stage.
pub const ID: &str = "layout.bipartite";

/// The 3D arm's capability id (`docs/measurements/p12-t4a.md`).
pub const ID_3D: &str = "layout.bipartite.3d";

/// networkx's default `aspect_ratio` over a unit height.
const WIDTH: f64 = 4.0 / 3.0;

/// SciGraphs `_bipartite_layout_3d`'s ring radius and plane offset
/// (`hierarchical.py:234-239`), at its default `scale = 1`.
const RADIUS_3D: f64 = 0.6;
const PLANE_3D: f64 = 0.5;

/// Runs the bipartite layout; never refuses.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let count = topology.node_count() as usize;
    let (first, second) = partition(&neighbours(topology));
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

/// `layout.bipartite.3d`: the two node sets on parallel planes, one ring each.
///
/// **The partition is shared, the placement is not** (`docs/measurements/p12-t4a.md`):
/// [`partition`] is SciGraphs' own `_bipartite_parts` with its greedy maximum cut, and both
/// arms call it, so the set membership is one implementation. The 2D arm draws two
/// vertical columns (networkx `bipartite_layout`); SciGraphs' `_bipartite_layout_3d`
/// (`hierarchical.py:213-242`) draws a ring of radius [`RADIUS_3D`] on each of the planes
/// `z = ∓PLANE_3D`, which is a different placement of the same two sets rather than the
/// same placement in three dimensions. That is why this is not a `dims` parameter over
/// [`run`].
pub fn run_3d(topology: &Topology) -> Result<Geometry, StageError> {
    let count = topology.node_count() as usize;
    let (first, second) = partition(&neighbours(topology));
    let (mut x, mut y, mut z) = (vec![0.0; count], vec![0.0; count], vec![0.0; count]);
    for (nodes, plane) in [(&first, -PLANE_3D), (&second, PLANE_3D)] {
        // `max(1, count)`: an empty set divides by nothing, and the reference guards it
        // the same way (`hierarchical.py:237`).
        let divisor = nodes.len().max(1) as f64;
        for (slot, &node) in nodes.iter().enumerate() {
            let angle = slot as f64 / divisor * 2.0 * std::f64::consts::PI;
            x[node as usize] = RADIUS_3D * libm::cos(angle);
            y[node as usize] = RADIUS_3D * libm::sin(angle);
            z[node as usize] = plane;
        }
    }
    // No rescale: the reference places at its default `scale = 1` and does not
    // `_rescale_positions` (see the 3D spectral arm for the same decision).
    Ok(point_geometry_with(&x, &y, Some(&z)))
}

#[cfg(test)]
mod tests;
