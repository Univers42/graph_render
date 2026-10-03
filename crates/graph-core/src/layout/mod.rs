//! LAYOUT (`prompt.md` §3): the pluggable seam, topology in, geometry out. The seam's own
//! types, the [`snapshot`] call every layout goes through, and [`Geometry`]'s constructors
//! live here; the `tests` module holds the seam's tests.
//!
//! Every layout is a [`crate::stage::Stage`] and is listed in [`crate::registry`]. They are
//! `basic_3d`, `bipartite`, `circle_packing`, `circular`, `force`, `forceatlas2`,
//! `graphviz`, `grid`, `hierarchical_3d`, `hierarchy`, `pivot_mds`, `planarity`, `radial`,
//! `random`, `spectral`, `spectral_stage`, `spiral`, `sugiyama`, `tidy_tree` and `treemap`;
//! `hierarchy` is the one repaired tree the tree layouts share. `adjacency` and `coords` are
//! private, and `tests` is `#[cfg(test)]`.

mod adjacency;
pub mod basic_3d;
pub mod bipartite;
pub mod circle_packing;
pub mod circular;
mod coords;
pub mod force;
pub mod forceatlas2;
pub mod graphviz;
pub mod grid;
pub mod hierarchical_3d;
pub mod hierarchy;
pub mod pivot_mds;
pub mod planarity;
pub mod radial;
pub mod random;
pub mod spectral;
pub mod spectral_stage;
pub mod spiral;
pub mod sugiyama;
#[cfg(test)]
mod tests;
pub mod tidy_tree;
pub mod treemap;

use crate::index::Topology;
use crate::stage::StageError;
use graph_contract::binary::{Snapshot, SnapshotParts, StringTable};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use graph_contract::notes::{Note, Notes};
use graph_contract::snapshot::{Dim, label_for};

/// What a layout stage produces: one geometry for the whole snapshot, and what it
/// repaired or approximated on the way.
///
/// Built through [`Geometry::planar`] or [`Geometry::in_space`], never a struct literal:
/// the z column is the reason, so that the next field does not touch every layout.
#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    /// Every node's geometry, in the topology's node order.
    pub nodes: NodeGeometry,
    /// Every edge's geometry, in the topology's edge order.
    pub edges: EdgeGeometry,
    /// The stage's notes, in any order: [`snapshot`] sorts them.
    pub notes: Vec<Note>,
    /// The third coordinate, one per node in node order, for a layout that placed its
    /// nodes in space. `None` is a 2D layout — what every layout in the tree emits today
    /// — and the column's presence is the only thing that tells a 3D snapshot from a 2D
    /// one ([`graph_contract::binary::SnapshotParts::dim`]).
    pub z: Option<Vec<f32>>,
}

impl Geometry {
    /// A 2D geometry: no z column, so a snapshot of it is labelled 0.3 and no 2D byte
    /// moves when 3D exists.
    pub fn planar(nodes: NodeGeometry, edges: EdgeGeometry, notes: Vec<Note>) -> Self {
        Self {
            nodes,
            edges,
            notes,
            z: None,
        }
    }

    /// A 3D geometry: `z` is one coordinate per node, in node order. Its length and
    /// values are not checked here — a z that does not fit is refused by [`snapshot`]
    /// under `node.z`, like any other column, so there is one place the rule lives.
    pub fn in_space(
        nodes: NodeGeometry,
        edges: EdgeGeometry,
        notes: Vec<Note>,
        z: Vec<f32>,
    ) -> Self {
        Self {
            nodes,
            edges,
            notes,
            z: Some(z),
        }
    }

    /// The dimension a snapshot of this geometry is labelled for: [`Dim::D3`] when it
    /// carries a z column, [`Dim::D2`] when it does not. Read once, before any field is
    /// moved, so the label and the column cannot disagree.
    pub fn dim(&self) -> Dim {
        match self.z {
            Some(_) => Dim::D3,
            None => Dim::D2,
        }
    }

    /// The point geometry a force kernel's coordinates describe: [`Self::planar`] at
    /// `dim` 2 and [`Self::in_space`] with the third column at `dim` 3.
    ///
    /// **The one place a dimension becomes a geometry.** Every force kernel ends by
    /// building one of these, and none of them builds its own: a kernel that reached for
    /// [`Self::in_space`] directly would have to re-decide the question on every call
    /// site, and a kernel that reached for [`Self::planar`] would silently drop z — the
    /// F1 failure `docs/decisions/contract-3d.md` names, where a 3D run answers as 2D.
    /// `z` is read only at `dim` 3, so a 2D caller passing a zero-length column is fine.
    pub fn points(dim: usize, x: Vec<f32>, y: Vec<f32>, z: Vec<f32>) -> Self {
        let nodes = NodeGeometry::Point { x, y };
        match dim {
            3 => Self::in_space(nodes, EdgeGeometry::Line, Vec::new(), z),
            _ => Self::planar(nodes, EdgeGeometry::Line, Vec::new()),
        }
    }

    /// The largest distance between two nodes along either in-plane axis: the drawing's
    /// own size, as a caller that has no `scale` parameter can measure it.
    ///
    /// For [`NodeGeometry::Point`] that is the larger of the x and y ranges. Other kinds
    /// carry sizes rather than centres in their second column, so this returns `0.0`
    /// rather than reading a height as a y coordinate — a caller using this as a scale
    /// wants a number it can trust is a position span.
    pub fn extent(&self) -> f32 {
        let NodeGeometry::Point { x, y } = &self.nodes else {
            return 0.0;
        };
        span(x).max(span(y))
    }

    /// `self` with its edge geometry replaced by `edges`, and its nodes, notes and z
    /// column carried through untouched. **The way an edge-only post pass rebuilds a
    /// geometry**, so such a pass cannot drop the z column by forgetting it: there is
    /// nowhere else for it to build one.
    ///
    /// Not the only constructor any more — [`Self::with_nodes`] is, for a pass that moves
    /// nodes — so what this one guarantees is now read from
    /// [`crate::post::Metadata::moves_nodes`]: a pass declaring `false` rebuilds through
    /// this and its node columns come out byte-identical, which the composability matrix
    /// asserts for every such row.
    pub fn with_edges(&self, edges: EdgeGeometry) -> Self {
        Self {
            nodes: self.nodes.clone(),
            edges,
            notes: self.notes.clone(),
            z: self.z.clone(),
        }
    }

    /// `self` with its node geometry replaced by `nodes`, and its edges, notes and z
    /// column carried through untouched.
    ///
    /// **Reachable only from a post pass whose [`crate::post::Metadata`] declares
    /// `moves_nodes: true`** — today exactly one, [`crate::post::separate`]. That
    /// restriction is the whole point: `with_edges` alone could guarantee the z column
    /// survived every pass because there was nowhere else to build a geometry. Now there is,
    /// and the invariant that constructor encoded lives on the metadata field instead, where
    /// the matrix reads it per pass rather than assuming it. A pass declaring `moves_nodes`
    /// must also refuse a geometry carrying a z column — separating 2D discs under a z
    /// column would answer a question nobody asked, and
    /// `docs/decisions/contract-3d-verdict.md` condition 6 says no POST pass rewrites node
    /// columns. `separate` refuses with `StageError::Param` rather than moving `x`/`y` and
    /// leaving `z` behind.
    pub fn with_nodes(&self, nodes: NodeGeometry) -> Self {
        Self {
            nodes,
            edges: self.edges.clone(),
            notes: self.notes.clone(),
            z: self.z.clone(),
        }
    }
}

/// `max - min` over a column, or `0.0` when it is empty or not finite. Two passes so the
/// answer does not depend on the column's order.
fn span(column: &[f32]) -> f32 {
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for &v in column {
        if v.is_finite() {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    if lo.is_finite() && hi.is_finite() {
        hi - lo
    } else {
        0.0
    }
}

/// The snapshot of `geometry` laid over `topology`: the topology's stable ids and edge
/// endpoints in its own order, then the geometry, then the notes sorted by
/// `(code, index)` — the whole of a note, so the order is total and a repeat is two equal
/// notes, which the snapshot refuses. Refused too when the geometry does not fit the
/// topology or holds a non-finite value (D9), z column included: it is a column like
/// any other, checked under `node.z`.
pub fn snapshot(topology: &Topology, mut geometry: Geometry) -> Result<Snapshot, StageError> {
    let dim = geometry.dim();
    let node_ids = (0..topology.node_count()).map(|i| topology.node(i).id);
    let edge_ids = (0..topology.edge_count()).map(|e| topology.edge(e).id);
    let parts = SnapshotParts {
        // The label follows the geometry, not the crate: a 2D layout's snapshot is
        // labelled 0.3 and a 3D one 0.4, so no 2D byte moves when a 3D layout arrives.
        // One call site of `label_for`, never `CURRENT_VERSION`
        // (`docs/decisions/contract-3d-verdict.md` condition 1).
        version: label_for(dim),
        node_ids: StringTable::from_strs("node.id", node_ids).map_err(StageError::Snapshot)?,
        edge_ids: StringTable::from_strs("edge.id", edge_ids).map_err(StageError::Snapshot)?,
        source: topology.edges().source.clone(),
        target: topology.edges().target.clone(),
        nodes: geometry.nodes,
        // The layout's z column, passed through as it is: its presence is what `dim`
        // above was read from, so the header's `dim` and the bytes cannot disagree.
        z: geometry.z,
        edges: geometry.edges,
        notes: {
            geometry.notes.sort();
            Notes::of(&geometry.notes)
        },
    };
    Snapshot::new(parts).map_err(StageError::Snapshot)
}
