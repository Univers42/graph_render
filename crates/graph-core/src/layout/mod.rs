//! LAYOUT (`prompt.md` §3): the pluggable seam, topology in, geometry out. Every layout
//! is a [`crate::stage::Stage`] and is listed in [`crate::registry`]. Phase 2 has one,
//! the grid; its job is to prove the pipeline, not to be interesting. Phase 3 adds the
//! tidy tree, treemap, circular and circle-packing layouts; [`hierarchy`] is the one
//! repaired tree the tree layouts share.

pub mod circle_packing;
pub mod circular;
pub mod force;
pub mod forceatlas2;
pub mod grid;
pub mod hierarchy;
pub mod planarity;
pub mod tidy_tree;
pub mod treemap;

use crate::index::Topology;
use crate::stage::StageError;
use graph_contract::binary::{Snapshot, SnapshotParts, StringTable};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use graph_contract::notes::{Note, Notes};
use graph_contract::version::CURRENT_VERSION;

/// What a layout stage produces: one geometry for the whole snapshot, and what it
/// repaired or approximated on the way.
#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    /// Every node's geometry, in the topology's node order.
    pub nodes: NodeGeometry,
    /// Every edge's geometry, in the topology's edge order.
    pub edges: EdgeGeometry,
    /// The stage's notes, in any order: [`snapshot`] sorts them.
    pub notes: Vec<Note>,
}

/// The snapshot of `geometry` laid over `topology`: the topology's stable ids and edge
/// endpoints in its own order, then the geometry, then the notes sorted by
/// `(code, index)` — the whole of a note, so the order is total and a repeat is two equal
/// notes, which the snapshot refuses. Refused too when the geometry does not fit the
/// topology or holds a non-finite value (D9).
pub fn snapshot(topology: &Topology, mut geometry: Geometry) -> Result<Snapshot, StageError> {
    let node_ids = (0..topology.node_count()).map(|i| topology.node(i).id);
    let edge_ids = (0..topology.edge_count()).map(|e| topology.edge(e).id);
    let parts = SnapshotParts {
        version: CURRENT_VERSION,
        node_ids: StringTable::from_strs("node.id", node_ids).map_err(StageError::Snapshot)?,
        edge_ids: StringTable::from_strs("edge.id", edge_ids).map_err(StageError::Snapshot)?,
        source: topology.edges().source.clone(),
        target: topology.edges().target.clone(),
        nodes: geometry.nodes,
        edges: geometry.edges,
        notes: {
            geometry.notes.sort();
            Notes::of(&geometry.notes)
        },
    };
    Snapshot::new(parts).map_err(StageError::Snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::records::build::{edge, node};
    use graph_contract::notes::{NoteCode, Notes, SNAPSHOT_WIDE};
    use graph_contract::snapshot::SnapshotError;

    fn topology() -> Topology {
        let nodes = [node("a", ""), node("b", ""), node("a", "dup")];
        let edges = [
            edge("e1", "b", "a"),
            edge("e2", "a", "zz"),
            edge("e3", "a", "a"),
        ];
        index_model(&nodes, &edges).expect("fits")
    }

    fn points(n: usize) -> Geometry {
        Geometry {
            nodes: NodeGeometry::Point {
                x: vec![0.0; n],
                y: vec![1.0; n],
            },
            edges: EdgeGeometry::Line,
            notes: Vec::new(),
        }
    }

    #[test]
    fn the_snapshot_carries_the_topologys_ids_and_endpoints_in_its_order() {
        let snapshot = snapshot(&topology(), points(2)).expect("fits");
        let p = snapshot.parts();
        assert_eq!(p.version, CURRENT_VERSION);
        assert_eq!(p.node_ids.iter().collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(p.edge_ids.iter().collect::<Vec<_>>(), ["e1", "e3"]);
        assert_eq!((&p.source[..], &p.target[..]), (&[1, 0][..], &[0, 0][..]));
        assert_eq!(p.nodes, points(2).nodes);
    }

    #[test]
    fn a_stages_notes_reach_the_snapshot_sorted_and_a_repeat_is_refused() {
        let note = |code, index| Note { code, index };
        let mut geometry = points(2);
        geometry.notes = vec![
            note(NoteCode::PackingApproximate, SNAPSHOT_WIDE),
            note(NoteCode::CycleEdgeDropped, 1),
            note(NoteCode::ExtraParentDropped, 0),
            note(NoteCode::CycleEdgeDropped, 0),
        ];
        let s = snapshot(&topology(), geometry.clone()).expect("fits");
        let want = Notes {
            code: vec![1, 1, 2, 3],
            index: vec![0, 1, 0, SNAPSHOT_WIDE],
        };
        assert_eq!(s.parts().notes, want, "sorted by (code, index)");
        geometry.notes.push(note(NoteCode::CycleEdgeDropped, 1));
        let repeat = SnapshotError::NoteOrder { index: 2 };
        assert_eq!(
            snapshot(&topology(), geometry),
            Err(StageError::Snapshot(repeat)),
            "(code, index) is the whole note: a tie is a repeat, and refused"
        );
    }

    #[test]
    fn geometry_that_does_not_fit_the_topology_is_refused() {
        let short = snapshot(&topology(), points(1)).expect_err("one node short");
        assert!(matches!(
            short,
            StageError::Snapshot(SnapshotError::Length {
                column: "node.x",
                ..
            })
        ));
        let mut nan = points(2);
        nan.nodes = NodeGeometry::Point {
            x: vec![0.0, f32::NAN],
            y: vec![0.0; 2],
        };
        let err = snapshot(&topology(), nan).expect_err("NaN");
        assert_eq!(
            err,
            StageError::Snapshot(SnapshotError::NonFinite {
                column: "node.x",
                index: 1
            })
        );
    }
}
