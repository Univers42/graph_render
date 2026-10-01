//! `layout.circular.ring`: every node on the unit circle in dense-index order, edges
//! ignored. Reference: networkx 3.6 `circular_layout`
//! (`networkx/drawing/layout.py:129`): angle `k * 2*pi / n` for node `k` (the duplicate
//! `2*pi` endpoint dropped), then `rescale_layout`. A lone node sits at the centre.
//!
//! Not `layout.circular.radial` (`super`), which is a tree radial over the repaired
//! hierarchy, and not `layout.circular.hierarchy` (`super::hierarchy`), which is
//! SciGraphs' own `CIRCULAR_HIERARCHY` closed form: this one shares with both nothing but
//! the word "circular" — it is the only ring layout that reads no structure at all.
//!
//! Angles are `f64` where networkx narrows them to `f32`, so the differential against it
//! is a tolerance (1e-6), never bytes. Nothing here is a heuristic.

use super::super::Geometry;
use super::super::coords::{point_geometry, rescale};
use crate::index::Topology;
use crate::stage::StageError;

/// The layout's capability id, which is also its hash-gate stage.
pub const ID: &str = "layout.circular.ring";

/// Runs the ring layout; never refuses.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let count = topology.node_count() as usize;
    if count < 2 {
        return Ok(point_geometry(&vec![0.0; count], &vec![0.0; count]));
    }
    let step = 1.0 / count as f64;
    let angles = (0..count).map(|k| k as f64 * step * (2.0 * core::f64::consts::PI));
    let (mut x, mut y): (Vec<f64>, Vec<f64>) = angles.map(|a| (libm::cos(a), libm::sin(a))).unzip();
    rescale(&mut x, &mut y);
    Ok(point_geometry(&x, &y))
}

#[cfg(test)]
mod tests;
