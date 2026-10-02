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
use super::{slack, Ctx};

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
            assert_eq!(root(&mut parent, n as usize), first, "node {n} unspanned {when}");
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
fn check_cut_values(g: &Fast, nodes: &[u32], ctx: &Ctx, when: &str) {
    for &edge in &ctx.tree_edge {
        assert_eq!(slack(g, edge), 0, "tree edge {edge} is not tight {when}");
        let side = tail_side(g, edge);
        let mut want = 0;
        for &n in nodes {
            for &f in &g.out[n as usize] {
                let r = &g.edges[f as usize];
                if side[r.tail as usize] && !side[r.head as usize] {
                    want += r.weight;
                }
            }
            for &f in &g.inn[n as usize] {
                let r = &g.edges[f as usize];
                if side[r.head as usize] && !side[r.tail as usize] {
                    want -= r.weight;
                }
            }
        }
        assert_eq!(
            g.edges[edge as usize].cutvalue, want,
            "cut value of {edge} {when}"
        );
    }
}

/// The nodes still reachable from `edge`'s tail once it is removed from the tree: the side
/// whose weights the cut value sums.
fn tail_side(g: &Fast, edge: u32) -> Vec<bool> {
    let mut side = vec![false; g.nodes.len()];
    let tail = g.edges[edge as usize].tail;
    side[tail as usize] = true;
    let mut stack = vec![tail];
    while let Some(n) = stack.pop() {
        for &x in &g.nodes[n as usize].tree_in {
            reach(g, x, edge, &mut side, &mut stack);
        }
        for &x in &g.nodes[n as usize].tree_out {
            reach(g, x, edge, &mut side, &mut stack);
        }
    }
    side
}

/// Claim the far end of tree edge `x`, unless `x` is the edge being cut.
fn reach(g: &Fast, x: u32, cut: u32, side: &mut [bool], stack: &mut Vec<u32>) {
    let w = if g.nodes[g.edges[x as usize].head as usize].tree_out.contains(&x) {
        g.edges[x as usize].head
    } else {
        g.edges[x as usize].tail
    };
    if x != cut && !side[w as usize] {
        side[w as usize] = true;
        stack.push(w);
    }
}