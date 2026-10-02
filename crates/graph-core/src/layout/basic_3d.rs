//! SciGraphs' four graph-free 3D placements: `SPHERE`, `HELIX`, `CUBE` and `SPIRAL_3D`
//! (`SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:22-103` and `:36-63`), the ones
//! that take `(num_nodes, scale)` and **no topology at all**. They are ported together,
//! in one module, for the reason the job names them together: the reference puts all four
//! in `basic.py` behind no dispatch beyond the name, they share the same two arguments,
//! and one differential (`harness/oracle-basic-3d.py`) arms three of them against
//! SciGraphs -- **not** [`spiral`], which that arm's `ARMS` does not cover yet.
//!
//! **They read the node count and nothing else.** Every edge is ignored, so a graph and
//! its edgeless version draw identically — that is the reference's own behaviour, and it
//! is worth saying out loud because it is the one thing a reader is most likely to
//! mistake for a bug. The four ids are therefore one kernel over four closed forms, and
//! each publishes its own id because each produces a different snapshot.
//!
//! **`CUBE` is the only one of the four that draws from a stream**, and this port does
//! not: see [`cube`] for the written seeding decision. `SPHERE`, `HELIX` and `SPIRAL_3D`
//! are closed form with no random number anywhere, so none owes a seed and none publishes
//! one.
//!
//! **Two of the four are spirals under two different names.** [`spiral`] is SciGraphs'
//! conical 3D spiral (`basic.py:36-63`); `layout.spiral` — a different module, one level
//! up — is graph-core's planar Archimedean spiral, a port of networkx's `spiral_layout`.
//! SciGraphs calls only the first. See [`spiral`]'s own header.
//!
//! No rescale, and that is deliberate: networkx's `rescale_layout` is not in the
//! reference for any of these four — `_sphere_layout`, `_helix_layout`, `_cube_layout` and
//! `_spiral_layout_3d` return `scale`-multiplied positions directly (`basic.py:34`,
//! `:79-81`, `:97-101`, `:61-63`). So the whole extent of the drawing is `scale`, and
//! nothing here recentres it.

use super::Geometry;
use crate::index::Topology;
use crate::stage::StageError;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// [`sphere`], [`helix`], [`cube`] and [`spiral`] are `pub` because each publishes the
/// capability id the registry registers it under; their `run` functions stay `pub(super)`,
/// because the id and the [`Topology`]-taking wrapper above are the module's whole public
/// surface.
pub mod cube;
pub mod helix;
pub mod sphere;
pub mod spiral;

/// SciGraphs' `apply_graph_layout` default scale (`layouts/dispatcher.py:14`), the value
/// the dispatcher hands each of the three functions it calls.
///
/// Published as a constant rather than a `Params` field because the reference takes one
/// `scale` with no default of its own and no caller in SciGraphs ever passes anything
/// else — so a struct would be a knob with one setting. The convention is the sibling
/// `layout.hierarchical3d` states (`layout/hierarchical_3d.rs:52-55`).
pub const SCALE: f64 = 5.0;

/// `Point` nodes in space at the three `f64` columns, narrowed to `f32` once.
///
/// Never [`Geometry::planar`]: the z column is the only thing that makes the snapshot a
/// 3D one, and a layout that dropped it would answer with a silently 2D drawing under a
/// 3D name.
pub(super) fn in_space(x: &[f64], y: &[f64], z: &[f64]) -> Geometry {
    let narrow = |column: &[f64]| column.iter().map(|&v| v as f32).collect::<Vec<f32>>();
    Geometry::in_space(
        NodeGeometry::Point {
            x: narrow(x),
            y: narrow(y),
        },
        EdgeGeometry::Line,
        Vec::new(),
        narrow(z),
    )
}

/// The node count these three read, which is the whole of their input.
pub(super) fn count(topology: &Topology) -> u32 {
    topology.node_count()
}

/// [`sphere::run`], the `SPHERE` placement. Never refuses.
pub fn sphere(topology: &Topology) -> Result<Geometry, StageError> {
    let geometry = sphere::run(count(topology))?;
    debug_assert_eq!(geometry.dim(), graph_contract::snapshot::Dim::D3);
    Ok(geometry)
}

/// [`helix::run`], the `HELIX` placement. Never refuses.
pub fn helix(topology: &Topology) -> Result<Geometry, StageError> {
    helix::run(count(topology))
}

/// [`cube::run`], the `CUBE` placement. Never refuses.
pub fn cube(topology: &Topology) -> Result<Geometry, StageError> {
    cube::run(count(topology))
}

/// [`spiral::run`], the `SPIRAL_3D` placement. Never refuses.
///
/// **Its own id, not `layout.spiral`'s.** That id is the planar Archimedean spiral; this
/// one is the reference's conical 3D spiral, a different curve under a different name.
pub fn spiral(topology: &Topology) -> Result<Geometry, StageError> {
    let geometry = spiral::run(count(topology))?;
    debug_assert_eq!(geometry.dim(), graph_contract::snapshot::Dim::D3);
    Ok(geometry)
}

#[cfg(test)]
mod tests;
