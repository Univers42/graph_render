//! Fruchterman-Reingold (`layout.force.fruchterman_reingold`), written from the prose spec
//! `docs/layouts/layout.force.fruchterman_reingold.md` and the paper (Fruchterman and
//! Reingold, Software: Practice and Experience 21(11), 1991) only, per
//! `docs/decisions/layouts-igraph.md`. 2D, dense, unweighted; the spec's grid variant
//! (n > 1000) is not implemented, so past that size this is the same O(n^2) loop.
//!
//! The iteration itself lives in [`super::fr_kernel`] at `D = 2`, shared with
//! `layout.force.fruchterman_reingold_3d` at `D = 3`: the two differ in one axis of arithmetic
//! and in the geometry they hand back, so this file is the id, the parameters, the ceiling and
//! the narrowing. [`sqrt`] and [`start_positions`] stay here because three other stages
//! (`lgl`, `drl`, `graphopt`) already read them from this module.

#[cfg(test)]
mod tests;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::force::fr_kernel;
use crate::layout::force::simple_graph;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// Parameters SciGraphs leaves at igraph's defaults (`niter` 500, `start_temp` sqrt(n)/10).
/// Shared with the 3-D stage: the spec's table gives one set of defaults for both dimensions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrParams {
    /// Iteration count; the loop always runs exactly this many.
    pub niter: u32,
    /// Initial temperature; `None` means `sqrt(n) / 10`.
    pub start_temp: Option<f64>,
    /// Seeds the start box and the tie-breaking noise (D5: no global generator).
    pub seed: u32,
}

impl Default for FrParams {
    fn default() -> Self {
        Self {
            niter: 500,
            start_temp: None,
            seed: 0,
        }
    }
}

/// Node count past which the dense loop is no longer usable; see the registry entry. The 3-D
/// stage registers the same ceiling, and says in its `Metadata` that it is this estimate.
pub const FR_CEILING: u64 = 2_000;

/// Fruchterman-Reingold layout stage.
///
/// Ponytail: chaotic like every force layout (one added node is a different picture), and
/// the noise is our counter hash, not igraph's generator, so coordinates never match
/// igraph's; the differential compares layout stress, not positions. Above 1000 nodes
/// igraph switches to a stale-grid approximation that this port does not have, so the two
/// differ most exactly where igraph is least exact.
pub struct FruchtermanReingold;

impl Stage for FruchtermanReingold {
    type Params = FrParams;
    const ID: &'static str = "layout.force.fruchterman_reingold";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let n = topology.node_count() as usize;
        let pos = fr_kernel::layout::<2>(&simple_graph(topology), n, params);
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

pub(super) fn sqrt(v: f64) -> f64 {
    libm::sqrt(v)
}

/// Uniform in the square of side sqrt(n) centred on the origin: the shared kernel's box
/// start at `D = 2`, one draw per axis per vertex in axis order.
pub(super) fn start_positions(n: usize, seed: u32) -> Vec<[f64; 2]> {
    fr_kernel::start::<2>(n, seed)
}
