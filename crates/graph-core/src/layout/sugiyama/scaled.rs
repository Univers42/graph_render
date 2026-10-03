//! SciGraphs' per-axis normalisation of this pipeline's own X and layer index
//! (`hierarchical.py:679-685`), as a second entry point over the same six stages.
//!
//! **Why this is not a transform in the conformance arm.** `layout.dag.sugiyama`
//! ([`run`](super::run)) is the registered layout, and it keeps the priority method's own
//! units and `LAYER_SPACING`: X is uncentred in `GAP` units, Y is `layer × LAYER_SPACING`.
//! The reference instead maps X onto `[-scale, scale]` and Y onto `[-scale, scale]` over the
//! layer range — and its `lo`/`hi` are the extremes over **every** vertex of the ordering
//! graph, dummies included, because `_assign_x` returns one X per vertex whether real or
//! dummy. So the normalisation is a function of the dummy X, which `Geometry` does not carry
//! and which no amount of post-processing on the real nodes' coordinates can recover. The
//! formula therefore lives here, beside the stages that produce its inputs, and
//! `conformance/motor.rs` calls this entry point for the `SUGIYAMA` row.

use super::Geometry;
use super::acyclic::Acyclic;
use super::coords::Coords;
use super::layered;
use super::layering::{Layering, Route};
use crate::index::Topology;
use crate::stage::StageError;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry, Paths};

/// The four numbers every coordinate of one drawing needs, bundled so a helper takes one
/// parameter instead of four (house style, and the same reason
/// [`Routing`](super::routing::routing::Routing) bundles its three).
struct Frame {
    lo: f64,
    width: f64,
    max_layer: u32,
    scale: f64,
}

impl Frame {
    /// `(lo, width)` from `x`: the reference's `min`, and `(hi - lo) or 1.0`
    /// (`hierarchical.py:679-681`) — a Python `or`, so a zero span is replaced by one rather
    /// than divided by.
    fn of(x: &[f64], max_layer: u32, scale: f32) -> Self {
        let lo = x.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let (lo, hi) = if lo.is_finite() && hi.is_finite() {
            (lo, hi)
        } else {
            (0.0, 0.0) // `x` empty: the reference's `if x else 0.0` on both bounds.
        };
        let span = hi - lo;
        Self {
            lo,
            width: if span == 0.0 { 1.0 } else { span },
            max_layer,
            scale: f64::from(scale),
        }
    }

    /// `((x - lo) / width * 2 - 1) * scale`, the reference's X column.
    fn x(&self, x: f64) -> f32 {
        (((x - self.lo) / self.width * 2.0 - 1.0) * self.scale) as f32
    }

    /// `((layer / max_layer) * 2 - 1) * scale`, or `0.0` on a single-layer drawing —
    /// the reference's conditional covers the whole expression, not just the quotient
    /// (`hierarchical.py:684`).
    fn y(&self, layer: u32) -> f32 {
        if self.max_layer == 0 {
            return 0.0;
        }
        (((f64::from(layer) / f64::from(self.max_layer)) * 2.0 - 1.0) * self.scale) as f32
    }
}

/// The same pipeline as [`run`](super::run), with the reference's axes: X centred on
/// `[-scale, scale]` over the whole ordering graph, Y over the layer range, `z = 0`.
///
/// **`scale` is finite and at or above 0, so `0.0` is a drawing.** `_sugiyama_layout(G, 0)`
/// (`hierarchical.py:684-685`) multiplies both columns by `scale` and so returns all zeros,
/// which this reproduces rather than refusing. `negative` and non-finite are refused: the
/// reference has no guard and would happily emit a drawing mirrored about the origin, but
/// `run` refuses a non-positive `layer_spacing` for the same reason and two entry points over
/// one pipeline should agree on what a length parameter may be.
pub fn run_scaled(topology: &Topology, scale: f32) -> Result<Geometry, StageError> {
    if !(scale.is_finite() && scale >= 0.0) {
        return Err(StageError::Param {
            name: "scale",
            rule: "finite and not negative",
        });
    }
    let (acyclic, layering, ordering) = layered(topology)?;
    let coords = Coords::build(&ordering, &layering, topology.node_count());
    let frame = Frame::of(
        &coords.0,
        layering.layer_of.iter().copied().max().unwrap_or(0),
        scale,
    );
    let count = topology.node_count() as usize;
    let mut x = Vec::with_capacity(count);
    let mut y = Vec::with_capacity(count);
    for v in 0..count {
        x.push(frame.x(coords.0[v]));
        y.push(frame.y(layering.layer_of[v]));
    }
    let paths = scaled_paths(&acyclic, &layering, &coords, &frame);
    let mut notes = acyclic.notes;
    notes.extend(layering.notes);
    Ok(Geometry::planar(
        NodeGeometry::Point { x, y },
        EdgeGeometry::Polyline(paths),
        notes,
    ))
}

/// Every edge's dummy-chain interior points on the same axes as the real nodes. A reversed
/// edge's chain was built tail-to-head in acyclic order and is walked back to front, as in
/// [`edge_paths`](super::routing::edge_paths).
fn scaled_paths(acyclic: &Acyclic, layering: &Layering, coords: &Coords, frame: &Frame) -> Paths {
    let mut offsets = Vec::with_capacity(layering.route.len() + 1);
    let mut pts = Vec::new();
    offsets.push(0);
    for (e, route) in layering.route.iter().enumerate() {
        if let Route::Chain { first, count } = *route {
            let back = usize::from(acyclic.reversed[e]);
            for step in 0..count {
                let d = if back == 1 {
                    first + count - 1 - step
                } else {
                    first + step
                };
                pts.push(frame.x(coords.0[d as usize]));
                pts.push(frame.y(layering.layer_of[d as usize]));
            }
        }
        offsets.push((pts.len() / 2) as u32);
    }
    Paths { offsets, pts }
}

#[cfg(test)]
mod tests;
