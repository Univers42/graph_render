//! Small graph builders shared by the planarity test suite. Nodes are always the dense
//! range `0..n`, edges plain `(u32, u32)` pairs — exactly [`planar_embedding`]'s own
//! input shape, so no adapter sits between "the graph I mean" and "the graph I test".
//!
//! [`planar_embedding`]: crate::layout::planarity::planar_embedding

use crate::synthetic::Mulberry32;

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

/// Replaces every edge with a length-2 path through a fresh node: a topological
/// subdivision, planar exactly when the original graph is.
pub(super) fn subdivide(n: u32, edges: &[(u32, u32)]) -> (u32, Vec<(u32, u32)>) {
    let mut next = n;
    let mut out = Vec::with_capacity(edges.len() * 2);
    for &(u, v) in edges {
        out.push((u, next));
        out.push((next, v));
        next += 1;
    }
    (next, out)
}

/// `0 - 1 - ... - (n - 1)`, a tree.
pub(super) fn path(n: u32) -> Vec<(u32, u32)> {
    (0..n.saturating_sub(1)).map(|i| (i, i + 1)).collect()
}

/// Node `0` joined to every other node, a tree.
pub(super) fn star(n: u32) -> Vec<(u32, u32)> {
    (1..n).map(|i| (0, i)).collect()
}

/// `0 - 1 - ... - (n - 1) - 0`.
pub(super) fn cycle(n: u32) -> Vec<(u32, u32)> {
    (0..n).map(|i| (i, (i + 1) % n)).collect()
}

/// A hub (node `0`) joined to every node of an `rim`-node cycle: `rim + 1` nodes total.
pub(super) fn wheel(rim: u32) -> (u32, Vec<(u32, u32)>) {
    let mut edges: Vec<(u32, u32)> = (1..=rim).map(|i| (0, i)).collect();
    edges.extend((1..=rim).map(|i| (i, if i == rim { 1 } else { i + 1 })));
    (rim + 1, edges)
}

/// A `rows` by `cols` grid graph, row-major dense index.
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

/// A fan triangulation of the `n`-cycle from node `0`: maximal outerplanar, every node
/// sits on the one unbounded face.
pub(super) fn outerplanar_fan(n: u32) -> Vec<(u32, u32)> {
    let mut edges = cycle(n);
    edges.extend((2..n.saturating_sub(1)).map(|i| (0, i)));
    edges
}

/// A random maximal planar graph (an Apollonian network): start from a triangle, then
/// `n - 3` times split a uniformly chosen existing triangular face by stacking the next
/// node inside it and joining it to the face's three corners — `3n - 6` edges, every
/// face a triangle, deterministic in `seed` and never a hash order.
pub(super) fn maximal_planar(seed: u32, n: u32) -> Vec<(u32, u32)> {
    if n < 3 {
        return path(n);
    }
    let mut edges = vec![(0, 1), (1, 2), (0, 2)];
    let mut faces = vec![[0u32, 1, 2]];
    let mut rng = Mulberry32::new(seed);
    for new_node in 3..n {
        let [a, b, c] = faces.swap_remove(rng.pick(faces.len()));
        edges.extend([(a, new_node), (b, new_node), (c, new_node)]);
        faces.extend([[a, b, new_node], [b, c, new_node], [a, c, new_node]]);
    }
    edges
}
