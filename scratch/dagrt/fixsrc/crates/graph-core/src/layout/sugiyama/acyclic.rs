//! Cycle breaking: every non-loop edge oriented forward along a vertex sequence, so the
//! graph the rest of the pipeline sees is a DAG that still holds all of them. Reference:
//! `SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:298-311` for the arcs and
//! `:244-296` for the greedy order.
//!
//! **The sequence is `list(G.nodes())`, not the greedy feedback-arc-set order.** `_acyclic_arcs`
//! reads `_greedy_fas_order(G) if G.is_directed() else list(G.nodes())`, and
//! `scigraphs_core/mesh/layouts/common.py:238` builds `nx.Graph()` — undirected — for every
//! layout, so the greedy branch is unreachable from the reference as it is built. See
//! [`ArcOrder`]. The greedy order is ported anyway, because it is the reference's other
//! branch and its tie-break is what D4 requires; `ArcOrder::Feedback` is the seam.
//!
//! [`ArcOrder::NodeIndex`] still breaks every cycle: it is a total order, so an edge whose
//! source sorts after its target is reversed rather than dropped, and `dag.edge_reversed`
//! notes each one (`prompt.md` §11). It simply breaks *more* of them than a greedy peel
//! would, which is cosmetic — a reversed edge is drawn head to tail, not lost.
//!
//! Ponytail: cycle breaking here is a heuristic, not a minimum feedback-arc-set solver.
//! Failing input: a graph whose minimum feedback arc set the chosen order cannot reach (worst
//! case, an adversarial tournament). Direction: more edges reversed than strictly necessary
//! — cosmetic (`dag.edge_reversed` notes each one), never a wrong graph, since a reversed
//! edge is drawn head to tail, not dropped.

use crate::index::Topology;
use graph_contract::notes::{Note, NoteCode};
use std::ops::Range;

/// Every edge's orientation after cycle breaking: `reversed[e]` for non-loop edge `e`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Acyclic {
    /// The feedback-arc-set order's rank of each node: `rank[v] < rank[w]` means `v`
    /// sorts before `w`. A self-loop is exempt (it has no arc to orient).
    rank: Vec<u32>,
    /// Whether edge `e` was drawn head to tail to keep the graph acyclic; `false` for a
    /// self-loop (which is never oriented).
    pub(crate) reversed: Vec<bool>,
    /// One `EdgeReversed` note per reversed edge, ascending by edge index already (built
    /// by a single ascending scan).
    pub(crate) notes: Vec<Note>,
}

impl Acyclic {
    /// Orients every non-loop edge of `topology` forward, recording which edges were
    /// reversed to get there.
    pub(crate) fn of(topology: &Topology) -> Self {
        Self::oriented(topology, ArcOrder::NodeIndex)
    }

    /// Orients every non-loop edge forward along `order`'s vertex sequence: `rank[v] <
    /// rank[w]` is what makes the edge `v -> w` a forward arc. Every edge whose source
    /// sorts after its target is reversed, not dropped, so no edge is lost.
    pub(crate) fn oriented(topology: &Topology, order: ArcOrder) -> Self {
        let rank = order.rank(topology);
        let cols = topology.edges();
        let mut reversed = vec![false; cols.source.len()];
        let mut notes = Vec::new();
        for (e, rev) in reversed.iter_mut().enumerate() {
            let (s, t) = (cols.source[e], cols.target[e]);
            if s != t && rank[s as usize] > rank[t as usize] {
                *rev = true;
                notes.push(Note {
                    code: NoteCode::EdgeReversed,
                    index: e as u32,
                });
            }
        }
        Self {
            rank,
            reversed,
            notes,
        }
    }

    /// Edge `e`'s acyclic-oriented endpoints, `(tail, head)`: always `rank[tail] <
    /// rank[head]`. Meaningless for a self-loop; callers check that first.
    pub(crate) fn arc(&self, topology: &Topology, e: u32) -> (u32, u32) {
        let cols = topology.edges();
        let (s, t) = (cols.source[e as usize], cols.target[e as usize]);
        if self.reversed[e as usize] {
            (t, s)
        } else {
            (s, t)
        }
    }
}

/// One arc: `(tail, head)` and the half-open range of **its own edges' slots** in
/// [`ArcList::members`] — never a range of edge indices, which two parallel edges that are
/// not adjacent in the input cannot spell (see [`ArcList::members`]).
pub(crate) type Arc = (u32, u32, Range<u32>);

/// The arcs of one graph's ordering graph, plus the counts the layering stage needs beside
/// them. Built once by [`Arcs::grouped`] and threaded through the whole layering phase, so
/// the sort it costs is paid once.
pub(crate) struct ArcList {
    /// One entry per distinct non-loop pair, ascending by `(tail, head)`.
    pub(crate) arcs: Vec<Arc>,
    /// The edge ids of every arc, in arc order: `members[arc.2]` is that arc's members and
    /// nothing else. This is the sort scratch [`Arcs::grouped`] already built, kept instead
    /// of a `Range<u32>` over edge indices, because an arc's members need not be adjacent
    /// in the input — `k` parallel edges with other pairs between them are one arc whose
    /// "range" would cover every edge in between, and `layering.rs` writes a `Route` per
    /// index it covers.
    pub(crate) members: Vec<u32>,
    /// Nodes in the graph the arcs came from.
    pub(crate) nodes: u32,
    /// Edges in that graph, including the self-loops and the parallel repeats the arcs
    /// coalesce: [`Route`](super::layering::Route) is indexed by edge, not by arc.
    pub(crate) edges: u32,
}

impl ArcList {
    /// The `(tail, head)` of every arc, without the edge ranges: the shape the stage dump
    /// records, since the reference's `_acyclic_arcs` returns pairs.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn pairs(&self) -> Vec<(u32, u32)> {
        self.arcs
            .iter()
            .map(|&(tail, head, _)| (tail, head))
            .collect()
    }
}

/// `topology` and the [`Acyclic`] orientation it was built from, bundled so downstream
/// stages take one context parameter instead of the pair everywhere.
pub(crate) struct Arcs<'a> {
    topology: &'a Topology,
    acyclic: &'a Acyclic,
}

impl<'a> Arcs<'a> {
    /// Bundles `topology` with its already-computed [`Acyclic`] orientation.
    pub(crate) fn new(topology: &'a Topology, acyclic: &'a Acyclic) -> Self {
        Self { topology, acyclic }
    }

    /// The reference's own arcs, grouped, with the node and edge counts the layering stage
    /// needs beside them: one entry per distinct non-loop `(tail, head)` pair, ascending,
    /// each with the half-open range of its own members' slots in [`ArcList::members`]
    /// (`hierarchical.py:306-311`). One entry per pair because `arcs` there is a `set`, so
    /// `k` parallel edges between two nodes are one arc and route through one dummy chain,
    /// not `k`.
    ///
    /// **This is a second view, not a replacement.** [`Self::edge_count`] and
    /// [`Self::tail_head`] stay per-edge, because `Route` and the `dag.edge_reversed` notes
    /// are indexed by edge and every parallel edge is still drawn — through the one chain
    /// this grouping gives them. What the ordering graph is built over is these arcs, which
    /// is what the reference builds it over.
    ///
    /// **One sort, and it is the pipeline's only one.** Every non-loop edge becomes a
    /// `(tail, head, edge)` triple, sorted once; equal pairs are then adjacent, so a run of
    /// them is one arc. `layered()` builds the list once and threads it down, so the whole
    /// layering phase costs one `O(m log m)` sort rather than one per callee.
    ///
    /// **A group is a member list, never an edge-index range.** Two copies of `b -> f` at
    /// edge 0 and edge 5 are one arc whose members are `0` and `5`, not everything between:
    /// `Range<u32>` over edge indices says `0..6` and hands six edges this arc's
    /// `Route` (`layering.rs`). So the triples' edge column is kept, in arc order, as
    /// [`ArcList::members`], and each arc's third field indexes that.
    ///
    /// **The arcs are ordered by node index, not by rank** — `_acyclic_arcs` sorts by
    /// `(rank[a[0]], rank[a[1]])` (`:311`). The two agree exactly when `rank == index`, which
    /// is what `ArcOrder::NodeIndex` gives and what the fixture contract makes the reference's
    /// own case; under `ArcOrder::Feedback` this is the one place the port is not verbatim,
    /// and it is confined to that branch, which `Acyclic::of` does not take.
    pub(crate) fn grouped(&self) -> ArcList {
        let mut pairs: Vec<(u32, u32, u32)> = (0..self.edge_count())
            .filter(|&e| !self.is_loop(e))
            .map(|e| {
                let (tail, head) = self.tail_head(e);
                (tail, head, e)
            })
            .collect();
        // Ascending by `(tail, head, edge)`, so a pair's edges are one ascending run and the
        // first of them leads it — and the list comes out ascending by `(tail, head)`, as
        // `sorted(arcs, key=...)` does on the reference side.
        pairs.sort_unstable();
        let mut arcs: Vec<Arc> = Vec::new();
        let mut members: Vec<u32> = Vec::with_capacity(pairs.len());
        for &(tail, head, edge) in &pairs {
            let slot = members.len() as u32;
            match arcs.last_mut() {
                Some((t, h, span)) if *t == tail && *h == head => span.end = slot + 1,
                _ => arcs.push((tail, head, slot..slot + 1)),
            }
            members.push(edge);
        }
        ArcList {
            arcs,
            members,
            nodes: self.node_count(),
            edges: self.edge_count(),
        }
    }

    /// Nodes in `topology`.
    pub(crate) fn node_count(&self) -> u32 {
        self.topology.node_count()
    }

    /// Edges in `topology`.
    pub(crate) fn edge_count(&self) -> u32 {
        self.topology.edge_count()
    }

    /// Whether edge `e` is a self-loop: never part of layering or routing.
    pub(crate) fn is_loop(&self, e: u32) -> bool {
        let cols = self.topology.edges();
        cols.source[e as usize] == cols.target[e as usize]
    }

    /// Edge `e`'s acyclic-oriented endpoints. See [`Acyclic::arc`].
    pub(crate) fn tail_head(&self, e: u32) -> (u32, u32) {
        self.acyclic.arc(self.topology, e)
    }
}

/// The vertex sequence the arcs are oriented along, as `hierarchical.py:304` writes it:
/// `_acyclic_arcs` picks one of two orders and takes the arcs forward along whichever it
/// picked.
///
/// **The reference takes [`ArcOrder::NodeIndex`] for every graph this pipeline is compared
/// against.** `_acyclic_arcs` reads
/// `_greedy_fas_order(G) if G.is_directed() else list(G.nodes())`, and
/// `scigraphs_core/mesh/layouts/common.py:238` builds `nx.Graph()` — undirected — for every
/// layout, so the graph `apply_graph_layout` hands the sugiyama pipeline never reaches the
/// greedy branch. `list(G.nodes())` is the node listing order, which under the conformance
/// fixture contract (`conformance/fixtures.rs`) is ascending dense index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArcOrder {
    /// The greedy feedback-arc-set order of Eades, Lin & Smyth (1993): the reference's
    /// `G.is_directed()` branch, unreachable from the reference as it is built, and the one
    /// [`feedback`] implements. Constructed only by
    /// `the_greedy_feedback_order_is_the_other_branch_and_reverses_more`, which is what keeps
    /// the port and its tie-break honest rather than deleted.
    ///
    /// **One thing is not verbatim under this order:** [`Arcs::grouped`] sorts the arc list
    /// by node index, where `_acyclic_arcs` sorts it by rank, and the two differ whenever
    /// rank != index. That changes which arc each pair's dummy chain belongs to, not how many
    /// there are, and it is confined to this branch — `Acyclic::of` takes `NodeIndex`, where
    /// the sort is exactly the reference's.
    #[cfg_attr(not(test), allow(dead_code))]
    Feedback,
    /// `list(G.nodes())`: dense index. Acyclic by construction, so nothing is ever
    /// reversed on a graph whose edges already point forward.
    NodeIndex,
}

impl ArcOrder {
    /// Each node's rank in this order, dense-indexed.
    pub(crate) fn rank(self, topology: &Topology) -> Vec<u32> {
        match self {
            Self::NodeIndex => (0..topology.node_count()).collect(),
            Self::Feedback => feedback::rank(topology),
        }
    }
}

/// The greedy feedback-arc-set order itself — the one branch of `_acyclic_arcs` the
/// reference does not reach.
mod feedback;

#[cfg(test)]
mod tests;
