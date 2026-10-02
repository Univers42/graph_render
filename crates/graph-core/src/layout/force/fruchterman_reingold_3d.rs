//! `layout.force.fruchterman_reingold_3d`: [`super::FruchtermanReingold`] at `dim = 3`.
//!
//! SciGraphs calls FR with `dim=3` (`igraph_layouts.py:74`), so this id — not the 2-D one — is
//! the row the conformance matrix compares. It is the same algorithm as the 2-D stage: the
//! iteration is [`super::fr_kernel`] at `D = 3`, the parameters and their defaults are the
//! 2-D stage's [`FrParams`] (`niter` 500, `start_temp` `sqrt(n)/10`), and the only thing that
//! differs is the arithmetic on one more axis and the geometry handed back.
//!
//! **What is different, and it is exactly one thing:** [`Geometry::in_space`] instead of
//! [`Geometry::planar`], so the snapshot carries the z column and is labelled 0.4 rather than
//! 0.3. Nothing about the loop needs a dimension-specific branch: the pair term folds each
//! component of `delta` into the accumulator of its own axis, which is the decision
//! `docs/layouts/layout.force.fruchterman_reingold.md` records under "Resolved for this tree"
//! (igraph's own 3-D disconnected correction types the z component into `D_y`; this port does
//! not reproduce that). igraph has no grid variant in 3-D, so the spec's `n > 1000` grid never
//! applies here.
//!
//! **Still chaotic, still not igraph's coordinates.** The start is a random box drawn from our
//! own seeded generator, not igraph's, so the two streams part company at the first coordinate
//! and the differential compares layout stress, not positions
//! (`crates/graph-cli/src/oracle_python/igraph.rs`).

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::force::fr_kernel;
use crate::layout::force::fruchterman_reingold::FrParams;
use crate::layout::force::simple_graph;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// The 3-D layout's capability id, which is also its hash-gate stage. A separate id from the
/// 2-D stage's because a separate snapshot: the same drawing with a z column hashes
/// differently from the 2-D one and must never be confused with it.
pub const ID_3D: &str = "layout.force.fruchterman_reingold_3d";

/// [`fr_kernel::layout`] at `D = 3`, then a geometry with the z column attached. The
/// narrowing is `f64 -> f32` per column, the same cast the 2-D stage makes.
pub struct FruchtermanReingold3D;

impl Stage for FruchtermanReingold3D {
    type Params = FrParams;
    const ID: &'static str = ID_3D;

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let n = topology.node_count() as usize;
        let pos = fr_kernel::layout::<3>(&simple_graph(topology), n, params);
        if let Some(axis) = non_finite_axis(&pos) {
            return Err(StageError::NonFinite { column: AXES[axis] });
        }
        let column = |axis: usize| -> Vec<f32> { pos.iter().map(|p| p[axis] as f32).collect() };
        Ok(Geometry::in_space(
            NodeGeometry::Point {
                x: column(0),
                y: column(1),
            },
            EdgeGeometry::Line,
            Vec::new(),
            column(2),
        ))
    }
}

/// The column names D9 reports a non-finite value under, in axis order.
const AXES: [&str; 3] = ["node.x", "node.y", "node.z"];

/// The first axis holding a non-finite coordinate, walked in axis order so the answer does not
/// depend on which of several bad axes comes first in the node order.
fn non_finite_axis<const D: usize>(pos: &[[f64; D]]) -> Option<usize> {
    (0..D).find(|&axis| pos.iter().any(|p| !p[axis].is_finite()))
}

#[cfg(test)]
#[path = "fruchterman_reingold_3d/tests.rs"]
mod tests;
