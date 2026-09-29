//! Builds the [`Geometry`](super::super::Geometry) this pipeline produces: `Point` nodes
//! at [`Coords`] and layer Y, `Polyline` edges through each edge's dummy chain.
//!
//! Convention (exact, not a heuristic, like `grid.rs`'s own stated conventions): X is the
//! priority method's own units (`coords.rs`'s `GAP = 1.0`), not centred; Y is
//! `layer × LAYER_SPACING`, growing with the layer, which way is down is the renderer's
//! to decide. A reversed edge's dummy chain was built tail-to-head in acyclic order
//! (`hierarchical.py`'s convention); its interior points are walked back to front so the
//! polyline still runs from the edge's real source to its real target.

use super::acyclic::Acyclic;
use super::coords::Coords;
use super::layering::{Layering, Route};
use graph_contract::geometry::Paths;

/// The spacing between adjacent layers on Y. Matches `coords.rs`'s `GAP` on X, so a
/// dummy's step between layers is the same visual size as a real vertex's step within one.
pub(crate) const LAYER_SPACING: f32 = 1.0;

/// `layering`, `coords` and `acyclic`, bundled so [`edge_paths`] and its helper take one
/// parameter instead of three (≤4 per house style).
pub(crate) struct Routing<'a> {
    pub(crate) layering: &'a Layering,
    pub(crate) coords: &'a Coords,
    pub(crate) acyclic: &'a Acyclic,
    /// Y per layer step; [`LAYER_SPACING`] unless a caller asks for another.
    pub(crate) spacing: f32,
}

/// Every real node's `(x, y)`, in node order.
pub(crate) fn node_positions(routing: &Routing, node_count: u32) -> (Vec<f32>, Vec<f32>) {
    let (mut x, mut y) = (Vec::with_capacity(node_count as usize), Vec::new());
    for v in 0..node_count {
        x.push(routing.coords.0[v as usize] as f32);
        y.push(routing.layering.layer_of[v as usize] as f32 * routing.spacing);
    }
    (x, y)
}

/// Every edge's interior points, CSR-shaped: a `Loop`, `Direct` or `Straight` route has
/// none, a `Chain` has one point per dummy.
pub(crate) fn edge_paths(routing: &Routing) -> Paths {
    let m = routing.layering.route.len();
    let mut offsets = Vec::with_capacity(m + 1);
    let mut pts = Vec::new();
    offsets.push(0);
    for e in 0..m as u32 {
        push_route(&mut pts, routing, e);
        offsets.push((pts.len() / 2) as u32);
    }
    Paths { offsets, pts }
}

/// Appends edge `e`'s interior points to `pts`, source to target.
fn push_route(pts: &mut Vec<f32>, routing: &Routing, e: u32) {
    let Route::Chain { first, count } = routing.layering.route[e as usize] else {
        return;
    };
    let reversed = routing.acyclic.reversed[e as usize];
    for step in 0..count {
        let d = if reversed {
            first + count - 1 - step
        } else {
            first + step
        };
        pts.push(routing.coords.0[d as usize] as f32);
        pts.push(routing.layering.layer_of[d as usize] as f32 * routing.spacing);
    }
}

#[cfg(test)]
mod tests;
