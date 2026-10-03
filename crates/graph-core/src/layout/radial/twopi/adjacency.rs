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
    ///
    /// `O(n + m)`, from the topology's own edge columns — see [`rows`], which is finding
    /// L-08. The form it replaces collected two `Vec<(u32, u32)>` of all `m` edges (16
    /// bytes an edge live at once) and `sort_unstable`d each, which is `O(m log m)` at the
    /// registry's own ceiling.
    pub(super) fn of(topology: &Topology) -> Self {
        let count = topology.node_count();
        let edges = topology.edges();
        Self {
            out: Csr::from_pairs(count, rows(count, &edges.source, &edges.target))
                .expect("node rows are in range"),
            inbound: Csr::from_pairs(count, rows(count, &edges.target, &edges.source))
                .expect("node rows in range"),
        }
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

/// Every edge's `(row, other)` pair, grouped by `row` and, inside a row, ascending by
/// `other` — the order `agedgeseqcmpf` gives the reference's out- and in-edge sweeps.
///
/// Least significant key first: a stable counting sort by `other`, then a stable counting
/// sort by `row`. That is the same `O(n + m)` row mapping `Topology::build_adjacency`
/// (`index.rs:175-185`) builds, plus the ordering on top of it, and it keeps two `m`-long
/// index buffers live rather than the two pair vectors. Parallel edges tie on both keys and
/// so arrive in edge order, which is the comparator's secondary key; their pairs are equal,
/// so nothing downstream can tell.
///
/// **A topology CSR cannot supply that ordering**: `index.rs:177-178` files each row's
/// edges by edge index, and an edge index is not the other endpoint's id, so a row read
/// out of one would be a different drawing. Hence the counting sort rather than a read.
fn rows<'a>(
    count: u32,
    row: &'a [u32],
    other: &'a [u32],
) -> impl Iterator<Item = (u32, u32)> + Clone + 'a {
    let all: Vec<u32> = (0..row.len() as u32).collect();
    let by_other = counting_sort(count, all, |edge| other[edge as usize]);
    let by_row = counting_sort(count, by_other, |edge| row[edge as usize]);
    by_row
        .into_iter()
        .map(move |edge| (row[edge as usize], other[edge as usize]))
}

/// `order` permuted by `key` ascending, stably: two edges with the same key keep the order
/// they had in `order`.
///
/// A counting sort over `0..count`, so `O(n + m)` with one `n`-long cursor and one `m`-long
/// output. The counts cannot overflow `u32`: `Topology`'s own `Csr::from_pairs` sums the
/// same counts with a checked add and refuses a model where they would
/// (`index.rs:177-178`, `csr.rs:39`), so a `Topology` that exists has `m <= u32::MAX`.
fn counting_sort(count: u32, order: Vec<u32>, key: impl Fn(u32) -> u32) -> Vec<u32> {
    let mut slot = vec![0_u32; count as usize + 1];
    for &edge in &order {
        slot[key(edge) as usize + 1] += 1;
    }
    for at in 1..slot.len() {
        slot[at] += slot[at - 1];
    }
    let mut out = vec![0_u32; order.len()];
    for &edge in &order {
        let at = &mut slot[key(edge) as usize];
        out[*at as usize] = edge;
        *at += 1;
    }
    out
}

/// The connected components, each **ascending by dense index**, the first taking the lowest
/// unvisited node.
///
/// Ascending because the reference scans `agfstnode` — creation order, which the harness's
/// dense `n0..n{n-1}` DOT names make identical to the dense index — and the centre is the
/// *first* node reaching the maximum `nStepsToLeaf` (`circle.c:107-113`, strictly greater).
/// A breadth-first component order would pick a different tie.
///
/// Flat, not a `Vec` per component: an edgeless graph has one component per node, and a
/// vector per component is one heap allocation per component — the same shape as finding
/// L-01, half the size.
pub(super) struct Components {
    /// Every node, component after component, each block ascending.
    nodes: Vec<u32>,
    /// Where each component's block starts; one past the last is `nodes.len()`.
    starts: Vec<u32>,
}

impl Components {
    /// Component `at`'s nodes, ascending by dense index.
    fn get(&self, at: usize) -> &[u32] {
        let start = self.starts[at] as usize;
        let end = match self.starts.get(at + 1) {
            Some(&next) => next as usize,
            None => self.nodes.len(),
        };
        &self.nodes[start..end]
    }

    /// Every component in turn, lowest unvisited node first.
    pub(super) fn iter<'a>(&'a self) -> impl Iterator<Item = &'a [u32]> + 'a {
        (0..self.starts.len()).map(move |at| self.get(at))
    }
}

/// The components of the whole graph, each block sorted in place as the flood finishes it.
pub(super) fn components(neighbours: &Neighbours, count: u32) -> Components {
    let mut seen = vec![false; count as usize];
    let mut nodes: Vec<u32> = Vec::new();
    let mut starts: Vec<u32> = Vec::new();
    for start in 0..count {
        if seen[start as usize] {
            continue;
        }
        starts.push(nodes.len() as u32);
        let first = starts.len() - 1;
        flood(neighbours, start, &mut seen, &mut nodes);
        nodes[starts[first] as usize..].sort_unstable();
    }
    Components { nodes, starts }
}

/// Breadth-first flood from `start` into `nodes`' last block, which is both the queue and
/// the answer: `at` walks it and each popped node contributes its unvisited neighbours.
fn flood(neighbours: &Neighbours, start: u32, seen: &mut [bool], nodes: &mut Vec<u32>) {
    seen[start as usize] = true;
    let mut at = nodes.len();
    nodes.push(start);
    while at < nodes.len() {
        let node = nodes[at];
        at += 1;
        let mut found = Vec::new();
        neighbours.for_each(node, |other| {
            if !seen[other as usize] {
                seen[other as usize] = true;
                found.push(other);
            }
        });
        nodes.extend(found);
    }
}
