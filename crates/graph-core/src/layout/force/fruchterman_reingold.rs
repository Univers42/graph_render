//! Fruchterman-Reingold (`layout.force.fruchterman_reingold`), written from the prose spec
//! `docs/layouts/layout.force.fruchterman_reingold.md` and the paper (Fruchterman and
//! Reingold, Software: Practice and Experience 21(11), 1991) only, per
//! `docs/decisions/layouts-igraph.md`. 2D, dense, unweighted; the spec's grid variant
//! (n > 1000) is not implemented, so past that size this is the same O(n^2) loop.
//!
//! The iteration itself is [`kernel::solve`], shared with
//! [`super::fruchterman_reingold_3d::FruchtermanReingold3D`] through a `const D`; this file is
//! the parameters, the 2D stage and the geometry it builds.

#[cfg(test)]
mod tests;

pub(crate) mod kernel;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

pub(crate) use kernel::Axis;
pub(super) use kernel::sqrt;

/// Parameters SciGraphs leaves at igraph's defaults (`niter` 500, `start_temp` sqrt(n)/10).
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

/// Node count past which the dense loop is no longer usable; see the registry entry.
pub const FR_CEILING: u64 = 2_000;

/// The capability id of the 3D sibling, named here so a caller can reach both dimensions'
/// stages through one import.
pub use super::fruchterman_reingold_3d::FruchtermanReingold3D;

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
        let pos = kernel::solve::<2>(topology, params)?;
        // `Point` nodes at the columns' own `f32` values, straight `Line` edges, no notes: the
        // planar constructor, so no 2D byte moves by carrying a z column it never had.
        Ok(Geometry::planar(
            NodeGeometry::Point {
                x: narrow(&pos, 0),
                y: narrow(&pos, 1),
            },
            EdgeGeometry::Line,
            Vec::new(),
        ))
    }
}

/// One live axis of the solved positions, narrowed once.
fn narrow(pos: &[Axis], axis: usize) -> Vec<f32> {
    pos.iter().map(|p| p[axis] as f32).collect()
}

/// [`kernel::start_positions`] at `D = 2`, narrowed to the two columns its other callers read.
///
/// Four layouts start from the same uniform box (`graphopt`, and this module's own history), so
/// the draw is one function rather than four; it is `pub(super)` because only `graphopt` reaches
/// outside this module for it, and it keeps the `D = 2` reading explicit at the call site.
pub(super) fn start_positions(n: usize, seed: u32) -> Vec<[f64; 2]> {
    kernel::start_positions::<2>(n, seed)
        .into_iter()
        .map(|point| [point[0], point[1]])
        .collect()
}
