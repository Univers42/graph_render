//! Undirected neighbour lists in networkx's own order, for the layouts that walk a graph
//! the way SciGraphs does (`bipartite`).
//!
//! SciGraphs builds an `nx.Graph` from the edge list in edge order, so a node's
//! neighbours come out in the order its edges were first added, each neighbour once
//! (`G.neighbors`); a self-loop lists the node itself. Direction is ignored.

use crate::index::Topology;

/// `lists[v]` is `v`'s distinct neighbours in first-edge order, `O(n + m)`.
pub(super) fn neighbours(topology: &Topology) -> Vec<Vec<u32>> {
    let columns = topology.edges();
    let mut lists = vec![Vec::new(); topology.node_count() as usize];
    for (&a, &b) in columns.source.iter().zip(&columns.target) {
        lists[a as usize].push(b);
        if a != b {
            lists[b as usize].push(a);
        }
    }
    let mut seen_by = vec![u32::MAX; lists.len()];
    for (node, list) in lists.iter_mut().enumerate() {
        list.retain(|&other| {
            let fresh = seen_by[other as usize] != node as u32;
            seen_by[other as usize] = node as u32;
            fresh
        });
    }
    lists
}
