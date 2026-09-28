//! The non-planar fallback: `K5` and `K3,3` (Kuratowski's own two minors, the module doc's
//! named failing input) must carry note code 3 and still produce finite, sane `Circle`
//! geometry, never a wrong "exact".

use super::support::{complete_bipartite, complete_graph, topology};
use crate::layout::circle_packing::run;
use graph_contract::geometry::NodeGeometry;
use graph_contract::notes::{Note, NoteCode, SNAPSHOT_WIDE};

fn approximate_note() -> Note {
    Note {
        code: NoteCode::PackingApproximate,
        index: SNAPSHOT_WIDE,
    }
}

fn assert_falls_back(n: u32, edges: &[(u32, u32)]) -> NodeGeometry {
    let geometry = run(&topology(n, edges)).expect("runs");
    assert_eq!(
        geometry.notes,
        vec![approximate_note()],
        "note code 3, exactly once"
    );
    geometry.nodes
}

#[test]
fn k5_is_not_planar_and_falls_back() {
    assert_falls_back(5, &complete_graph(5));
}

#[test]
fn k33_is_not_planar_and_falls_back() {
    assert_falls_back(6, &complete_bipartite(3, 3));
}

#[test]
fn the_fallback_still_produces_finite_non_negative_circle_geometry() {
    let NodeGeometry::Circle { x, y, r } = assert_falls_back(5, &complete_graph(5)) else {
        panic!("circle packing always emits Circle geometry");
    };
    for value in x.iter().chain(&y).chain(&r) {
        assert!(value.is_finite(), "{value}");
    }
    assert!(r.iter().all(|&v| v >= 0.0), "no negative radius");
}

#[test]
fn a_k5_minor_hidden_inside_a_bigger_graph_still_falls_back() {
    // K5 on 0..5, plus two extra nodes hanging off it: still not planar.
    let mut edges = complete_graph(5);
    edges.push((0, 5));
    edges.push((5, 6));
    assert_falls_back(7, &edges);
}
