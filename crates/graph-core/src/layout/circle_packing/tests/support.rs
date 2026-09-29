//! Small graph builders and the `Topology` they turn into, shared by this module's whole
//! test suite. Nodes are the dense range `0..n`, edges plain `(u32, u32)` pairs — exactly
//! [`super::super::run`]'s own input shape once read off a topology, so no adapter sits
//! between "the graph I mean" and "the graph under test".

use crate::index::{Topology, index_model};
use crate::records::build::{edge, node};

/// Builds a topology of `n` nodes named `n0..n{n-1}` and one undirected `relation` edge
/// per pair in `edges`.
pub(super) fn topology(n: u32, edges: &[(u32, u32)]) -> Topology {
    let ids: Vec<String> = (0..n).map(|i| format!("n{i}")).collect();
    let nodes: Vec<_> = ids.iter().map(|id| node(id, "")).collect();
    let edges: Vec<_> = edges
        .iter()
        .enumerate()
        .map(|(i, &(u, v))| edge(&format!("e{i}"), &ids[u as usize], &ids[v as usize]))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

/// Every distinct pair among `n` nodes: `K_n`.
pub(super) fn complete_graph(n: u32) -> Vec<(u32, u32)> {
    (0..n)
        .flat_map(|i| ((i + 1)..n).map(move |j| (i, j)))
        .collect()
}

/// `K_{a,b}`: `a` nodes `0..a`, `b` nodes `a..a+b`, every cross pair.
pub(super) fn complete_bipartite(a: u32, b: u32) -> Vec<(u32, u32)> {
    (0..a)
        .flat_map(|i| (0..b).map(move |j| (i, a + j)))
        .collect()
}

/// `0 - 1 - ... - (n - 1) - 0`.
pub(super) fn cycle(n: u32) -> Vec<(u32, u32)> {
    (0..n).map(|i| (i, (i + 1) % n)).collect()
}

/// A hub (node `0`) joined to every node of an `rim`-node cycle: `rim + 1` nodes total,
/// maximal planar once triangulated (every rim face is already a triangle).
pub(super) fn wheel(rim: u32) -> (u32, Vec<(u32, u32)>) {
    let mut edges: Vec<(u32, u32)> = (1..=rim).map(|i| (0, i)).collect();
    edges.extend((1..=rim).map(|i| (i, if i == rim { 1 } else { i + 1 })));
    (rim + 1, edges)
}

/// A `rows` by `cols` grid graph, row-major dense index: planar, sparse, no triangles at
/// all before this module's own triangulation adds any.
pub(super) fn grid(rows: u32, cols: u32) -> (u32, Vec<(u32, u32)>) {
    let idx = |r: u32, c: u32| r * cols + c;
    let mut edges = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            if c + 1 < cols {
                edges.push((idx(r, c), idx(r, c + 1)));
            }
            if r + 1 < rows {
                edges.push((idx(r, c), idx(r + 1, c)));
            }
        }
    }
    (rows * cols, edges)
}
