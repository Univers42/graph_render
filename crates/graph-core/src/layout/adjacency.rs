//! Undirected neighbour lists in networkx's own order, for the layouts that walk a graph
//! the way SciGraphs does (`bipartite`).
//!
//! SciGraphs builds an `nx.Graph` from the edge list in edge order, so a node's
//! neighbours come out in the order its edges were first added, each neighbour once
//! (`G.neighbors`); a self-loop lists the node itself. Direction is ignored.

use crate::index::Topology;

/// `lists[v]` is `v`'s distinct neighbours in first-edge order, `O(n + m)`.
///
/// **The asymptotic form hides where the cost goes, so here it is.** One `Vec` header per
/// node — 24 bytes on every target — before a single neighbour is written, so `n` nodes and
/// no edges pay `24n` for an empty answer. The rows then grow by doubling, so the peak is
/// under `2m` `u32` slots before the dedup pass, which walks all of them again. The flat
/// CSR form (`crate::csr`) is the one that matches the declared cost; it is not used here
/// because its rows are `&[u32]` slices and this returns `Vec<Vec<u32>>`, which the only
/// caller (`bipartite::partition`) hands straight to a walk. Switching is a change of this
/// function's return type **and** of `bipartite.rs`, so it is recorded here rather than made
/// silently inside a layout that only wants a neighbour list.
///
/// Ponytail: the dedup marker below is `u64` rather than `u32` — one word per node, `8n`
/// bytes — so that the sentinel cannot alias a dense node index, which is the same width.
pub(super) fn neighbours(topology: &Topology) -> Vec<Vec<u32>> {
    let columns = topology.edges();
    let mut lists = vec![Vec::new(); topology.node_count() as usize];
    for (&a, &b) in columns.source.iter().zip(&columns.target) {
        lists[a as usize].push(b);
        if a != b {
            lists[b as usize].push(a);
        }
    }
    let mut seen_by = vec![u64::MAX; lists.len()];
    for (node, list) in lists.iter_mut().enumerate() {
        let owner = node as u64;
        list.retain(|&other| {
            let fresh = seen_by[other as usize] != owner;
            seen_by[other as usize] = owner;
            fresh
        });
    }
    lists
}

#[cfg(test)]
mod tests {
    use super::neighbours;
    use crate::layout::coords::probe::graph;

    /// First-edge order, each neighbour once, a self-loop listing the node itself — the
    /// three properties `G.neighbors` has and the dedup pass is there to keep. The pin
    /// matters because the marker the pass uses is a sentinel: an alias between it and a
    /// node index would drop a neighbour without any of this changing shape.
    #[test]
    fn neighbours_are_in_first_edge_order_deduped_with_self_loops_kept() {
        let topology = graph(3, &[(0, 1), (0, 1), (0, 0), (1, 2)]);
        assert_eq!(neighbours(&topology), vec![vec![1, 0], vec![0, 2], vec![1]]);
        assert_eq!(
            neighbours(&graph(3, &[(2, 1), (0, 2)])),
            vec![vec![2], vec![2], vec![1, 0]]
        );
        assert_eq!(neighbours(&graph(0, &[])), Vec::<Vec<u32>>::new());
    }
}
