//! The simplex's own invariants, checked under `cfg(test)` after every pivot.
//!
//! They live in their own module because they are test-only and because they are the only
//! part of the simplex that is not a port: `check_invariants` re-derives, from scratch, the
//! three properties the incremental bookkeeping is supposed to maintain, and compares.
//!
//! **Why this is here rather than a comment.** A pivot that breaks one of these leaves a
//! ranking that is still *feasible* and still self-consistent, and therefore wrong without
//! being obviously wrong: the objective stops improving and the ranks drift away by millions
//! without any assertion anywhere firing. Two bugs in this pass were exactly that — a union
//! find whose heap permuted the array it indexed into, and an interior-node pivot that moved
//! the wrong side of the cut — and neither showed up in a coordinate, only in the ranks not
//! agreeing with the oracle.
//!
//! The spanning check comes first on purpose: without it the cut-value check is *vacuous*,
//! because a cyclic edge set makes the tail side of every edge "every node", so every cut
//! value reads zero and agrees with itself.

use super::super::fast::Fast;
use super::{Ctx, slack};

/// Every property the pass maintains, checked against a recomputation from scratch.
///
/// The spanning check comes first on purpose: without it the cut-value check is *vacuous*,
/// because a cyclic edge set makes the tail side of every edge "every node", so every cut
/// value reads zero and agrees with itself.
pub fn check(g: &Fast, nodes: &[u32], ctx: &Ctx, when: &str) {
    let mut parent: Vec<usize> = (0..g.nodes.len()).collect();
    assert_eq!(
        ctx.tree_edge.len(),
        nodes.len().saturating_sub(1),
        "tree edge count {when}"
    );
    for &edge in &ctx.tree_edge {
        let t = g.edges[edge as usize].tail as usize;
        let h = g.edges[edge as usize].head as usize;
        let (a, b) = (root(&mut parent, t), root(&mut parent, h));
        assert_ne!(a, b, "tree edge {edge} makes a cycle {when}");
        parent[a] = b;
    }
    if nodes.len() > 1 {
        let first = root(&mut parent, nodes[0] as usize);
        for &n in nodes {
            assert_eq!(
                root(&mut parent, n as usize),
                first,
                "node {n} unspanned {when}"
            );
        }
    }
    check_parent_edges(g, nodes, when);
    check_feasible(g, nodes, when);
    check_cut_values(g, nodes, ctx, when);
}

/// The union-find root, used only to test that the tree is a tree.
fn root(p: &mut [usize], mut at: usize) -> usize {
    while p[at] != at {
        at = p[at];
    }
    at
}

/// Every node's `par` is a tree edge, and it leads one hop nearer the root: `lim` decreases
/// with depth, so a parent is always the endpoint with the larger interval. A stale `par`
/// makes `rerank` walk the wrong set of nodes, which is the one way a pivot can leave every
/// other invariant holding and still be wrong.
fn check_parent_edges(g: &Fast, nodes: &[u32], when: &str) {
    for &n in nodes {
        let Some(parent) = g.nodes[n as usize].par else {
            continue;
        };
        let r = &g.edges[parent as usize];
        let other = if r.head == n { r.tail } else { r.head };
        assert!(
            g.nodes[n as usize].lim < g.nodes[other as usize].lim,
            "lim does not decrease from {other} to {n} ({when})"
        );
    }
}

/// No edge's slack is negative, so the ranking is still a feasible one.
fn check_feasible(g: &Fast, nodes: &[u32], when: &str) {
    for &n in nodes {
        for &f in &g.out[n as usize] {
            assert!(
                slack(g, f) >= 0,
                "edge {f} has negative slack {when}: {} -> {}",
                g.edges[f as usize].tail,
                g.edges[f as usize].head
            );
        }
    }
}

/// Every tree edge is tight, and every cut value equals the one recomputed from scratch:
/// the weight leaving the tail side minus the weight entering it.
///
/// **One traversal of the tree for the whole check, not one per edge.** The cut value of a
/// tree edge is read off the subtree at its *deeper* endpoint — `x_cutval` picks the endpoint
/// whose `par` is the edge, and that is the child — as the outgoing weight of that subtree
/// less the incoming weight, negated when the child is the head rather than the tail. That
/// negation is `x_val`'s `dir`, and dropping it is the one thing that makes this check wrong
/// rather than merely slow.
///
/// This is the same number the loop it replaces summed directly, edge by edge, over a
/// depth-first walk of the tree: an edge internal to the subtree adds its weight to both totals
/// and cancels, one leaving adds, one entering subtracts, and an interior tree edge's own cut
/// value telescopes into the same sum. What changes is that it is `O(n + m)` per check instead
/// of `O(n * (n + m))`, and it still reads nothing but the tree's shape and the edge weights.
///
/// The cost mattered because of **where** this runs. The rank pass checks a 42-node graph; the
/// position pass checks the *auxiliary* graph, which carries one chain dummy per rank an input
/// edge spans, so the hairball's 42 nodes and 178 edges arrive as 3860 nodes and 8170 edges.
/// At the old shape the check cost about 1.2 s per pivot and the position pass took 1757 s on
/// that fixture. Every assertion is unchanged; the whole pass now takes seconds.
fn check_cut_values(g: &Fast, nodes: &[u32], ctx: &Ctx, when: &str) {
    let totals = subtree_totals(g, nodes, ctx);
    for &edge in &ctx.tree_edge {
        assert_eq!(slack(g, edge), 0, "tree edge {edge} is not tight {when}");
        let (t, h) = (g.edges[edge as usize].tail, g.edges[edge as usize].head);
        let child = if totals.low[t as usize] > totals.low[h as usize] { t } else { h };
        let balance = totals.outgoing[child as usize] - totals.incoming[child as usize];
        let want = if child == t { balance } else { -balance };
        assert_eq!(
            i64::from(g.edges[edge as usize].cutvalue), want,
            "cut value of {edge} {when}"
        );
    }
}

/// What [`check_cut_values`] reads every cut value off: where each node sits in the spanning
/// tree, and the edge weight its subtree carries in each direction.
struct SubtreeTotals {
    /// `low[n]`: the node's own pre-order number. Two tree neighbours are a parent and a
    /// child, so the child's `low` is the larger, and that is how the two ends of a tree edge
    /// are told apart.
    low: Vec<usize>,
    /// The subtree's total weight on edges entering it.
    incoming: Vec<i64>,
    /// The subtree's total weight on edges leaving it.
    outgoing: Vec<i64>,
}

/// The per-node edge weight totals, then one pre-order walk of the tree and one reverse pass
/// over it, which is where the subtree totals come from.
fn subtree_totals(g: &Fast, nodes: &[u32], ctx: &Ctx) -> SubtreeTotals {
    let tree = tree_neighbours(g, ctx);
    let (mut incoming, mut outgoing) = edge_weight_totals(g, nodes);
    let (low, order) = preorder(&tree, nodes);
    for &at in order.iter().rev() {
        for &next in &tree[at] {
            if low[next as usize] > low[at] {
                incoming[at] += incoming[next as usize];
                outgoing[at] += outgoing[next as usize];
            }
        }
    }
    SubtreeTotals { low, incoming, outgoing }
}

/// Each node's own incoming and outgoing edge weight. One pass over every edge, in adjacency
/// order; the subtree totals are these, accumulated.
fn edge_weight_totals(g: &Fast, nodes: &[u32]) -> (Vec<i64>, Vec<i64>) {
    let count = g.nodes.len();
    let (mut incoming, mut outgoing) = (vec![0i64; count], vec![0i64; count]);
    for &node in nodes {
        for &edge in &g.out[node as usize] {
            outgoing[node as usize] += i64::from(g.edges[edge as usize].weight);
        }
        for &edge in &g.inn[node as usize] {
            incoming[node as usize] += i64::from(g.edges[edge as usize].weight);
        }
    }
    (incoming, outgoing)
}

/// The spanning tree as an undirected adjacency list, both directions of every tree edge.
fn tree_neighbours(g: &Fast, ctx: &Ctx) -> Vec<Vec<u32>> {
    let mut tree: Vec<Vec<u32>> = vec![Vec::new(); g.nodes.len()];
    for &edge in &ctx.tree_edge {
        let (t, h) = (g.edges[edge as usize].tail, g.edges[edge as usize].head);
        tree[t as usize].push(h);
        tree[h as usize].push(t);
    }
    tree
}

/// A pre-order walk of the spanning tree from its first node, and the order it visited in.
///
/// A depth-first walk with a node marked when it is *pushed*, so a parent is always numbered
/// before its children and the reverse of `order` is a valid bottom-up order.
fn preorder(tree: &[Vec<u32>], nodes: &[u32]) -> (Vec<usize>, Vec<usize>) {
    let mut low = vec![usize::MAX; tree.len()];
    let mut order: Vec<usize> = Vec::with_capacity(nodes.len());
    let mut seen = vec![false; tree.len()];
    let Some(&root) = nodes.first() else {
        return (low, order);
    };
    seen[root as usize] = true;
    let mut stack = vec![root as usize];
    while let Some(at) = stack.pop() {
        low[at] = order.len();
        order.push(at);
        for &next in &tree[at] {
            if !seen[next as usize] {
                seen[next as usize] = true;
                stack.push(next as usize);
            }
        }
    }
    (low, order)
}
