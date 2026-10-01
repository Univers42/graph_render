//! The skeleton: `remove_pair_edges` (`blockpath.c:182-222`).
//!
//! Before a block can be drawn on a circle it needs an *order*, and the reference builds one in
//! three passes: thin the block to a skeleton, span it, and read its long path off the spanning
//! tree. This is the first — delete every edge that joins two neighbours of a node, working
//! from the lowest-degree node upwards, which is what keeps the circle order from snaking
//! through a clique. [`tree`] is the other two.
//!
//! Everything here works in **block-local** node indices (`0..block.nodes.len()`) so the data
//! structures stay small. The reference's one pointer comparison,
//! `(uintptr_t)n1 < (uintptr_t)n2` (`blockpath.c:124`), is an allocation-order comparison, and
//! the derived graph allocates its nodes in ascending id order, which is the local index order.

mod tree;

use super::graph::BlockGraph;
use tree::{longest_path, spanning_tree};

/// The block's nodes in the order the circle will carry them, as block-local indices.
pub(super) fn order_of(block: &BlockGraph) -> Vec<u32> {
    let work = remove_pair_edges(Work::new(block));
    let tree = spanning_tree(&work);
    longest_path(&tree)
}

/// `remove_pair_edges`'s mutable working copy: the block's induced subgraph, plus which of
/// its edges survive into the spanning-tree graph.
struct Work {
    /// Edges as `(tail, head)`, block-local. Never renumbered, so `keep` stays a parallel
    /// array and no id is invalidated by an insertion.
    ends: Vec<(u32, u32)>,
    /// Whether the edge is still in the working copy.
    live: Vec<bool>,
    /// Per node, its live edge ids: out-half ascending by head, then in-half ascending by
    /// tail — `agfstedge` order.
    rows: Vec<Vec<u32>>,
    /// `DEGREE(n)`, which the pair-edge pass raises and lowers by hand as it adds and drops
    /// edges, so it is not this graph's degree.
    degree: Vec<i32>,
    /// Whether the edge survives into the spanning-tree graph (`outg`).
    keep: Vec<bool>,
}

impl Work {
    /// The working copy: the block's induced subgraph, every edge live and kept.
    fn new(block: &BlockGraph) -> Self {
        let ends: Vec<(u32, u32)> = block.ends().to_vec();
        let mut rows: Vec<Vec<u32>> = (0..block.nodes.len()).map(|_| Vec::new()).collect();
        for (id, &(tail, head)) in ends.iter().enumerate() {
            rows[tail as usize].push(id as u32);
            rows[head as usize].push(id as u32);
        }
        for (node, row) in rows.iter_mut().enumerate() {
            row.sort_by_key(|&id| half_key(&ends[id as usize], node as u32));
        }
        let degree = rows.iter().map(|row| row.len() as i32).collect();
        Self {
            live: vec![true; ends.len()],
            keep: vec![true; ends.len()],
            ends,
            rows,
            degree,
        }
    }

    /// `agfstedge(g, node)` as `(other, edge)` pairs, live edges only.
    fn row(&self, node: u32) -> Vec<(u32, u32)> {
        self.rows[node as usize]
            .iter()
            .copied()
            .filter(|&id| self.live[id as usize])
            .map(|id| (self.other(id, node), id))
            .collect()
    }

    /// The endpoint of `edge` that is not `node`.
    fn other(&self, edge: u32, node: u32) -> u32 {
        let (tail, head) = self.ends[edge as usize];
        if head == node { tail } else { head }
    }

    /// `agfindedge(g, a, b)`, either orientation.
    fn find(&self, a: u32, b: u32) -> Option<u32> {
        self.rows[a as usize]
            .iter()
            .copied()
            .find(|&id| self.live[id as usize] && self.other(id, a) == b)
    }

    /// The neighbours the **spanning tree** sees: every kept edge, in `agfstedge` order.
    ///
    /// Deliberately not [`Work::row`]. `agdelete(g, currnode)` at the end of a skeleton pass
    /// takes the node and its edges out of the working copy `g`, but `outg` — the graph the
    /// tree is actually spanned over — is a *subgraph* of the block and keeps every node the
    /// block had. So dropping a node here must not drop it from the tree: on a 5-cycle the two
    /// passes delete `n0` and `n4` from the working copy and the tree is still the whole cycle,
    /// which is what makes the block's long path the block itself.
    fn kept_row(&self, node: u32) -> Vec<u32> {
        self.rows[node as usize]
            .iter()
            .copied()
            .filter(|&id| self.keep[id as usize])
            .map(|id| self.other(id, node))
            .collect()
    }

    /// `agedge(g, tail, head, NULL, 1)` plus the reference's own `DEGREE` bumps — which it
    /// makes whether or not the edge was new, so this does too.
    fn link(&mut self, tail: u32, head: u32) {
        if self.find(tail, head).is_none() {
            let id = self.ends.len() as u32;
            self.ends.push((tail, head));
            self.live.push(true);
            self.keep.push(false);
            for node in [tail, head] {
                let key = half_key(&self.ends[id as usize], node);
                let at = self.rows[node as usize]
                    .iter()
                    .position(|&e| half_key(&self.ends[e as usize], node) > key)
                    .unwrap_or(self.rows[node as usize].len());
                self.rows[node as usize].insert(at, id);
            }
        }
        self.degree[tail as usize] += 1;
        self.degree[head as usize] += 1;
    }
}

/// An edge's place in a node's `agfstedge` row: the out-half first, each ascending by the
/// other endpoint.
fn half_key(ends: &(u32, u32), node: u32) -> (u8, u32) {
    let out_half = ends.1 != node;
    (u8::from(!out_half), if out_half { ends.1 } else { ends.0 })
}

/// `remove_pair_edges` (`blockpath.c:182-222`): `nodeCount - 3` times over, take the
/// lowest-degree node still standing, thin its neighbourhood, then drop the node.
fn remove_pair_edges(mut work: Work) -> Work {
    let mut list: Vec<u32> = (0..work.degree.len() as u32).collect();
    sort_by_degree(&mut list, &work);
    for _ in 0..work.degree.len().saturating_sub(3) {
        let Some(current) = list.pop() else {
            break;
        };
        let neighbours = work
            .row(current)
            .into_iter()
            .map(|(other, _)| other)
            .collect::<Vec<_>>();
        for other in neighbours {
            list.retain(|&node| node != other);
        }
        find_pair_edges(&mut work, current);
        for (other, _) in work.row(current) {
            work.degree[other as usize] -= 1;
            list.push(other);
        }
        sort_by_degree(&mut list, &work);
        for id in work.rows[current as usize].clone() {
            work.live[id as usize] = false;
        }
    }
    work
}

/// `LIST_SORT(&dl, cmpDegree)`: descending degree.
///
/// **Ponytail (tie order): this is a stable sort and the reference's is `qsort`**
/// (`lib/util/list.c:363`), which glibc 2.41 does not make stable. Failing input: two nodes of
/// equal degree in a block of four nodes or more, measured, that is 984 of the 1000 gate
/// seeds, and the worst differential gap over them is 6.460e+04 points
/// (`docs/measurements/p13-gv1-circo.md`). Direction: the first tie decides which node is
/// thinned first, so the block's **circle order** differs from Graphviz's: a different
/// drawing of the same blocks at the same radii, not a wrong one. Escape hatch: the fourteen
/// analytically determined closed cases in this module's `tests`, whose blocks are small enough
/// that no tie decides anything, and which agree with Graphviz byte for byte.
fn sort_by_degree(list: &mut [u32], work: &Work) {
    list.sort_by_key(|&node| -work.degree[node as usize]);
}

/// `find_pair_edges` (`blockpath.c:102-177`): drop the edges inside `node`'s neighbourhood,
/// then pair up what is left so the skeleton's degree is unchanged.
fn find_pair_edges(work: &mut Work, node: u32) {
    let (node_degree, mut edge_count) = (work.degree[node as usize], 0);
    let mut with = Vec::new();
    let mut without = Vec::new();
    for (one, edge) in work.row(node) {
        let mut paired = false;
        for (two, other) in work.row(node) {
            if edge != other && work.find(one, two).is_some() {
                paired = true;
                if one < two {
                    edge_count += 1;
                    if let Some(found) = work.find(one, two) {
                        work.keep[found as usize] = false;
                    }
                }
            }
        }
        if paired {
            with.push(one)
        } else {
            without.push(one)
        }
    }
    pair_up(work, &with, &without, node_degree - 1 - edge_count);
}

/// The degree top-up: pair the unpaired neighbours off, two at a time.
fn pair_up(work: &mut Work, with: &[u32], without: &[u32], mut diff: i32) {
    if diff <= 0 {
        return;
    }
    if (diff as usize) < without.len() {
        let mut mark = 0;
        while mark + 1 < without.len() {
            work.link(without[mark], without[mark + 1]);
            diff -= 1;
            mark += 2;
        }
        for mark in 2..without.len() {
            if diff <= 0 {
                return;
            }
            work.link(without[0], without[mark]);
            diff -= 1;
        }
    } else if diff as usize == without.len() {
        // `agedge(g, NULL, hp, NULL, 1)` makes an anonymous node the reference never walks
        // again; only its degree bump on `hp` survives, and that is all this reproduces.
        for &head in without {
            if let Some(&root) = with.first() {
                work.link(root, head);
            }
            work.degree[head as usize] += 1;
        }
    }
}
