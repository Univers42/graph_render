//! The neighbour order the reference walks, and the components it lays out one at a time.
//!
//! **This order is part of the drawing.** `lib/twopigen/circle.c` sweeps
//! `agfstedge`/`agnxtedge` five times — the leaf test, the breadth-first search, the leaf
//! walk, the span sweep and the angle sweep — and the angle sweep accumulates
//! `theta += SPAN(child)` across that sequence, so a graph drawn with the wrong order is a
//! *different drawing*, not the same one rounded differently.
//!
//! **The order is by the other endpoint's id, not by edge creation.** It is the one place
//! this port is easiest to get wrong, and the reason is cgraph's dictionary comparator
//! rather than the sweep itself. `agfstout`/`agnxtout` and `agfstin`/`agnxtin` walk
//! `g->e_seq`, ordered by `agedgeseqcmpf` (`lib/cgraph/edge.c:388-400`), and that
//! comparator reads **`e->node` first** — which is the *head* for an out-half
//! (`newedge`, `edge.c:196-216`: `in->node = t; out->node = h`) and the *tail* for an
//! in-half. So a node's out-edges come out ordered by the head's id and its in-edges by the
//! tail's id, each tie broken by the edge's own sequence number. Sorting by edge index
//! instead agrees only when a node's other endpoints happen to be in creation order.
//!
//! Everything else follows from `agfstedge` (`edge.c:89-116`): the out-edges first and only
//! then the in-edges, with a self-loop yielded once (it is an out-edge) and skipped on the
//! way in, which is what `agnxtedge`'s `while (rv && rv->node == n)` filter does.
//!
//! [`Neighbours`] builds that order once and is the only place it exists; the layout's
//! five sweeps all read it, so none of them can disagree with another.

use crate::csr::Csr;
use crate::index::Topology;

/// A node's neighbours, in the reference's order.
pub(super) struct Neighbours {
    out: Csr,
    inbound: Csr,
}

impl Neighbours {
    /// The order for every node: out-edges by head id, then in-edges by tail id.
    pub(super) fn of(topology: &Topology) -> Self {
        let count = topology.node_count();
        let edges = topology.edges();
        let mut out: Vec<(u32, u32)> = edges
            .source
            .iter()
            .zip(edges.target.iter())
            .map(|(&source, &target)| (source, target))
            .collect();
        let mut inbound: Vec<(u32, u32)> = edges
            .target
            .iter()
            .zip(edges.source.iter())
            .map(|(&target, &source)| (target, source))
            .collect();
        // Lexicographic, so each row arrives sorted by its value and `Csr::from_pairs`
        // keeps that arrival order; a stable sort leaves parallel edges in creation
        // order, which is the comparator's secondary key.
        out.sort_unstable();
        inbound.sort_unstable();
        Self {
            out: Csr::from_pairs(count, out.into_iter()).expect("node rows are in range"),
            inbound: Csr::from_pairs(count, inbound.into_iter()).expect("node rows in range"),
        }
    }

    /// How many node rows there are: one slot per node, since the dense index is the
    /// node id here.
    pub(super) fn rows(&self) -> usize {
        self.out.rows() as usize
    }

    /// Visits `node`'s neighbours: its out-edges by head, then its in-edges by tail.
    ///
    /// A self-loop is an out-edge and so is yielded once; the in-edge copy is the same
    /// loop seen from the other end and is dropped, as `agnxtedge` drops it.
    pub(super) fn for_each(&self, node: u32, mut visit: impl FnMut(u32)) {
        for &other in self.out.row(node) {
            visit(other);
        }
        for &other in self.inbound.row(node) {
            if other != node {
                visit(other);
            }
        }
    }
}

/// The connected components, each **ascending by dense index**, the first taking the lowest
/// unvisited node.
///
/// Ascending because the reference scans `agfstnode` — creation order, which the harness's
/// dense `n0..n{n-1}` DOT names make identical to the dense index — and the centre is the
/// *first* node reaching the maximum `nStepsToLeaf` (`circle.c:107-113`, strictly greater).
/// A breadth-first component order would pick a different tie.
pub(super) fn components(neighbours: &Neighbours, count: u32) -> Vec<Vec<u32>> {
    let mut seen = vec![false; count as usize];
    let mut out: Vec<Vec<u32>> = Vec::new();
    for start in 0..count {
        if seen[start as usize] {
            continue;
        }
        let mut component = vec![start];
        flood(neighbours, start, &mut seen, &mut component);
        component.sort_unstable();
        out.push(component);
    }
    out
}

/// Breadth-first flood from `start` into `component`, which is both the queue and the
/// answer: `at` walks it and each popped node contributes its unvisited neighbours.
fn flood(neighbours: &Neighbours, start: u32, seen: &mut [bool], component: &mut Vec<u32>) {
    seen[start as usize] = true;
    let mut at = 0;
    while at < component.len() {
        let node = component[at];
        at += 1;
        let mut found = Vec::new();
        neighbours.for_each(node, |other| {
            if !seen[other as usize] {
                seen[other as usize] = true;
                found.push(other);
            }
        });
        component.extend(found);
    }
}
