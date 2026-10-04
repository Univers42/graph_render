//! SciGraphs' four graph-free 3D placements: `SPHERE`, `HELIX`, `CUBE` and `SPIRAL_3D`
//! (`SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:22-103` and `:36-63`), the ones
//! that take `(num_nodes, scale)` and **no topology at all**. They are ported together,
//! in one module, for the reason the job names them together: the reference puts all four
//! in `basic.py` behind no dispatch beyond the name, they share the same two arguments,
//! and one differential (`harness/oracle-basic-3d.py`) arms all four against
//! SciGraphs, with [`mod@spiral`] the fourth since job `sg-basic3d-spiral-oracle`.
//!
//! **They read the node count and nothing else.** Every edge is ignored, so a graph and
//! its edgeless version draw identically — that is the reference's own behaviour, and it
//! is worth saying out loud because it is the one thing a reader is most likely to
//! mistake for a bug. The four ids are therefore one kernel over four closed forms, and
//! each publishes its own id because each produces a different snapshot.
//!
//! **`CUBE` is the only one of the four that draws from a stream**, and it draws from the
//! reference's own: see [`mod@cube`] for the generator and the seeding decision. `SPHERE`,
//! `HELIX` and `SPIRAL_3D` are closed form with no random number anywhere, so none owes a
//! seed and none publishes one.
//!
//! **Two of the four are spirals under two different names.** [`mod@spiral`] is SciGraphs'
//! conical 3D spiral (`basic.py:36-63`); `layout.spiral` — a different module, one level
//! up — is graph-core's planar Archimedean spiral, a port of networkx's `spiral_layout`.
//! SciGraphs calls only the first. See [`mod@spiral`]'s own header.
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

/// [`mod@sphere`], [`mod@helix`], [`mod@cube`] and [`mod@spiral`] are `pub` because each
/// publishes the capability id the registry registers it under; their `run` functions stay
/// `pub(super)`,
/// because the id and the [`Topology`]-taking wrapper above are the module's whole public
/// surface. [`CORNERS`] is re-exported here because the `CUBE` row of the ledger names
/// `layout::basic_3d::CORNERS` as its escape hatch, and an escape hatch has to be the path
/// the ledger says it is.
pub mod bipartite_3d;
pub mod cube;
pub mod helix;
pub mod sphere;
pub mod spiral;

pub use cube::CORNERS;

/// SciGraphs' `apply_graph_layout` **default** scale (`layouts/dispatcher.py:14`), and the
/// scale every registered id in this module is drawn at.
///
/// The dispatcher hands it to each of the **five** functions that read it from this module:
/// `_sphere_layout` (`basic.py:22`), `_spiral_layout_3d` (`:36`, reached at
/// `dispatcher.py:105-106`), `_helix_layout` (`:65`), `_cube_layout` (`:83`) and
/// `_bipartite_layout_3d` (`layouts/bipartite.py`).
///
/// **It is a default, not the only value the reference accepts** — and this comment used to
/// say the opposite, which was false. SciGraphs' `layout_scale` is a user-facing
/// `FloatProperty` spanning `min=0.1, max=100.0`
/// (`SciGraphs/properties/scene_properties.py:478-483`), passed as `scale=self.scale` into
/// `apply_graph_layout` (`SciGraphs/ui/operators/scigraphs/layout_operators.py:315`) and
/// from there into `_sphere_layout` & co. (`layouts/dispatcher.py:101-110`). A user dragging
/// the slider to `12.0` reaches this module's kernel; there was no way to say so.
///
/// So `scale` is a **parameter of the five `*_scaled` entry points below**, one per id, and
/// this constant is what each of the plain wrappers passes. The registered default does not
/// move: `run_scaled(n, SCALE)` is the identical product sequence, `5.0` is exact in
/// `f64`, and no hashed snapshot changes. The convention is the one
/// `layout/grid/scaled.rs` sets for a second kernel and `layout/random.rs` for a second
/// stream: an additive entry point, never a moved default.
pub const SCALE: f64 = 5.0;

/// The one scale rule the five `*_scaled` entry points share: finite and above 0, the rule
/// `grid/scaled.rs` states for `run_scaled` and `GridParams` for `spacing`. Without it a
/// `NaN` or a negative scale reaches the kernel and is refused later, at `snapshot`, under
/// `node.x` — naming a column rather than the argument that caused it.
fn checked(scale: f64) -> Result<f64, StageError> {
    if scale.is_finite() && scale > 0.0 {
        Ok(scale)
    } else {
        Err(StageError::Param {
            name: "scale",
            rule: "finite and above 0",
        })
    }
}

/// `Point` nodes in space at the three `f64` columns, narrowed to `f32` once.
///
/// Never [`Geometry::planar`]: the z column is the only thing that makes the snapshot a
/// 3D one, and a layout that dropped it would answer with a silently 2D drawing under a
/// 3D name.
///
/// **This is the one place the 3D shape is checked**, for every layout that reaches it —
/// the five wrappers below and `layout::random`'s SciGraphs arm. `Geometry::in_space` does
/// not length-check `z` by design (`layout/mod.rs`: "there is one place the rule lives",
/// which is `snapshot`, refusing a short column as `SnapshotError::Length { column:
/// "node.z" }`), so the length is asserted here, in the builder, where a future edit that
/// pushed one value per node on two of three columns trips in every debug build instead of
/// hashing a z column of the wrong size. A `debug_assert` and not a refusal on purpose:
/// no input reaches it today, and turning it into an error would put a branch in five
/// release paths to guard against an edit that has not happened.
pub(super) fn in_space(x: &[f64], y: &[f64], z: &[f64]) -> Geometry {
    let narrow = |column: &[f64]| column.iter().map(|&v| v as f32).collect::<Vec<f32>>();
    let (x, y, z) = (narrow(x), narrow(y), narrow(z));
    debug_assert_eq!(
        x.len(),
        y.len(),
        "the x and y columns must be one value per node"
    );
    debug_assert_eq!(
        z.len(),
        x.len(),
        "the z column must be one value per node, or snapshot refuses it under node.z"
    );
    Geometry::in_space(
        NodeGeometry::Point { x, y },
        EdgeGeometry::Line,
        Vec::new(),
        z,
    )
}

/// The node count the graph-free placements read. **The whole of their input for four of
/// them** — `sphere`, `helix`, `cube` and `spiral` ignore every edge — and not for
/// `bipartite_3d`, which reads the graph's own 2-colouring.
pub(super) fn count(topology: &Topology) -> u32 {
    topology.node_count()
}

/// [`sphere::run`], the `SPHERE` placement at [`SCALE`]. Never refuses.
pub fn sphere(topology: &Topology) -> Result<Geometry, StageError> {
    sphere::run(count(topology))
}

/// [`sphere::run_scaled`]: `SPHERE` at the scale the caller asks for, which is what
/// SciGraphs' `layout_scale` slider passes (`scene_properties.py:478-483`).
pub fn sphere_scaled(topology: &Topology, scale: f64) -> Result<Geometry, StageError> {
    sphere::run_scaled(count(topology), checked(scale)?)
}

/// [`helix::run`], the `HELIX` placement at [`SCALE`]. Never refuses.
pub fn helix(topology: &Topology) -> Result<Geometry, StageError> {
    helix::run(count(topology))
}

/// [`helix::run_scaled`]: `HELIX` at an explicit `scale`.
pub fn helix_scaled(topology: &Topology, scale: f64) -> Result<Geometry, StageError> {
    helix::run_scaled(count(topology), checked(scale)?)
}

/// [`cube::run`], the `CUBE` placement at [`SCALE`]. Never refuses.
pub fn cube(topology: &Topology) -> Result<Geometry, StageError> {
    cube::run(count(topology))
}

/// [`cube::run_scaled`]: `CUBE` at an explicit `scale`, with the reference's own seed.
pub fn cube_scaled(topology: &Topology, scale: f64) -> Result<Geometry, StageError> {
    cube::run_scaled(count(topology), checked(scale)?)
}

/// [`spiral::run`], the `SPIRAL_3D` placement at [`SCALE`]. Never refuses.
///
/// **Its own id, not `layout.spiral`'s.** That id is the planar Archimedean spiral; this
/// one is the reference's conical 3D spiral, a different curve under a different name.
pub fn spiral(topology: &Topology) -> Result<Geometry, StageError> {
    spiral::run(count(topology))
}

/// [`spiral::run_scaled`]: `SPIRAL_3D` at an explicit `scale`.
pub fn spiral_scaled(topology: &Topology, scale: f64) -> Result<Geometry, StageError> {
    spiral::run_scaled(count(topology), checked(scale)?)
}

/// [`bipartite_3d::run`], the `BIPARTITE_3D` placement at [`SCALE`]. Never refuses.
///
/// **The one member of this module that reads the graph**, and the only reason it lives here
/// rather than beside `layout.bipartite` is that it shares [`SCALE`] and [`in_space`] with
/// the others and nothing else — the module doc's "reads the node count and nothing
/// else" is true of `SPHERE`, `HELIX`, `CUBE` and `SPIRAL_3D`, and is stated here so it is not read as a
/// claim about this one. Its id is `layout.bipartite_3d`, deliberately outside the
/// `layout.basic3d.*` namespace, because that namespace means "reads no graph".
pub fn bipartite_3d(topology: &Topology) -> Result<Geometry, StageError> {
    bipartite_3d::run(topology)
}

/// [`bipartite_3d::run_scaled`]: `BIPARTITE_3D` at an explicit `scale`.
pub fn bipartite_3d_scaled(topology: &Topology, scale: f64) -> Result<Geometry, StageError> {
    bipartite_3d::run_scaled(topology, checked(scale)?)
}

#[cfg(test)]
mod tests;
