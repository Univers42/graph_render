//! The dense variant of the seeded model: the same nodes and edges, plus Relation edges up
//! to a requested edges-per-node ratio, for the ids whose cost grows with m.

use graph_core::{EdgeKind, EdgeRecord, NodeRecord};

/// Appends Relation edges to `edges` until there are ⌈`ratio`·n⌉ of them. Edge `k` joins
/// node `k mod n` to a node a hashed hop away, so every node gains about the same number of
/// edges and they reach across the whole graph rather than its neighbourhood. Never a
/// self-edge; a ratio at or below the current one appends nothing.
///
/// Caveat: the added edges are uniform at random over pairs, so the dense model has no
/// community structure. A real graph with the same m concentrates its edges, and an id's
/// cost on it can sit on either side of the cost measured on this model.
pub fn densify(nodes: &[NodeRecord], edges: &mut Vec<EdgeRecord>, ratio: f64) {
    let n = nodes.len();
    if n < 2 {
        return;
    }
    // `ratio` is within 0..=64 (the flag's parser) and n ≤ 2^20, so the product fits.
    let wanted = (ratio * n as f64).ceil() as usize;
    edges.reserve(wanted.saturating_sub(edges.len()));
    for k in 0..wanted.saturating_sub(edges.len()) {
        let from = k % n;
        let hop = 1 + (spread(k as u64) % (n as u64 - 1)) as usize;
        edges.push(EdgeRecord {
            id: format!("dense-e-{k}"),
            source: nodes[from].id.clone(),
            target: nodes[(from + hop) % n].id.clone(),
            kind: EdgeKind::Relation,
            label: String::new(),
            strength: 0.5,
            directed: true,
            record_id: None,
            child_first: false,
        });
    }
}

/// Fibonacci hashing: the high half of `k` times 2^64/φ, a cheap well-spread sequence.
fn spread(k: u64) -> u64 {
    k.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32
}
