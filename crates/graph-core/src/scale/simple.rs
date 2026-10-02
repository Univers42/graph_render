//! The undirected, deduplicated, self-loop-free adjacency the simplification passes walk:
//! one row per node, its distinct neighbours in **ascending dense index**.
//!
//! This is the same shape `layout::force`'s own `SimpleGraph` builds (that one is
//! `pub(crate)` inside the layout module, and it carries per-edge link strengths this
//! stage has no use for), rebuilt here at `O(n + m log m)` so the scale stage owns its
//! input instead of reaching into a layout's internals. It is not a second spatial
//! structure and it is not a second model: it is the same convention — parallel edges
//! collapse onto one unordered pair, self-loops drop — stated once more where it is used,
//! and `simplify`'s own tests pin the collapse so the two cannot drift apart silently.

use crate::index::Topology;
use std::collections::BTreeSet;

/// A node's distinct neighbours, ascending, plus the node count.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Simple {
    /// The row offsets, `n + 1` of them. `usize`, not `u32`: `2^31` distinct pairs are
    /// `2^32` neighbour entries (review M34). Never on the wire, so D6 does not apply.
    pub(super) offsets: Vec<usize>,
    /// Every node's neighbours, ascending within a row.
    pub(super) neighbours: Vec<u32>,
}

impl Simple {
    /// Node `v`'s neighbours, ascending.
    pub(super) fn row(&self, v: u32) -> &[u32] {
        &self.neighbours[self.offsets[v as usize]..self.offsets[v as usize + 1]]
    }

    /// Node `v`'s degree: how many distinct neighbours it has.
    pub(super) fn degree(&self, v: u32) -> u32 {
        self.row(v).len() as u32
    }

    /// Every unordered pair once, as `(lower, higher)` in ascending edge order — the
    /// pairs themselves, so a caller can name the edges it removed.
    pub(super) fn pairs(t: &Topology) -> Vec<(u32, u32)> {
        let mut seen = BTreeSet::new();
        let edges = t.edges();
        for e in 0..t.edge_count() as usize {
            let (a, b) = (edges.source[e], edges.target[e]);
            if a != b {
                seen.insert((a.min(b), a.max(b)));
            }
        }
        seen.into_iter().collect()
    }
}

/// `Simple` over `t`: each node's distinct neighbours, ascending.
pub(super) fn build(t: &Topology) -> Simple {
    let n = t.node_count();
    let mut rows: Vec<BTreeSet<u32>> = vec![BTreeSet::new(); n as usize];
    for (a, b) in Simple::pairs(t) {
        rows[a as usize].insert(b);
        rows[b as usize].insert(a);
    }
    let mut offsets = Vec::with_capacity(n as usize + 1);
    let mut neighbours = Vec::new();
    offsets.push(0);
    for row in &rows {
        neighbours.extend(row.iter().copied());
        offsets.push(neighbours.len());
    }
    Simple {
        offsets,
        neighbours,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::records::build::{edge, node};

    #[test]
    fn parallel_edges_collapse_self_loops_drop_and_neighbours_ascend() {
        let nodes = [node("a", "db"), node("b", "db"), node("c", "db")];
        let edges = [
            edge("e0", "a", "b"),
            edge("e1", "b", "a"), // parallel, reversed
            edge("e2", "c", "c"), // self-loop
            edge("e3", "b", "c"),
        ];
        let t = index_model(&nodes, &edges).expect("fits");
        let simple = build(&t);
        assert_eq!(Simple::pairs(&t), vec![(0, 1), (1, 2)]);
        assert_eq!(simple.row(0), &[1]);
        assert_eq!(simple.row(1), &[0, 2]);
        assert_eq!(simple.row(2), &[1]);
        assert_eq!(
            (simple.degree(0), simple.degree(1), simple.degree(2)),
            (1, 2, 1)
        );
    }

    #[test]
    fn an_empty_graph_has_empty_rows_and_a_lone_node_has_none() {
        assert_eq!(
            build(&crate::index::empty_model()),
            Simple {
                offsets: vec![0],
                neighbours: Vec::new()
            }
        );
        assert_eq!(
            Simple::default(),
            Simple {
                offsets: Vec::new(),
                neighbours: Vec::new()
            }
        );
        let t = index_model(&[node("a", "db")], &[]).expect("fits");
        let simple = build(&t);
        assert_eq!(simple.offsets, vec![0, 0]);
        assert_eq!(simple.degree(0), 0);
    }

    /// M34. A row offset holds any length the neighbour column reaches. The edge index
    /// allows up to `u32::MAX - 1` edges (`index/tests.rs:166-175`), so `2^31` distinct
    /// pairs fill `2^32` neighbour entries and a `u32` offset wraps. That input is tens of
    /// GiB, so the test pins the offset's width instead of building it.
    #[test]
    fn a_row_offset_is_as_wide_as_the_neighbour_column_length() {
        let simple = build(&crate::index::empty_model());
        assert_eq!(
            std::mem::size_of_val(&simple.offsets[0]),
            std::mem::size_of::<usize>()
        );
    }

    /// The rows are in dense index order and each row ascending, whatever order the
    /// edges arrived in: a reversal of the edge list cannot change the adjacency.
    #[test]
    fn the_adjacency_does_not_depend_on_the_order_the_edges_arrived_in() {
        let nodes: Vec<_> = (0..6).map(|i| node(&format!("n{i}"), "db")).collect();
        let pairs = [(0, 5), (1, 3), (2, 4), (0, 1), (4, 5)];
        let forward: Vec<_> = pairs
            .iter()
            .enumerate()
            .map(|(i, (a, b))| edge(&format!("e{i}"), &format!("n{a}"), &format!("n{b}")))
            .collect();
        let mut backward = forward.clone();
        backward.reverse();
        let (l, r) = (
            index_model(&nodes, &forward).expect("f"),
            index_model(&nodes, &backward).expect("b"),
        );
        assert_eq!(build(&l), build(&r));
        assert_eq!(build(&l).row(0), &[1, 5]);
    }
}
