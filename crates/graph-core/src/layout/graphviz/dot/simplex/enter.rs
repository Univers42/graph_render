//! `enter_edge` (`ns.c:282-297`) over `dfs_enter_outedge` / `dfs_enter_inedge`
//! (`ns.c:215-280`): the non-tree edge to bring onto the tree.
//!
//! The pivot loop's leaving edge `e` splits the tree in two. One of its ends — the *down
//! node*, whichever endpoint has the larger `ND_lim`, which is the one nearer the root —
//! roots a search over its own tree subtree for an edge that leaves that subtree: the one
//! that can pay for the slack the leaving edge found. Which of the reference's two searches
//! runs is fixed by the direction that search looks at edges, and it has to match the
//! direction `update` then slides the ranks in.
//!
//! Determinism: the search is a depth-first walk with an explicit stack in the reference's
//! push order — out-edges before tree-in-edges at each node, popped last-in-first-out — and
//! the minimum-slack candidate is the first one found, ties going to the earlier edge.

use super::super::fast::Fast;
use super::tree::slack;

/// One node of the search, waiting on the stack.
#[derive(Clone, Copy)]
struct Search {
    node: u32,
}

/// `enter_edge`: the non-tree edge to bring onto the tree, or `None` when none leaves the
/// searched subtree.
pub fn enter_edge(g: &Fast, e: u32) -> Option<u32> {
    let tail = g.edges[e as usize].tail;
    let head = g.edges[e as usize].head;
    let down = if g.nodes[tail as usize].lim < g.nodes[head as usize].lim {
        (tail, false)
    } else {
        (head, true)
    };
    search(g, down.0, down.1)
}

/// The body of both of the reference's searches, which differ only in which end of an edge
/// they look at: `from_out` picks `dfs_enter_outedge` over `dfs_enter_inedge`.
///
/// `low` and `lim` are the root's interval, which decide whether a candidate *leaves* the
/// searched subtree. The descent tests the **current** node's `ND_lim(v)` instead — the
/// reference compares against the node being read, not the one the walk started at — and
/// that is what keeps the walk inside the subtree: intervals grow towards the root, so
/// `lim(other) < lim(node)` is "further from the root".
fn search(g: &Fast, root: u32, from_out: bool) -> Option<u32> {
    let n = root as usize;
    let low = g.nodes[n].low;
    let lim = g.nodes[n].lim;
    let mut best: Option<u32> = None;
    let mut worst = i32::MAX;
    let mut stack = vec![Search { node: root }];
    while let Some(Search { node }) = stack.pop() {
        let here = g.nodes[node as usize].lim;
        for edge in edges_at(g, node, from_out, false) {
            let other = far(g, edge, from_out);
            if g.edges[edge as usize].tree_index < 0 {
                let out = g.nodes[other as usize].lim;
                if !(low <= out && out <= lim) && (worst > slack(g, edge) || best.is_none()) {
                    best = Some(edge);
                    worst = slack(g, edge);
                }
            } else if g.nodes[other as usize].lim < here {
                stack.push(Search { node: other });
            }
        }
        for edge in edges_at(g, node, !from_out, true) {
            if worst <= 0 {
                break;
            }
            let other = far(g, edge, !from_out);
            if g.nodes[other as usize].lim < here {
                stack.push(Search { node: other });
            }
        }
    }
    best
}

/// The adjacency list one half of the search reads: `from_out` picks the node's out list
/// against its in list, and `tree` picks the tree-out/tree-in list against the plain one.
fn edges_at(g: &Fast, node: u32, from_out: bool, tree: bool) -> Vec<u32> {
    let n = node as usize;
    match (tree, from_out) {
        (false, true) => g.out[n].clone(),
        (false, false) => g.inn[n].clone(),
        (true, true) => g.nodes[n].tree_out.clone(),
        (true, false) => g.nodes[n].tree_in.clone(),
    }
}

/// The far endpoint of `edge` from the node whose list it was read in.
fn far(g: &Fast, edge: u32, from_out: bool) -> u32 {
    if from_out {
        g.edges[edge as usize].head
    } else {
        g.edges[edge as usize].tail
    }
}
