//! Kamada-Kawai (`layout.force.kamada_kawai`), written from the prose spec
//! `docs/layouts/layout.force.kamada_kawai.md` and the paper (Kamada and Kawai,
//! Information Processing Letters 31(1), 1989) only, per `docs/decisions/layouts-igraph.md`.
//! 2D, unweighted (every edge has length 1, so all-pairs distances are breadth-first).
//!
//! The descent lives in [`super::kk_kernel`] at `D = 2`, shared with
//! `layout.force.kamada_kawai_3d` at `D = 3`; this file is the id, the parameters, the ceiling,
//! the circle start and the narrowing. The circle is this port's own: the spec records that
//! igraph's own 2-D circle lands its first and last vertex on the same point and explicitly
//! does not bless it, and the 2-D id is pinned byte for byte, so it is left exactly as it was.

#[cfg(test)]
mod tests;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::force::kk_kernel;
use crate::layout::force::simple_graph;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// Parameters SciGraphs leaves at igraph's defaults. Shared with the 3-D stage: the spec's
/// table gives one set of defaults for both dimensions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KkParams {
    /// Maximum single-vertex moves; `None` means `50 * n`.
    pub maxiter: Option<u32>,
    /// Stop when the largest squared gradient norm falls below this; 0 never stops early.
    pub epsilon: f64,
    /// Spring strength constant; `None` means `n`.
    pub kkconst: Option<f64>,
}

impl Default for KkParams {
    fn default() -> Self {
        Self {
            maxiter: None,
            epsilon: 0.0,
            kkconst: None,
        }
    }
}

/// Node count past which the O(n^2) matrices and 50 n^2 moves are no longer usable. The 3-D
/// stage registers the same ceiling, and says in its `Metadata` that it is this estimate.
pub const KK_CEILING: u64 = 2_000;

/// Kamada-Kawai layout stage. Fully deterministic: the start is a circle, no generator.
///
/// Ponytail: Newton descent on one vertex at a time finds a local minimum of the spring
/// energy, not the global one, so a graph with a folded start can settle folded. An
/// edgeless graph with n >= 2 has no finite distance; igraph divides by zero there, this
/// port takes every distance as 1 instead, so it draws a regular polygon-like cloud.
pub struct KamadaKawai;

impl Stage for KamadaKawai {
    type Params = KkParams;
    const ID: &'static str = "layout.force.kamada_kawai";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let n = topology.node_count() as usize;
        let pos = kk_kernel::descend::<2>(&simple_graph(topology), n, circle_start(n), params);
        if pos.iter().flatten().any(|v| !v.is_finite()) {
            return Err(StageError::NonFinite { column: "node.x" });
        }
        Ok(Geometry::planar(
            NodeGeometry::Point {
                x: pos.iter().map(|p| p[0] as f32).collect(),
                y: pos.iter().map(|p| p[1] as f32).collect(),
            },
            EdgeGeometry::Line,
            Vec::new(),
        ))
    }
}

/// Vertices on a circle of radius `0.36 * sqrt(n)` (the spec's empirical start radius).
fn circle_start(n: usize) -> Vec<[f64; 2]> {
    let radius = 0.36 * libm::sqrt(n as f64);
    (0..n)
        .map(|i| {
            let angle = 2.0 * core::f64::consts::PI * i as f64 / n as f64;
            [radius * libm::cos(angle), radius * libm::sin(angle)]
        })
        .collect()
}
