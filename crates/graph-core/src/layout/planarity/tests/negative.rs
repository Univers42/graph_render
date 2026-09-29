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

/// K6 is the one negative here that never reaches the LR test at all: 15 edges against
/// the `3 * 6 - 6 = 12` bound, so `too_dense` refuses it outright.
///
/// The name used to promise the edge bound was what rejected it while asserting only
/// `is_none()` — which a broken `too_dense` would still satisfy, by letting the DFS run
/// and reject K6 the hard way. So the mechanism itself is asserted here, directly on
/// `too_dense`; the `is_none()` is kept as the negative control on the public entry
/// point.
#[test]
fn k6_is_rejected_by_the_edge_bound_before_the_dfs_even_runs() {
    let edges = complete_graph(6);
    assert_eq!(edges.len(), 15, "K6 has 15 edges");
    assert!(
        super::super::lr::too_dense(6, 15),
        "15 > 3 * 6 - 6 = 12, so the bound alone refuses this graph"
    );
    assert!(planar_embedding(6, &edges).is_none());
}

/// The other side of the same bound, so the `too_dense` assertion above is a claim about a
/// threshold rather than a constant that rejects everything: K3,3 has 9 edges against a
/// `3 * 6 - 6 = 12` bound, so `too_dense` lets it through and the **LR test itself** has
/// to reject it. (K5 is not usable as the contrast — at 10 edges it is over its own
/// `3 * 5 - 6 = 9` bound, so the edge bound catches that one too and the DFS never sees
/// it.)
#[test]
fn k33_is_under_the_edge_bound_and_must_be_rejected_by_the_test_itself() {
    let edges = complete_bipartite(3, 3);
    assert_eq!(edges.len(), 9, "K3,3 has 9 edges");
    assert!(!super::super::lr::too_dense(6, 9), "9 < 3 * 6 - 6 = 12");
    assert!(planar_embedding(6, &edges).is_none());
}
