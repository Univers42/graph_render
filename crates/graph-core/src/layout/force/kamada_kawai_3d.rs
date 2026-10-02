//! `layout.force.kamada_kawai_3d`: [`super::KamadaKawai`] at `dim = 3`.
//!
//! SciGraphs calls KK with `dim=3` and nothing else (`igraph_layouts.py:99-107`: `params =
//! {'dim': 3}`, with `maxiter`, `epsilon` and `kkconst` added only when the caller gives a
//! positive value), so the 3D variant is the one the reference actually runs and this id is what
//! `IGRAPH_KK` compares against.
//!
//! **It is not a second algorithm.** The spring model, the all-pairs distances, the gradient, the
//! Hessian block, the vertex pick and the incremental update are the spec's steps 2 to 5.5
//! (`docs/layouts/layout.force.kamada_kawai.md`), and they are the very same
//! [`super::kamada_kawai::descent`] functions the 2D stage calls, at `D = 3`. Two things differ:
//! the geometry ([`Geometry::in_space`] rather than [`Geometry::planar`], so the snapshot carries
//! a z column and is labelled 0.4), and the linear algebra — the 3x3 Hessian block and Cramer's
//! rule where the 2D stage uses the closed 2x2 form. The start is the other difference, and it is
//! a *start*: the spec's sphere placement instead of a circle, from the formula written out under
//! "The 3D start: the sphere" in that same spec.
//!
//! **No seed, and therefore no licence gap on this row.** KK's default path takes no random
//! numbers at all (spec, "Where randomness enters"), so this layout and igraph's agree on the
//! start exactly. What still separates them is the solver: Newton descent finds a local minimum,
//! and two implementations that differ in the last ulp of a gradient settle in different ones.

use super::kamada_kawai::{Axis, KkParams, solve};
use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// The 3D layout's capability id, which is also its hash-gate stage. A separate id from the 2D
/// stage's because a separate snapshot: the same drawing, labelled 0.4 and carrying a z column,
/// hashes differently from the 2D one.
pub const ID_3D: &str = "layout.force.kamada_kawai_3d";

/// Kamada-Kawai at `dim = 3`: this same solver, in space.
///
/// Shares [`KkParams`] with the 2D stage rather than declaring its own — igraph takes one
/// `maxiter`, one `epsilon` and one `kkconst` for both dimensions (`igraph_layouts.py:99-105`),
/// so a caller comparing the two dimensions moves one value, not two.
pub struct KamadaKawai3D;

impl Stage for KamadaKawai3D {
    type Params = KkParams;
    const ID: &'static str = ID_3D;

    /// [`solve`] at `D = 3`, then a geometry with the z column attached. The narrowing is
    /// `f64 -> f32` per axis, the same cast the 2D stage makes, so a 3D node's x and y are the
    /// `f32` of the same `f64` the 2D solve would have reached on its first two axes — not the
    /// same value, because the third axis changes the gradient, but the same cast.
    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let pos = solve::<3>(topology, params)?;
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

#[cfg(test)]
#[path = "kamada_kawai_3d/tests.rs"]
mod tests;
