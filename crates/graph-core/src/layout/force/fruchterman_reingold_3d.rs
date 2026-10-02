//! `layout.force.fruchterman_reingold_3d`: [`super::FruchtermanReingold`] at `dim = 3`.
//!
//! SciGraphs calls FR with `dim=3` (`igraph_layouts.py:74`: `params = {'niter': iterations,
//! 'dim': 3}`, no seed), so the 3D variant is the one the reference actually runs and this id is
//! what `IGRAPH_FR` compares against. It is **not** a second algorithm: the start box, the pair
//! repulsion, the edge pull, the move and the linear cooling are the spec's dimension-generic
//! steps 1 to 3.5 (`docs/layouts/layout.force.fruchterman_reingold.md:39-59`), and they are the
//! very same [`super::fruchterman_reingold::kernel`] functions the 2D stage calls, at `D = 3`.
//! So this is a separate id and a separate snapshot — which hashes differently and must never be
//! confused with the 2D one — over one kernel, the way `layout.force.spring3d` is
//! `layout.force.spring`.
//!
//! **What is different, and it is exactly one thing:** the geometry. [`Geometry::in_space`]
//! instead of [`Geometry::planar`], so the snapshot carries the z column and is labelled 0.4
//! rather than 0.3. Every step of the solve walks one more axis; nothing else about it differs.
//!
//! **The start is a random box, not a deterministic sphere.** The spec's step 1 draws every
//! coordinate uniformly from `[-sqrt(n)/2, +sqrt(n)/2]` per axis with no seed given
//! (`fruchterman_reingold.md:41-42`), so this stage is as seed-dependent as the 2D one: it draws
//! from graph-core's Mulberry32 and igraph from Mersenne Twister, and the two part company at the
//! first coordinate. That is the licence gap recorded as `G_IGRAPH_SEED`, not a port defect.

use super::fruchterman_reingold::{Axis, FrParams, kernel};
use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// The 3D layout's capability id, which is also its hash-gate stage. A separate id from the 2D
/// stage's because a separate snapshot: the same drawing, labelled 0.4 and carrying a z column,
/// hashes differently from the 2D one.
pub const ID_3D: &str = "layout.force.fruchterman_reingold_3d";

/// Fruchterman-Reingold at `dim = 3`: this same algorithm, in space.
///
/// Shares [`FrParams`] with the 2D stage rather than declaring its own — igraph takes one
/// `niter` and one `start_temp` for both dimensions (`igraph_layouts.py:74-77`), so a caller
/// comparing the two dimensions moves one value, not two.
pub struct FruchtermanReingold3D;

impl Stage for FruchtermanReingold3D {
    type Params = FrParams;
    const ID: &'static str = ID_3D;

    /// [`kernel::solve`] at `D = 3`, then a geometry with the z column attached. The narrowing
    /// is `f64 -> f32` per axis, the same cast the 2D stage makes, so a 3D node's x and y are the
    /// `f32` of the same `f64` the 2D kernel would have produced on its first two axes.
    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let pos = kernel::solve::<3>(topology, params)?;
        Ok(Geometry::in_space(
            NodeGeometry::Point {
                x: narrow(&pos, 0),
                y: narrow(&pos, 1),
            },
            EdgeGeometry::Line,
            Vec::new(),
            narrow(&pos, 2),
        ))
    }
}

/// One live axis of the solved positions, narrowed once.
fn narrow(pos: &[Axis], axis: usize) -> Vec<f32> {
    pos.iter().map(|p| p[axis] as f32).collect()
}
