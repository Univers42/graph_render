//! The greedy feedback-arc-set order: `ArcOrder::Feedback`, the branch of
//! `_acyclic_arcs` (`hierarchical.py:304`) the reference takes only for a `nx.DiGraph`.
//!
//! **Not what the pipeline runs.** `scigraphs_core/mesh/layouts/common.py:238` builds an
//! `nx.Graph()` — undirected — for every layout, so `apply_graph_layout` never hands the
//! sugiyama pipeline a directed graph and `_greedy_fas_order` is unreachable from the
//! reference. It is ported here rather than deleted because it is the reference's other
//! branch, its tie-break is what the determinism rules require, and a port that deletes a
//! branch of the function it ports is not a port. `super::ArcOrder::Feedback` is the seam and
//! `super::ArcOrder::NodeIndex` is the order the pipeline takes; the difference between the
//! two on the smallest graph where they disagree is pinned by
//! `the_greedy_feedback_order_is_the_other_branch_and_reverses_more`.
//!
//! Peels sinks to the right, sources to the left, otherwise the vertex of largest out-degree
//! minus in-degree (a max-heap on that key, ties broken by dense index — the node listing
//! order is already `0..n`, so no separate rank array is needed the way the reference's
//! `enumerate(nodes)` builds one).

use crate::index::Topology;
use core::cmp::Reverse;
use std::collections::BinaryHeap;

/// Each node's rank in the greedy order, dense-indexed.
pub(super) fn rank(topology: &Topology) -> Vec<u32> {
    let (succ, pred) = unique_neighbours(topology);
    let order = greedy_fas_order(&succ, &pred);
    let mut rank = vec![0u32; topology.node_count() as usize];
    for (position, &node) in (0u32..).zip(&order) {
        rank[node as usize] = position;
    }
    rank
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
