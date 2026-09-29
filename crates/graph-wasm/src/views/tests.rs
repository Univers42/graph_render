use super::*;
use graph_contract::binary::{SnapshotParts, StringTable};
use graph_contract::geometry::{NodeGeometry, Paths};
use graph_contract::version::CURRENT_VERSION;

fn snapshot(nodes: NodeGeometry, edges: EdgeGeometry, ids: &[&str], edge_ids: &[&str]) -> Snapshot {
    let n = ids.len() as u32;
    let parts = SnapshotParts {
        version: CURRENT_VERSION,
        node_ids: StringTable::from_strs("node.id", ids.iter().copied()).expect("fits"),
        edge_ids: StringTable::from_strs("edge.id", edge_ids.iter().copied()).expect("fits"),
        source: vec![0; edge_ids.len()],
        target: vec![n.saturating_sub(1); edge_ids.len()],
        nodes,
        edges,
        notes: graph_contract::notes::Notes::default(),
    };
    Snapshot::new(parts).expect("valid by construction")
}

fn point(x: Vec<f32>, y: Vec<f32>) -> NodeGeometry {
    NodeGeometry::Point { x, y }
}

#[test]
fn point_nodes_expose_x_and_y_and_nothing_else() {
    let snap = snapshot(
        point(vec![1.0, 2.0], vec![3.0, 4.0]),
        EdgeGeometry::Line,
        &["a", "b"],
        &[],
    );
    assert!(matches!(column(&snap, id::NODE_X), Column::F32(v) if v == [1.0, 2.0]));
    assert!(matches!(column(&snap, id::NODE_Y), Column::F32(v) if v == [3.0, 4.0]));
    for reserved in [id::NODE_R, id::NODE_W, id::NODE_H] {
        assert!(
            matches!(column(&snap, reserved), Column::Absent),
            "id {reserved}"
        );
    }
    assert_eq!(node_kind_tag(&snap), 0, "Point");
}

#[test]
fn circle_nodes_expose_r_and_box_columns_stay_absent() {
    let nodes = NodeGeometry::Circle {
        x: vec![0.0],
        y: vec![0.0],
        r: vec![5.0],
    };
    let snap = snapshot(nodes, EdgeGeometry::Line, &["a"], &[]);
    assert!(matches!(column(&snap, id::NODE_R), Column::F32(v) if v == [5.0]));
    assert!(matches!(column(&snap, id::NODE_W), Column::Absent));
    assert_eq!(node_kind_tag(&snap), 1, "Circle");
}

#[test]
fn box_nodes_expose_w_and_h_and_r_stays_absent() {
    let nodes = NodeGeometry::Box {
        x: vec![0.0],
        y: vec![0.0],
        w: vec![2.0],
        h: vec![3.0],
    };
    let snap = snapshot(nodes, EdgeGeometry::Line, &["a"], &[]);
    assert!(matches!(column(&snap, id::NODE_W), Column::F32(v) if v == [2.0]));
    assert!(matches!(column(&snap, id::NODE_H), Column::F32(v) if v == [3.0]));
    assert!(matches!(column(&snap, id::NODE_R), Column::Absent));
    assert_eq!(node_kind_tag(&snap), 2, "Box");
}

/// C3: "a reserved or absent id returns ptr 0 and len 0 — tell absent apart from empty
/// by geometry kind, tested on an n=0 graph." Circle with 0 nodes: `r` is *present but
/// empty* (an empty slice, not Absent); Point with 0 nodes: `r` is absent regardless.
#[test]
fn on_an_empty_graph_absent_and_empty_are_told_apart_by_geometry_kind_not_by_length() {
    let empty_circle = snapshot(
        NodeGeometry::Circle {
            x: vec![],
            y: vec![],
            r: vec![],
        },
        EdgeGeometry::Line,
        &[],
        &[],
    );
    match column(&empty_circle, id::NODE_R) {
        Column::F32(v) => assert_eq!(v.len(), 0, "present, just empty"),
        Column::Absent => panic!("Circle's r column exists even at n = 0"),
        Column::U32(_) => panic!("wrong element type"),
    }
    assert_eq!(node_kind_tag(&empty_circle), 1, "still Circle at n = 0");

    let empty_point = snapshot(point(vec![], vec![]), EdgeGeometry::Line, &[], &[]);
    assert!(
        matches!(column(&empty_point, id::NODE_R), Column::Absent),
        "Point never has r, whatever n is"
    );
    assert_eq!(node_kind_tag(&empty_point), 0);
}

#[test]
fn edge_columns_are_reserved_for_p3s_notes_and_present_for_polyline_and_curve() {
    for id in [id::NOTE_CODE, id::NOTE_INDEX] {
        let snap = snapshot(point(vec![0.0], vec![0.0]), EdgeGeometry::Line, &["a"], &[]);
        assert!(
            matches!(column(&snap, id), Column::Absent),
            "id {id} is reserved"
        );
    }
    let paths = Paths {
        offsets: vec![0, 2],
        pts: vec![0.1, 0.2, 0.3, 0.4],
    };
    let poly = snapshot(
        point(vec![0.0, 1.0], vec![0.0, 1.0]),
        EdgeGeometry::Polyline(paths.clone()),
        &["a", "b"],
        &["e"],
    );
    assert!(matches!(column(&poly, id::EDGE_OFFSETS), Column::U32(v) if v == [0, 2]));
    assert!(matches!(column(&poly, id::EDGE_PTS), Column::F32(v) if v == [0.1, 0.2, 0.3, 0.4]));
    assert!(matches!(
        column(&poly, id::EDGE_CURVE_DEGREE),
        Column::Absent
    ));
    assert_eq!(edge_kind_tag(&poly), 1, "Polyline");

    let curve = snapshot(
        point(vec![0.0, 1.0], vec![0.0, 1.0]),
        EdgeGeometry::Curve { degree: 3, paths },
        &["a", "b"],
        &["e"],
    );
    assert!(matches!(column(&curve, id::EDGE_CURVE_DEGREE), Column::U32(v) if v == [3]));
    assert_eq!(edge_kind_tag(&curve), 2, "Curve");

    let line = snapshot(point(vec![0.0], vec![0.0]), EdgeGeometry::Line, &["a"], &[]);
    for id in [id::EDGE_OFFSETS, id::EDGE_PTS, id::EDGE_CURVE_DEGREE] {
        assert!(
            matches!(column(&line, id), Column::Absent),
            "Line has no path columns"
        );
    }
}

#[test]
fn edge_source_and_target_are_dense_indices_for_every_edge_kind() {
    let snap = snapshot(
        point(vec![0.0, 1.0], vec![0.0, 1.0]),
        EdgeGeometry::Line,
        &["a", "b"],
        &["e"],
    );
    assert!(matches!(column(&snap, id::EDGE_SOURCE), Column::U32(v) if v == [0]));
    assert!(matches!(column(&snap, id::EDGE_TARGET), Column::U32(v) if v == [1]));
}

#[test]
fn an_id_past_the_allocated_set_is_absent_not_a_panic() {
    let snap = snapshot(point(vec![0.0], vec![0.0]), EdgeGeometry::Line, &["a"], &[]);
    assert!(matches!(column(&snap, 999), Column::Absent));
}

/// `Snapshot::new` itself refuses non-finite geometry (D9), so the only way a *live*
/// `Snapshot` ever holds a NaN is the scenario `has_non_finite` exists for: a caller's
/// typed-array view aliases a column's buffer directly and writes through it after
/// construction. Reproduced here with a raw pointer into the already-valid snapshot's own
/// column, exactly what that aliased view would do — never by trying to construct an
/// invalid one, which the constructor correctly rejects.
fn tamper_f32(values: &[f32], at: usize, to: f32) {
    // SAFETY: `at` is in bounds (checked below) and `values` outlives this write — it is
    // borrowed from a `Snapshot` that is still owned by the caller's local variable, the
    // same lifetime shape a wasm caller's raw-pointer view has over the motor's buffer.
    assert!(at < values.len());
    unsafe {
        std::ptr::write(values.as_ptr().add(at) as *mut f32, to);
    }
}

#[test]
fn non_finite_is_caught_in_node_columns_and_in_edge_points() {
    let clean = snapshot(
        point(vec![0.0, 1.0], vec![2.0, 3.0]),
        EdgeGeometry::Line,
        &["a", "b"],
        &[],
    );
    assert!(!has_non_finite(&clean));

    let tampered_node = snapshot(
        point(vec![0.0, 1.0], vec![2.0, 3.0]),
        EdgeGeometry::Line,
        &["a", "b"],
        &[],
    );
    let Column::F32(x) = column(&tampered_node, id::NODE_X) else {
        panic!("Point nodes expose x");
    };
    tamper_f32(x, 1, f32::NAN);
    assert!(
        has_non_finite(&tampered_node),
        "NaN written through an aliased column view"
    );

    let paths = Paths {
        offsets: vec![0, 1],
        pts: vec![0.0, 0.0],
    };
    let tampered_edge = snapshot(
        point(vec![0.0, 1.0], vec![0.0, 1.0]),
        EdgeGeometry::Polyline(paths),
        &["a", "b"],
        &["e"],
    );
    let Column::F32(pts) = column(&tampered_edge, id::EDGE_PTS) else {
        panic!("Polyline edges expose pts");
    };
    tamper_f32(pts, 0, f32::INFINITY);
    assert!(
        has_non_finite(&tampered_edge),
        "infinite value written through an edge-path view"
    );
}
