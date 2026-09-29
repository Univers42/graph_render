//! The non-planar fallback: `K5` and `K3,3` (Kuratowski's own two minors, the module doc's
//! named failing input) must carry note code 3 and still produce finite, sane `Circle`
//! geometry, never a wrong "exact".

use super::support::{complete_bipartite, complete_graph, cycle, topology};
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

#[test]
fn a_duplicate_edge_does_not_change_the_fallback_packing() {
    // The module doc promises the fallback runs on the same `simple` reduction the exact
    // path does. A duplicate `(0, 1)` — in either orientation, and however many times —
    // must not reach it: `G.degree` on a simple graph counts a pair once, and the
    // relaxation's springs are per *pair*, not per record.
    let plain = run(&topology(5, &complete_graph(5))).expect("runs");
    for extra in [
        vec![(0, 1)],
        vec![(1, 0)],
        vec![(0, 1), (1, 0), (0, 1)],
        vec![(2, 3), (3, 2), (4, 0)],
    ] {
        let mut edges = complete_graph(5);
        edges.extend_from_slice(&extra);
        let messy = run(&topology(5, &edges)).expect("runs");
        assert_eq!(
            messy, plain,
            "duplicates {extra:?} must not reach the fallback"
        );
    }
}

#[test]
fn a_self_loop_does_not_change_the_fallback_packing() {
    // A self-loop is not a spring. `G.degree` on a simple graph counts it as two of that
    // node's incident edges, but SciGraphs' force-directed path builds its spring array
    // with `u != v` filtered out, and this port applies the reduction once, upstream of
    // both paths — so a self-loop must not become a zero-length spring that fights the
    // overlap pass, nor a degree of its own.
    let plain = run(&topology(5, &complete_graph(5))).expect("runs");
    for extra in [vec![(0, 0)], vec![(4, 4), (2, 2)]] {
        let mut edges = complete_graph(5);
        edges.extend_from_slice(&extra);
        let looped = run(&topology(5, &edges)).expect("runs");
        assert_eq!(
            looped, plain,
            "self-loops {extra:?} must not reach the fallback"
        );
    }
}

#[test]
fn a_duplicate_edge_does_not_change_an_exact_packing_either() {
    // The exact path already reduces them inside `planar_embedding`; this pins that the
    // two paths now share one rule, so a graph that is planar *or* not gives the same
    // answer either way.
    let base: Vec<(u32, u32)> = cycle(6);
    let mut messy = base.clone();
    messy.extend_from_slice(&[(0, 1), (5, 0), (0, 1)]);
    messy.push((2, 2));
    let plain = run(&topology(6, &base)).expect("runs");
    let reduced = run(&topology(6, &messy)).expect("runs");
    assert_eq!(plain, reduced);
    assert!(plain.notes.is_empty(), "still the exact path");
}
