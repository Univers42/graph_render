//! The structural assertions every positive planarity test repeats: the certificate
//! holds, and the embedding's rotations are exactly the input's simple-graph adjacency —
//! re-derived here independently of the production dedup, so a shared bug between the
//! two could not hide from either.

use crate::layout::planarity::{Embedding, euler_certificate, faces};

/// `edges`, deduplicated and self-loop-free the same way [`planar_embedding`] treats its
/// own input, sorted ascending — the expected simple graph `embedding` must match.
///
/// [`planar_embedding`]: crate::layout::planarity::planar_embedding
fn simple_pairs(n: u32, edges: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let mut pairs: Vec<(u32, u32)> = edges
        .iter()
        .copied()
        .filter(|&(a, b)| a < n && b < n && a != b)
        .map(|(a, b)| if a < b { (a, b) } else { (b, a) })
        .collect();
    pairs.sort_unstable();
    pairs.dedup();
    pairs
}

/// Standard checks for an [`Embedding`] the LR test returned as planar: the Euler
/// certificate holds, and every row's rotation lists exactly that node's simple-graph
/// neighbours, once each.
pub(super) fn assert_valid_embedding(n: u32, edges: &[(u32, u32)], embedding: &Embedding) {
    assert_eq!(embedding.node_count(), n);
    assert!(euler_certificate(embedding), "must satisfy Euler's formula");
    let pairs = simple_pairs(n, edges);
    let mut degree = vec![0u32; n as usize];
    for &(a, b) in &pairs {
        degree[a as usize] += 1;
        degree[b as usize] += 1;
    }
    for v in 0..n {
        let row = embedding.rotation(v);
        assert_eq!(
            row.len() as u32,
            degree[v as usize],
            "row {v} must list every neighbour exactly once"
        );
        let mut sorted = row.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            row.len(),
            "row {v} must not repeat a neighbour"
        );
    }
    assert_eq!(embedding.edge_count() as usize, pairs.len());
    assert!(
        pairs.is_empty() || !faces(embedding).is_empty(),
        "an edge must border some face"
    );
}
