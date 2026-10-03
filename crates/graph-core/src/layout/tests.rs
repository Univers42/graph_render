//! The seam's tests: what the topology's ids do to a snapshot, what a geometry that does not
//! fit is refused for, and what a z column changes — under `node.z`, like any other column.

use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};
use graph_contract::canonical_json::{from_json, to_json};
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
    Geometry::planar(
        NodeGeometry::Point {
            x: vec![0.0; n],
            y: vec![1.0; n],
        },
        EdgeGeometry::Line,
        Vec::new(),
    )
}

/// The same points a third of the way up: the fixture a 3D layout would hand over.
fn points3(n: usize) -> Geometry {
    Geometry::in_space(
        NodeGeometry::Point {
            x: vec![0.0; n],
            y: vec![1.0; n],
        },
        EdgeGeometry::Line,
        Vec::new(),
        (0..n).map(|i| i as f32 * 0.5).collect(),
    )
}

#[test]
fn the_snapshot_carries_the_topologys_ids_and_endpoints_in_its_order() {
    let snapshot = snapshot(&topology(), points(2)).expect("fits");
    let p = snapshot.parts();
    // A 2D layout's snapshot is labelled 0.3, not the crate's 0.4: the label is the
    // lowest version that can express the snapshot, and 2D is expressible in 0.3.
    assert_eq!(p.version, label_for(Dim::D2));
    assert_eq!(p.z, None, "and carries no z column");
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

#[test]
fn a_z_column_labels_the_snapshot_0_4_and_round_trips_through_both_faces() {
    let snap = snapshot(&topology(), points3(2)).expect("fits");
    assert_eq!(
        snap.parts().version,
        label_for(Dim::D3),
        "a z column is what lifts the label off 0.3"
    );
    assert_eq!(
        snap.parts().z,
        Some(vec![0.0, 0.5]),
        "passed through as it is"
    );
    assert_eq!(snap.header().dim, Dim::D3);
    let bytes = snap.to_bytes();
    assert_eq!(
        Snapshot::from_bytes(&bytes).expect("reads").parts().z,
        snap.parts().z
    );
    let json = to_json(&snap);
    assert_eq!(from_json(&json).expect("reads").parts().z, snap.parts().z);
}

#[test]
fn a_z_column_that_is_the_wrong_length_is_refused_like_any_other_column() {
    let mut geometry = points3(2);
    geometry.z = Some(vec![0.0]);
    assert_eq!(
        snapshot(&topology(), geometry),
        Err(StageError::Snapshot(SnapshotError::Length {
            column: "node.z",
            expected: 2,
            found: 1,
        }))
    );
}

#[test]
fn a_z_column_that_holds_a_non_finite_value_is_refused() {
    let mut geometry = points3(2);
    geometry.z = Some(vec![0.0, f32::INFINITY]);
    assert_eq!(
        snapshot(&topology(), geometry),
        Err(StageError::Snapshot(SnapshotError::NonFinite {
            column: "node.z",
            index: 1,
        }))
    );
}

#[test]
fn re_edging_carries_the_z_column_and_the_notes_untouched() {
    // The one path a post pass has: it cannot reach z except by handing it on.
    let geometry = points3(2);
    let rebundled = geometry.with_edges(EdgeGeometry::Line);
    assert_eq!(rebundled.z, geometry.z, "z is never dropped here");
    assert_eq!(rebundled.notes, geometry.notes);
    assert_eq!(rebundled.dim(), Dim::D3, "so the label follows");
    assert_eq!(points(2).with_edges(EdgeGeometry::Line).dim(), Dim::D2);
}

#[test]
fn a_0_node_topology_snapshots_as_a_labeled_snapshot_with_empty_id_tables() {
    let empty = index_model(&[], &[]).expect("fits");
    let snap = snapshot(&empty, points(0)).expect("fits");
    let p = snap.parts();
    assert_eq!(p.version, label_for(Dim::D2), "no z column, so 0.3");
    assert_eq!(p.z, None);
    assert_eq!(p.node_ids.iter().collect::<Vec<_>>(), Vec::<&str>::new());
    assert_eq!(p.edge_ids.iter().collect::<Vec<_>>(), Vec::<&str>::new());
}

#[test]
fn a_1_node_topology_carries_exactly_one_node_id() {
    let one = index_model(&[node("a", "")], &[]).expect("fits");
    let snap = snapshot(&one, points(1)).expect("fits");
    let p = snap.parts();
    assert_eq!(p.version, label_for(Dim::D2), "no z column, so 0.3");
    assert_eq!(p.z, None);
    assert_eq!(p.node_ids.iter().collect::<Vec<_>>(), ["a"]);
    assert_eq!(p.edge_ids.iter().collect::<Vec<_>>(), Vec::<&str>::new());
}
