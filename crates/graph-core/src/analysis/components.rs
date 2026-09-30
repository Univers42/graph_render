//! Connected components (`prompts/phase-07-analysis.md` step 2): weak, direction
//! ignored, and strong, direction respected for directed input. Exact — no `Ponytail`
//! owed.
//!
//! Component ids are assigned in **ascending dense-index-of-first-member order**, not
//! discovery order (which depends on where a traversal or a union-find union happened to
//! start): [`canonicalize`] renumbers by scanning nodes `0..n` and handing out the next
//! id the first time each raw label is seen.

use crate::csr_petgraph::{CsrDigraph, NodeIx};
use crate::index::Topology;
use petgraph::unionfind::UnionFind;

/// [`weak`]'s analysis id, the one graph-wasm registers it under and the hash gate
/// hashes it as.
///
/// A constant per analysis rather than one `ID` per module because this module holds two:
/// a caller asking for `analysis.components.weak` is asking for *this* function, so the
/// string lives beside it the way `layout::tidy_tree::ID` does, and both the registry row
/// and the gate's stage list read it from here rather than spelling it again.
pub const WEAK: &str = "analysis.components.weak";

/// [`strong`]'s analysis id, [`WEAK`]'s way.
pub const STRONG: &str = "analysis.components.strong";

/// Weakly connected components: an edge counts regardless of `directed`. Built with
/// petgraph's own [`UnionFind`] (`prompts/phase-07-analysis.md`: "reuse before
/// implement") over the raw edge columns — petgraph's own [`petgraph::algo::
/// connected_components`] returns only a count, not a labeling, so it backs this as a
/// debug cross-check rather than the computation itself.
pub fn weak(topology: &Topology) -> Vec<u32> {
    let n = topology.node_count() as usize;
    let mut sets = UnionFind::<u32>::new(n);
    let edges = topology.edges();
    for i in 0..topology.edge_count() as usize {
        sets.union(edges.source[i], edges.target[i]);
    }
    let raw: Vec<u32> = (0..n as u32).map(|v| sets.find(v)).collect();
    let labels = canonicalize(&raw);
    let components = labels.iter().copied().max().map_or(0, |m| m + 1);
    debug_assert_eq!(
        petgraph::algo::connected_components(CsrDigraph::new(topology)),
        components as usize,
        "hand labeling and petgraph's own count must agree"
    );
    labels
}

/// Strongly connected components: an edge counts only in its own direction (an
/// undirected edge, both). Tarjan's algorithm, reused from petgraph over
/// [`CsrDigraph`]; the canonicalisation is ours, same as [`weak`].
pub fn strong(topology: &Topology) -> Vec<u32> {
    let n = topology.node_count() as usize;
    let sccs = petgraph::algo::tarjan_scc(CsrDigraph::new(topology));
    let mut raw = vec![0u32; n];
    for (id, members) in (0u32..).zip(&sccs) {
        for &NodeIx(v) in members {
            raw[v as usize] = id;
        }
    }
    canonicalize(&raw)
}

/// Renumbers arbitrary same/different labels, each `< labels.len()`, into
/// `0..k` in the order their first member is seen scanning index `0..n` ascending.
/// `pub(crate)`: `communities::louvain` reuses it for the same reason.
pub(crate) fn canonicalize(raw: &[u32]) -> Vec<u32> {
    let mut canonical = vec![u32::MAX; raw.len()];
    let mut next = 0u32;
    raw.iter()
        .map(|&label| {
            let slot = &mut canonical[label as usize];
            if *slot == u32::MAX {
                *slot = next;
                next += 1;
            }
            *slot
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::records::build::{edge, node};

    #[test]
    fn weak_components_ignore_direction_and_number_by_first_member() {
        let nodes = [node("a", ""), node("b", ""), node("c", ""), node("d", "")];
        let mut a_to_b = edge("ab", "a", "b");
        a_to_b.directed = true; // a -> b only, but weak still joins them
        let edges = [a_to_b, edge("cd", "c", "d")];
        let t = index_model(&nodes, &edges).expect("fits");
        assert_eq!(weak(&t), [0, 0, 1, 1]);
    }

    #[test]
    fn strong_components_split_a_directed_edge_the_reverse_cannot_cross() {
        let nodes = [node("a", ""), node("b", "")];
        let mut a_to_b = edge("ab", "a", "b");
        a_to_b.directed = true;
        let t = index_model(&nodes, &[a_to_b]).expect("fits");
        assert_eq!(strong(&t), [0, 1], "no path back from b to a");
        let t2 = index_model(&nodes, &[edge("ab", "a", "b")]).expect("fits"); // undirected
        assert_eq!(strong(&t2), [0, 0], "undirected: each reaches the other");
    }

    #[test]
    fn an_isolated_node_is_its_own_component_of_both_kinds() {
        let nodes = [node("a", ""), node("b", ""), node("z", "")];
        let t = index_model(&nodes, &[edge("ab", "a", "b")]).expect("fits");
        assert_eq!(weak(&t), [0, 0, 1]);
        assert_eq!(strong(&t), [0, 0, 1]);
    }

    #[test]
    fn repeated_runs_agree_bit_for_bit() {
        let nodes = [node("a", ""), node("b", ""), node("c", "")];
        let edges = [edge("ab", "a", "b"), edge("bc", "b", "c")];
        let t = index_model(&nodes, &edges).expect("fits");
        assert_eq!(weak(&t), weak(&t));
        assert_eq!(strong(&t), strong(&t));
    }
}
