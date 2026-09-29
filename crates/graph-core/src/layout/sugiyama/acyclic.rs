//! Cycle breaking: the greedy feedback-arc-set heuristic of Eades, Lin & Smyth, "A fast and effective heuristic for the feedback arc set problem" (1993). Reference: `SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:244-296`.
//!
//! Peels sinks to the right, sources to the left, otherwise the vertex of largest out-degree minus in-degree (a max-heap on that key, ties broken by dense index — node listing order is already `0..n`, so no separate rank array is needed the way the reference's `enumerate(nodes)` builds one). Every edge whose source sorts after its target is reversed, not dropped, so no edge is lost (`prompt.md` §11: FAS is greedy, not minimum).
//!
//! Ponytail: a heuristic, not a minimum feedback-arc-set solver. Failing input: a graph whose minimum feedback arc set this local peeling order cannot reach (worst case, an adversarial tournament). Direction: more edges reversed than strictly necessary — cosmetic (`dag.edge_reversed` notes each one), never a wrong graph, since a reversed edge is drawn head to tail, not dropped.

use crate::index::Topology;
use core::cmp::Reverse;
use graph_contract::notes::{Note, NoteCode};
use std::collections::BinaryHeap;

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
    /// Breaks every cycle in `topology`, recording which edges were reversed.
    pub(crate) fn of(topology: &Topology) -> Self {
        let (succ, pred) = unique_neighbours(topology);
        let order = greedy_fas_order(&succ, &pred);
        let mut rank = vec![0u32; topology.node_count() as usize];
        for (position, &node) in (0u32..).zip(&order) {
            rank[node as usize] = position;
        }
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

/// Distinct out/in neighbours per node, self-loops excluded, ascending: the heuristic's
/// degree counts are over neighbours, not incident edges (`G.successors(n)` on a
/// multigraph yields each neighbour once).
fn unique_neighbours(topology: &Topology) -> (Vec<Vec<u32>>, Vec<Vec<u32>>) {
    let n = topology.node_count() as usize;
    let cols = topology.edges();
    let (mut succ, mut pred) = (vec![Vec::new(); n], vec![Vec::new(); n]);
    for (&s, &t) in cols.source.iter().zip(&cols.target) {
        if s != t {
            succ[s as usize].push(t);
            pred[t as usize].push(s);
        }
    }
    for row in succ.iter_mut().chain(pred.iter_mut()) {
        row.sort_unstable();
        row.dedup();
    }
    (succ, pred)
}

/// `in_degree - out_degree` and the node itself: the heap's total order, node index the
/// tie-break.
fn heap_key(in_degree: &[u32], out_degree: &[u32], v: u32) -> (i64, u32) {
    let (i, o) = (in_degree[v as usize], out_degree[v as usize]);
    (i64::from(i) - i64::from(o), v)
}

/// Mutable peeling state shared by [`greedy_fas_order`]'s two admission paths (a ready
/// sink/source, or the heap's best remaining vertex).
struct Peeling<'a> {
    succ: &'a [Vec<u32>],
    pred: &'a [Vec<u32>],
    alive: Vec<bool>,
    out_degree: Vec<u32>,
    in_degree: Vec<u32>,
    ready: Vec<u32>,
    heap: BinaryHeap<Reverse<(i64, u32)>>,
}

impl<'a> Peeling<'a> {
    fn new(succ: &'a [Vec<u32>], pred: &'a [Vec<u32>]) -> Self {
        let out_degree: Vec<u32> = succ.iter().map(|s| s.len() as u32).collect();
        let in_degree: Vec<u32> = pred.iter().map(|p| p.len() as u32).collect();
        let ready = (0..succ.len() as u32)
            .filter(|&v| out_degree[v as usize] == 0 || in_degree[v as usize] == 0)
            .collect();
        let heap = (0..succ.len() as u32)
            .map(|v| Reverse(heap_key(&in_degree, &out_degree, v)))
            .collect();
        Self {
            succ,
            pred,
            alive: vec![true; succ.len()],
            out_degree,
            in_degree,
            ready,
            heap,
        }
    }

    /// Removes `node`, decrementing its still-alive neighbours' degrees, queuing any
    /// that newly became a sink or source and re-keying every touched one in the heap.
    fn peel(&mut self, node: u32) {
        self.alive[node as usize] = false;
        for &v in &self.succ[node as usize] {
            if self.alive[v as usize] {
                self.in_degree[v as usize] -= 1;
                if self.in_degree[v as usize] == 0 {
                    self.ready.push(v);
                }
                self.heap
                    .push(Reverse(heap_key(&self.in_degree, &self.out_degree, v)));
            }
        }
        for &u in &self.pred[node as usize] {
            if self.alive[u as usize] {
                self.out_degree[u as usize] -= 1;
                if self.out_degree[u as usize] == 0 {
                    self.ready.push(u);
                }
                self.heap
                    .push(Reverse(heap_key(&self.in_degree, &self.out_degree, u)));
            }
        }
    }
}

/// The greedy feedback-arc-set vertex order: sources first, then the peeled-by-key
/// middle, then sinks (reference `_greedy_fas_order`, `hierarchical.py:244-296`).
fn greedy_fas_order(succ: &[Vec<u32>], pred: &[Vec<u32>]) -> Vec<u32> {
    let mut state = Peeling::new(succ, pred);
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let mut remaining = succ.len() as u32;
    while remaining > 0 {
        while let Some(node) = state.ready.pop() {
            if !state.alive[node as usize] {
                continue;
            }
            if state.out_degree[node as usize] == 0 {
                right.push(node);
            } else {
                left.push(node);
            }
            state.peel(node);
            remaining -= 1;
        }
        if remaining == 0 {
            break;
        }
        while let Some(Reverse((key, node))) = state.heap.pop() {
            let current = heap_key(&state.in_degree, &state.out_degree, node).0;
            if state.alive[node as usize] && key == current {
                left.push(node);
                state.peel(node);
                remaining -= 1;
                break;
            }
        }
    }
    right.reverse();
    left.extend(right);
    left
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::records::build::{edge, node};

    fn topology(nodes: &[&str], edges: &[(&str, &str, &str)]) -> Topology {
        let n: Vec<_> = nodes.iter().map(|id| node(id, "")).collect();
        let e: Vec<_> = edges.iter().map(|&(id, s, t)| edge(id, s, t)).collect();
        index_model(&n, &e).expect("fits")
    }

    #[test]
    fn a_chain_reverses_nothing() {
        let t = topology(&["a", "b", "c"], &[("ab", "a", "b"), ("bc", "b", "c")]);
        let acyclic = Acyclic::of(&t);
        assert_eq!(acyclic.reversed, [false, false]);
        assert!(acyclic.notes.is_empty());
        assert_eq!(acyclic.arc(&t, 0), (0, 1));
        assert_eq!(acyclic.arc(&t, 1), (1, 2));
    }

    #[test]
    fn a_cycle_is_broken_by_reversing_one_edge_and_noted() {
        // a -> b -> c -> a: a 3-cycle, every node in/out degree 1, tied. The peel loop
        // has no ready (sink/source) node, so the heap picks the lowest dense index
        // among the (in-out=0) ties: node 0 becomes a source, breaking the cycle at its
        // incoming edge c->a.
        let t = topology(
            &["a", "b", "c"],
            &[("ab", "a", "b"), ("bc", "b", "c"), ("ca", "c", "a")],
        );
        let acyclic = Acyclic::of(&t);
        assert_eq!(acyclic.reversed, [false, false, true], "c->a reversed");
        assert_eq!(
            acyclic.notes,
            [Note {
                code: NoteCode::EdgeReversed,
                index: 2
            }]
        );
        // Every arc now points from a lower rank to a higher one, and repeating the
        // build gives the same result: determinism.
        for e in 0..3 {
            let (tail, head) = acyclic.arc(&t, e);
            assert!(
                acyclic.rank[tail as usize] < acyclic.rank[head as usize],
                "e{e}"
            );
        }
        assert_eq!(Acyclic::of(&t), acyclic);
    }

    #[test]
    fn a_self_loop_is_excluded_and_never_reversed() {
        let t = topology(&["a"], &[("aa", "a", "a")]);
        let acyclic = Acyclic::of(&t);
        assert_eq!(acyclic.reversed, [false]);
        assert!(acyclic.notes.is_empty());
    }

    #[test]
    fn an_empty_topology_produces_empty_acyclic_state() {
        let t = index_model(&[], &[]).expect("fits");
        let acyclic = Acyclic::of(&t);
        assert!(acyclic.rank.is_empty() && acyclic.reversed.is_empty());
    }
}
