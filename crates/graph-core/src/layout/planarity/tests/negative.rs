//! Non-planar graphs: [`planar_embedding`] must return `None` for every one of them —
//! never a wrong embedding.

use super::graphs::{complete_bipartite, complete_graph, subdivide};
use crate::layout::planarity::planar_embedding;

#[test]
fn k5_is_not_planar() {
    assert!(planar_embedding(5, &complete_graph(5)).is_none());
}

#[test]
fn k33_is_not_planar() {
    assert!(planar_embedding(6, &complete_bipartite(3, 3)).is_none());
}

#[test]
fn a_subdivision_of_k5_is_not_planar() {
    let (n, edges) = subdivide(5, &complete_graph(5));
    assert!(planar_embedding(n, &edges).is_none());
}

#[test]
fn a_subdivision_of_k33_is_not_planar() {
    let (n, edges) = subdivide(6, &complete_bipartite(3, 3));
    assert!(planar_embedding(n, &edges).is_none());
}

#[test]
fn k6_is_rejected_by_the_edge_bound_before_the_dfs_even_runs() {
    // 15 edges > 3*6-6 = 12: too_dense rejects it outright, still correctly non-planar.
    assert!(planar_embedding(6, &complete_graph(6)).is_none());
}
