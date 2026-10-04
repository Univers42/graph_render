//! The first `keep` nodes, and only the edges with both endpoints among them. Node order is
//! the model's own, so the kept prefix keeps the dense rows `index_model` gave it, and that
//! is what lets the carry map ids across the two topologies.
//!
//! One copy, in [`crate::bench::tick`], because two measurements need it and a second copy is
//! a second thing to keep in step: `--grow` carries a session across a re-index of a prefix of
//! the model, and `--stream`'s line 0 is the same prefix as a document.
//!
//! Caveat: the ids are compared through a [`HashSet`], so the edges come out in the model's own
//! order (the filter preserves it) but the *set* is unordered and nothing may ever be read out
//! of it. `prefix` returns two `Vec`s in model order, which is the only order anything downstream
//! reads them in.

use graph_core::{EdgeRecord, NodeRecord};
use std::collections::HashSet;

/// `keep` nodes and the edges with both endpoints among them, in the model's own order.
pub fn prefix(
    nodes: &[NodeRecord],
    edges: &[EdgeRecord],
    keep: u32,
) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    let kept: Vec<NodeRecord> = nodes.iter().take(keep as usize).cloned().collect();
    let ids: HashSet<&str> = kept.iter().map(|n| n.id.as_str()).collect();
    let edges: Vec<EdgeRecord> = edges
        .iter()
        .filter(|e| ids.contains(e.source.as_str()) && ids.contains(e.target.as_str()))
        .cloned()
        .collect();
    (kept, edges)
}
