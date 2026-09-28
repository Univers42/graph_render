//! Planar graphs: [`planar_embedding`] must return `Some` with a structurally valid,
//! certified embedding for every one of them.

use super::checks::assert_valid_embedding;
use super::graphs::{
    complete_graph, cycle, grid, maximal_planar, outerplanar_fan, path, star, wheel,
};
use crate::layout::planarity::planar_embedding;

fn check(n: u32, edges: &[(u32, u32)]) {
    let embedding = planar_embedding(n, edges)
        .unwrap_or_else(|| panic!("expected planar: n={n} edges={edges:?}"));
    assert_valid_embedding(n, edges, &embedding);
}

#[test]
fn the_empty_graph_and_a_single_node_are_planar() {
    check(0, &[]);
    check(1, &[]);
}

#[test]
fn several_isolated_nodes_are_planar_together() {
    check(4, &[]);
}

#[test]
fn isolated_nodes_do_not_break_the_certificate_next_to_an_edge() {
    // One triangle plus two isolated nodes.
    check(5, &[(0, 1), (1, 2), (0, 2)]);
}

#[test]
fn a_disconnected_planar_graph_is_still_planar() {
    // Two disjoint triangles.
    check(6, &[(0, 1), (1, 2), (0, 2), (3, 4), (4, 5), (3, 5)]);
}

#[test]
fn k4_is_planar_and_already_edge_maximal() {
    check(4, &complete_graph(4));
}

#[test]
fn a_path_is_planar() {
    check(20, &path(20));
}

#[test]
fn a_star_is_planar() {
    check(15, &star(15));
}

#[test]
fn cycles_are_planar() {
    for n in [3, 4, 5, 10, 50] {
        check(n, &cycle(n));
    }
}

#[test]
fn grids_are_planar() {
    for (rows, cols) in [(2, 2), (3, 3), (4, 7), (1, 9)] {
        let (n, edges) = grid(rows, cols);
        check(n, &edges);
    }
}

#[test]
fn wheels_are_planar() {
    for rim in [3, 4, 8, 30] {
        let (n, edges) = wheel(rim);
        check(n, &edges);
    }
}

#[test]
fn maximal_outerplanar_fans_are_planar() {
    for n in [4, 5, 10, 40] {
        check(n, &outerplanar_fan(n));
    }
}

#[test]
fn seeded_maximal_planar_graphs_are_planar_and_edge_maximal() {
    for seed in 0..12u32 {
        let n = 6 + seed * 7 % 90;
        let edges = maximal_planar(seed, n);
        assert_eq!(
            edges.len() as u32,
            3 * n - 6,
            "an Apollonian network is edge-maximal"
        );
        check(n, &edges);
    }
}
