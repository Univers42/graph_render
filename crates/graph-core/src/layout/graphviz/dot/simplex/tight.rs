//! The two searches `feasible_tree` is built from, split out of `tree.rs` so each stays
//! inside the house's file limit: the maximal tight subtree under a seed, and the walk that
//! finds the tightest edge leaving one.
//!
//! Determinism: both are depth-first in the reference's edge order — in-list before
//! out-list at each node, each in insertion order — and neither reads a clock, a hash order
//! or a random number (`prompt.md` §6 D1-D10).

use super::super::fast::Fast;
use super::subtree::NO_TREE;
use super::tree::add_tree_edge;
use super::{Ctx, Error};

/// One frame of the tight-subtree sweep: a node and the two adjacency slots it has yet to
/// read. The reference keeps the same three numbers in its `tst_t` (`ns.c:324-328`).
#[derive(Clone, Copy)]
pub struct Frame {
    /// The node the frame is at.
    pub node: u32,
    /// The next slot of `ND_in` to read.
    pub in_at: usize,
    /// The next slot of `ND_out` to read.
    pub out_at: usize,
}

/// The one tight edge a frame takes next, and the side it came from. The two sides keep
/// separate cursors, which is why this carries the side rather than just the edge.
pub struct Step {
    /// The edge.
    pub edge: u32,
    /// The node on its far side, which is the one being claimed.
    pub other: u32,
    /// Whether it came from the node's in list.
    pub from_in: bool,
}

/// `grow_tight` = `tight_subtree_search` (`ns.c:331-404`) with `find_tight_subtree`'s
/// bookkeeping: the maximal tight subtree under `root`, every node it reaches claimed for
/// `slot`, and its node count, which is the heap's key.
///
/// The frame stack resumes rather than recurses, so the cursor a node had when it was
/// interrupted is the cursor it comes back to — which is what makes the walk visit edges in
/// the reference's order and not merely visit the same edges.
pub fn grow_tight(g: &mut Fast, ctx: &mut Ctx, root: u32, slot: usize) -> Result<usize, Error> {
    let mut size = 0;
    let mut stack = vec![Frame {
        node: root,
        in_at: 0,
        out_at: 0,
    }];
    claim(g, root, slot);
    loop {
        let Some(frame) = stack.last().copied() else {
            return Ok(size);
        };
        match next_tight(g, frame) {
            Some(step) => {
                add_tree_edge(g, ctx, step.edge)?;
                claim(g, step.other, slot);
                let at = stack.len() - 1;
                if step.from_in {
                    stack[at].in_at += 1;
                } else {
                    stack[at].out_at += 1;
                }
                stack.push(Frame {
                    node: step.other,
                    in_at: 0,
                    out_at: 0,
                });
            }
            None => {
                stack.pop();
                size += 1;
            }
        }
    }
}

/// `ND_subtree_set(agtail(e), st)` (`ns.c:357,381`).
fn claim(g: &mut Fast, node: u32, slot: usize) {
    g.nodes[node as usize].subtree = i32::try_from(slot).expect("slot fits i32");
}

/// The first tight edge at or after a frame's two cursors whose far node is unclaimed:
/// in-list first, then out-list, each in its own order. This is the body of the
/// `tight_subtree_search` inner loop with its two `for` headers as cursors.
fn next_tight(g: &Fast, frame: Frame) -> Option<Step> {
    let n = frame.node as usize;
    let mut at = frame.in_at;
    while at < g.inn[n].len() {
        let edge = g.inn[n][at];
        if g.edges[edge as usize].tree_index < 0 {
            let other = g.edges[edge as usize].tail;
            if g.nodes[other as usize].subtree == NO_TREE && tight(g, edge) {
                return Some(Step {
                    edge,
                    other,
                    from_in: true,
                });
            }
        }
        at += 1;
    }
    let mut at = frame.out_at;
    while at < g.out[n].len() {
        let edge = g.out[n][at];
        if g.edges[edge as usize].tree_index < 0 {
            let other = g.edges[edge as usize].head;
            if g.nodes[other as usize].subtree == NO_TREE && tight(g, edge) {
                return Some(Step {
                    edge,
                    other,
                    from_in: false,
                });
            }
        }
        at += 1;
    }
    None
}

/// `SLACK(e) == 0` (`ns.c:43`): the only edges the tight sweep follows.
fn tight(g: &Fast, edge: u32) -> bool {
    super::slack(g, edge) == 0
}
