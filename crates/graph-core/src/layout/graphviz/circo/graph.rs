//! The derived graph `circomps` builds, and the per-block induced subgraphs it works on.
//!
//! `circomps` (`circularinit.c:95-158`) does not lay out the graph it is handed. It builds
//! a **strict undirected** derived graph out of it — one node per input node, one edge per
//! distinct non-loop input edge — and everything after that reads the derived graph. Two
//! consequences are load-bearing here:
//!
//! - a **self-loop is not an edge at all** (`if (dt != dh)`, `circularinit.c:119`), so it
//!   reaches no block and no circle;
//! - the two directions of one pair are **one** edge, because `agedge` on a strict graph
//!   returns the edge it already made.
//!
//! **Iteration order is the drawing.** The reference walks `agfstedge`/`agnxtedge`, which
//! is the out-half then the in-half (`edge.c:89-116`), and each half is sorted by *the
//! other endpoint's id* — `agedgeseqcmpf` (`edge.c:388-400`) compares `e->node` first,
//! which is the head for an out-half and the tail for an in-half. Ties break on the edge's
//! own sequence number, and after the dedup above there are none. So a node's neighbour
//! list here is: its out-neighbours ascending, then its in-neighbours ascending. `n0..n{n-1}`
//! in the harness's DOT make that id order the dense index order, which is the only reason
//! a dense index may stand in for an id.

use super::Block;
use crate::index::Topology;
use std::collections::HashMap;

/// The strict undirected derived graph, as two neighbour rows per node.
pub(super) struct Derived {
    /// A node's out-half: the nodes it has an edge to as tail, ascending.
    out: Vec<Vec<u32>>,
    /// A node's in-half: the nodes with an edge into it, ascending.
    inbound: Vec<Vec<u32>>,
}

impl Derived {
    /// The derived graph of `topology`: self-loops dropped, parallel pairs collapsed, each
    /// row in the reference's order.
    pub(super) fn of(topology: &Topology) -> Self {
        let count = topology.node_count() as usize;
        let columns = topology.edges();
        let mut out: Vec<Vec<u32>> = (0..count).map(|_| Vec::new()).collect();
        for (&source, &target) in columns.source.iter().zip(columns.target.iter()) {
            if source != target {
                out[source as usize].push(target);
            }
        }
        for row in &mut out {
            row.sort_unstable();
            row.dedup();
        }
        let mut inbound: Vec<Vec<u32>> = (0..count).map(|_| Vec::new()).collect();
        for (tail, row) in out.iter().enumerate() {
            for &head in row {
                inbound[head as usize].push(tail as u32);
            }
        }
        Self { out, inbound }
    }

    /// `node`'s neighbours in `agfstedge` order: its out-half, then its in-half.
    pub(super) fn neighbours(&self, node: u32) -> Vec<u32> {
        let node = node as usize;
        let mut all = Vec::with_capacity(self.out[node].len() + self.inbound[node].len());
        all.extend_from_slice(&self.out[node]);
        all.extend_from_slice(&self.inbound[node]);
        all
    }
}

/// One block's induced subgraph — the reference's `block_t::sub_graph` — with a mutable
/// `EDGEORDER` per edge (`blockpath.c` resets it at every crossing count).
////// Edges are numbered once, in ascending `(tail, head)` pair order, and every walk below goes
/// through a node's own [`BlockGraph::row`], so there is no map here to iterate and no order a
/// hash map could impose on the output (`prompt.md` §6 D2).
pub(super) struct BlockGraph {
    /// The block's nodes in `agsubnode` order — the order `addNode` inserted them, which
    /// is *not* ascending, and which `place_residual_nodes` and `remove_pair_edges` walk.
    pub(super) nodes: Vec<u32>,
    ends: Vec<(u32, u32)>,
    rows: Vec<Vec<u32>>,
    /// `EDGEORDER`, zero = unset. **Only the reference's own walk still writes it**, which
    /// is now a `#[cfg(test)]` function kept as the oracle for [`crate::layout::graphviz::circo::crossings::Counter`];
    /// the sweep that replaced it needs no scratch on the graph at all, so this is gated with it.
    /// One `i32` per edge, not two: the reference's two `Agedge_t` per undirected edge share one
    /// attribute record, so both images read and write the same `EDGEORDER` here too.
    #[cfg(test)]
    order: Vec<i32>,
}

impl BlockGraph {
    /// `block`'s induced subgraph: every derived edge with both endpoints in the block,
    /// oriented as the derived graph orients it.
    pub(super) fn of(derived: &Derived, block: &Block) -> Self {
        let nodes = block.nodes.clone();
        let local: HashMap<u32, u32> = nodes
            .iter()
            .enumerate()
            .map(|(at, &node)| (node, at as u32))
            .collect();
        let mut ends = Vec::new();
        for (at, &node) in nodes.iter().enumerate() {
            for &other in derived.out[node as usize].iter() {
                let Some(head) = local.get(&other) else {
                    continue;
                };
                ends.push((at as u32, *head));
            }
        }
        ends.sort_unstable();
        let mut rows: Vec<Vec<u32>> = (0..nodes.len()).map(|_| Vec::new()).collect();
        for (id, &(tail, head)) in ends.iter().enumerate() {
            rows[tail as usize].push(id as u32);
            rows[head as usize].push(id as u32);
        }
        // Each half is ordered by the other endpoint and the out-half comes first. A
        // self-loop cannot be here — the derived graph has none — so no id is in a row twice.
        for (node, row) in rows.iter_mut().enumerate() {
            row.sort_by_key(|&id| {
                let (tail, head) = ends[id as usize];
                let out_half = head != node as u32;
                (u8::from(!out_half), if out_half { head } else { tail })
            });
        }
        Self {
            nodes,
            #[cfg(test)]
            order: vec![0; ends.len()],
            ends,
            rows,
        }
    }

    /// Every edge as `(tail, head)`, ascending, which is the order they are numbered in.
    pub(super) fn ends(&self) -> &[(u32, u32)] {
        &self.ends
    }

    /// `node`'s own edge ids in `agfstedge` order.
    pub(super) fn row(&self, node: u32) -> &[u32] {
        &self.rows[node as usize]
    }

    /// `node`'s **out**-half edge ids, `agfstout` order.
    pub(super) fn out_row(&self, node: u32) -> Vec<u32> {
        self.rows[node as usize]
            .iter()
            .copied()
            .filter(|&id| self.ends[id as usize].0 == node)
            .collect()
    }

    /// `node`'s **in**-half edge ids, `agfstin` order.
    pub(super) fn in_row(&self, node: u32) -> Vec<u32> {
        self.rows[node as usize]
            .iter()
            .copied()
            .filter(|&id| self.ends[id as usize].1 == node)
            .collect()
    }

    /// The endpoint of `edge` that is not `node`.
    pub(super) fn other(&self, edge: u32, node: u32) -> u32 {
        let (tail, head) = self.ends[edge as usize];
        if head == node { tail } else { head }
    }

    /// `aghead` of `edge`.
    pub(super) fn head(&self, edge: u32) -> u32 {
        self.ends[edge as usize].1
    }

    /// `agtail` of `edge`.
    pub(super) fn tail(&self, edge: u32) -> u32 {
        self.ends[edge as usize].0
    }

    /// `EDGEORDER(edge)`.
    #[cfg(test)]
    pub(super) fn order(&self, edge: u32) -> i32 {
        self.order[edge as usize]
    }

    /// `EDGEORDER(edge) = value`.
    #[cfg(test)]
    pub(super) fn set_order(&mut self, edge: u32, value: i32) {
        self.order[edge as usize] = value;
    }

    /// Clears every `EDGEORDER`, as `count_all_crossings` does before its sweep.
    #[cfg(test)]
    pub(super) fn clear_orders(&mut self) {
        self.order.iter_mut().for_each(|slot| *slot = 0);
    }
}
