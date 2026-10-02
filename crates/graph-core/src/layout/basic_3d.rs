//! SciGraphs' three graph-free 3D placements: `SPHERE`, `HELIX` and `CUBE`
//! (`SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:22-103`), the ones that take
//! `(num_nodes, scale)` and **no topology at all**. They are ported together, in one
//! module, for the reason the job names them together: the reference puts all three in
//! `basic.py` behind no dispatch beyond the name, they share the same two arguments, and
//! one differential (`harness/oracle-basic-3d.py`) arms all three against SciGraphs.
//!
//! **They read the node count and nothing else.** Every edge is ignored, so a graph and
//! its edgeless version draw identically — that is the reference's own behaviour, and it
//! is worth saying out loud because it is the one thing a reader is most likely to
//! mistake for a bug. The three ids are therefore one kernel over three closed forms, and
//! each publishes its own id because each produces a different snapshot.
//!
//! **`CUBE` is the only one of the three that draws from a stream**, and it draws from the
//! reference's own: see [`cube`] for the generator and the seeding decision. `SPHERE` and
//! `HELIX` are closed form with no random number anywhere, so neither owes a seed and
//! neither publishes one.
//!
//! No rescale, and that is deliberate: networkx's `rescale_layout` is not in the
//! reference for any of these three — `_sphere_layout`, `_helix_layout` and
//! `_cube_layout` return `scale`-multiplied positions directly (`basic.py:34`, `:79-81`,
//! `:97-101`). So the whole extent of the drawing is `scale`, and nothing here
//! recentres it.

use super::Geometry;
use crate::index::Topology;
use crate::stage::StageError;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// [`sphere`], [`helix`] and [`cube`] are `pub` because each publishes the capability id
/// the registry registers it under; their `run` functions stay `pub(super)`, because the
/// id and the [`Topology`]-taking wrapper above are the module's whole public surface.
pub mod bipartite_3d;
pub mod cube;
pub mod helix;
pub mod sphere;

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

/// [`bipartite_3d::run`], the `BIPARTITE_3D` placement. Never refuses.
///
/// **The one member of this module that reads the graph**, and the only reason it lives here
/// rather than beside `layout.bipartite` is that it shares [`SCALE`] and [`in_space`] with
/// the other three and nothing else — the module doc's "reads the node count and nothing
/// else" is true of `SPHERE`, `HELIX` and `CUBE`, and is stated here so it is not read as a
/// claim about this one. Its id is `layout.bipartite_3d`, deliberately outside the
/// `layout.basic3d.*` namespace, because that namespace means "reads no graph".
pub fn bipartite_3d(topology: &Topology) -> Result<Geometry, StageError> {
    bipartite_3d::run(topology)
}

#[cfg(test)]
mod tests;
