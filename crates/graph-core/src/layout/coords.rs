//! Shared plumbing of the closed-form point layouts (`random`, `circular::ring`,
//! `bipartite`, `spiral`): networkx's `rescale_layout` and the `Point`/`Line` geometry
//! every one of them emits.
//!
//! Reference: networkx 3.6 `drawing/layout.py` `rescale_layout`. Coordinates are computed
//! in `f64` and cast to `f32` once, at the end; networkx keeps some intermediates in
//! `f32` (the ring's angles), so the differential against it is a tolerance, not bytes.

use super::Geometry;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// networkx `rescale_layout(pos, scale=1)`: subtract the centroid, then divide by the
/// largest absolute coordinate. A cloud that collapses to a point is left at the origin.
/// The centroid is summed in index order, so the result is fixed on every target.
pub(super) fn rescale(x: &mut [f64], y: &mut [f64]) {
    if x.is_empty() {
        return;
    }
    let count = x.len() as f64;
    let mean_x = x.iter().sum::<f64>() / count;
    let mean_y = y.iter().sum::<f64>() / count;
    let mut limit = 0.0_f64;
    for (px, py) in x.iter_mut().zip(y.iter_mut()) {
        *px -= mean_x;
        *py -= mean_y;
        limit = limit.max(px.abs()).max(py.abs());
    }
    if limit > 0.0 {
        for value in x.iter_mut().chain(y.iter_mut()) {
            *value /= limit;
        }
    }
}

/// `Point` nodes at `(x, y)` narrowed to `f32`, straight `Line` edges, no notes.
pub(super) fn point_geometry(x: &[f64], y: &[f64]) -> Geometry {
    let narrow = |column: &[f64]| column.iter().map(|&v| v as f32).collect::<Vec<f32>>();
    Geometry {
        nodes: NodeGeometry::Point {
            x: narrow(x),
            y: narrow(y),
        },
        edges: EdgeGeometry::Line,
        notes: Vec::new(),
    }
}

#[cfg(test)]
mod tests;

/// Test helpers the closed-form layouts' tests share.
#[cfg(test)]
pub(super) mod probe;
