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

    /// `self` with its edge geometry replaced by `edges`, and its nodes, notes and z
    /// column carried through untouched. The one way a post pass rebuilds a geometry, so
    /// a pass cannot drop the z column by forgetting it: there is nowhere else to build
    /// one.
    pub fn with_edges(&self, edges: EdgeGeometry) -> Self {
        Self {
            nodes: self.nodes.clone(),
            edges,
            notes: self.notes.clone(),
            z: self.z.clone(),
        }
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
