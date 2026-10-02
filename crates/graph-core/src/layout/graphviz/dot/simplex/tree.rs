//! `feasible_tree` (`ns.c:622-672`) and the two searches it is built from: the maximal
//! tight subtree under a seed (`ns.c:331-417`) and the minimum-slack edge leaving one
//! (`ns.c:454-530`).
//!
//! The pass is a phase-0 simplex warm start, so it produces a *feasible* ranking without
//! pivoting: a spanning tree whose every edge is tight (`LENGTH(e) == ED_minlen(e)`). It
//! is built in two moves. First every maximal tight subtree is collected in one sweep —
//! the graph may arrive as many small ones, because `init_rank`'s longest-path ranking
//! leaves slack on most edges. Then the smallest is merged into its neighbour through the
//! minimum-slack edge between them, sliding that whole subtree by the slack, until one
//! tree is left.
//!
//! Smallest first is what makes it deterministic: the order the subtrees are merged in is
//! the order the reference's heap hands them out, and the tight tree it reaches is the tree
//! `init_cutvalues` and the pivot loop then walk.
//!
//! Determinism: the subtree sweep walks in-edges before out-edges on each node and each
//! list in insertion order, the inter-tree walk follows the reference's own stack order,
//! and the heap is a plain binary min-heap over an array compared on the subtree size.

use super::super::fast::Fast;
use super::subtree::{self, Subtree, NO_TREE};
use super::{init_cutvalues, Ctx, Error};

/// One frame of the tight-subtree sweep: a node and the two adjacency slots it has yet to
/// read. The reference keeps the same three numbers in its `tst_t`.
#[derive(Clone, Copy)]
struct Frame {
    node: u32,
    in_at: usize,
    out_at: usize,
}

/// The one tight edge a frame takes next, and the side it came from. The two sides keep
/// separate cursors, which is why this carries the side rather than just the edge.
struct Step {
    edge: u32,
    other: u32,
    from_in: bool,
}

/// One frame of the inter-tree walk: a node, the subtree it belongs to, and the node it was
/// reached from. `from` is what stops the walk stepping back over the edge it arrived by;
/// it is `None` at the seed, the reference's `NULL`.
#[derive(Clone, Copy)]
struct Reach {
    node: u32,
    subtree: usize,
    from: Option<u32>,
    out_at: usize,
    in_at: usize,
}

/// `add_tree_edge` (`ns.c:57-83`): give `edge` the next free tree slot, file it at both
/// endpoints and mark both.
pub fn add_tree_edge(g: &mut Fast, ctx: &mut Ctx, edge: u32) -> Result<(), Error> {
    if g.edges[edge as usize].tree_index >= 0 {
        return Err(Error::Tree);
    }
    let slot = i32::try_from(ctx.tree_edge.len()).expect("tree length fits i32");
    g.edges[edge as usize].tree_index = slot;
    ctx.tree_edge.push(edge);
    let tail = g.edges[edge as usize].tail;
    let head = g.edges[edge as usize].head;
    g.nodes[tail as usize].mark = true;
    g.nodes[tail as usize].tree_out.push(edge);
    g.nodes[head as usize].mark = true;
    g.nodes[head as usize].tree_in.push(edge);
    Ok(())
}

/// `feasible_tree`: one maximal tight spanning tree over `nodes`, adjusting the ranks as
/// it goes, and then the initial cut values.
pub fn feasible_tree(g: &mut Fast, ctx: &mut Ctx, nodes: &[u32]) -> Result<(), Error> {
    let mut trees: Vec<Subtree> = Vec::with_capacity(nodes.len());
    for &n in nodes {
        if g.nodes[n as usize].subtree != NO_TREE {
            continue;
        }
        let slot = trees.len();
        trees.push(Subtree {
            rep: n,
            size: 0,
            par: slot,
            heap_index: None,
        });
        trees[slot].size = grow_tight(g, ctx, n, slot)?;
    }
    let mut size = trees.len();
    let mut heap: Vec<usize> = (0..size).collect();
    subtree::build_heap(&mut heap, &mut trees);
    while size > 1 {
        let extracted = subtree::extract_min(&mut heap, &mut trees, size);
        size -= 1;
        let rep = trees[extracted].rep;
        let Some(edge) = inter_tree_edge(g, &mut trees, rep) else {
            return Err(Error::Disconnected);
        };
        let rep = merge_trees(g, ctx, &mut trees, edge)?;
        // The representative of a merge is always a subtree still on the heap, because
        // exactly one of the two merged was the one just extracted, so this index is live.
        if let Some(at) = trees[rep].heap_index {
            subtree::sift_down(&mut heap, &mut trees, size, at);
        }
    }
    init_cutvalues(g, nodes)?;
    Ok(())
}

/// `grow_tight` = `tight_subtree_search` (`ns.c:331-404`) with `find_tight_subtree`'s
/// bookkeeping: the maximal tight subtree under `root`, every node it reaches claimed for
/// `slot`, and its node count, which is the heap's key.
fn grow_tight(g: &mut Fast, ctx: &mut Ctx, root: u32, slot: usize) -> Result<usize, Error> {
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

/// `inter_tree_edge` (`ns.c:527-530`) over `inter_tree_edge_search` (`ns.c:454-525`): the
/// minimum-slack edge leaving the tight subtree under `rep`. A tight candidate ends the
/// search immediately, which is the reference's early exit and the reason the walk is
/// bounded.
fn inter_tree_edge(g: &Fast, trees: &mut [Subtree], rep: u32) -> Option<u32> {
    let mut best: Option<u32> = None;
    let mut stack = vec![Reach {
        node: rep,
        subtree: subtree::find(trees, g.nodes[rep as usize].subtree as usize),
        from: None,
        out_at: 0,
        in_at: 0,
    }];
    while let Some(mut reach) = stack.last().copied() {
        if reach.out_at == 0 && reach.in_at == 0 && best.is_some_and(|b| slack(g, b) == 0) {
            stack.pop();
            continue;
        }
        let descended = scan_out(g, trees, &mut reach, &mut best)
            .or_else(|| scan_in(g, trees, &mut reach, &mut best));
        let frame = stack.last_mut().expect("the frame just read");
        frame.out_at = reach.out_at;
        frame.in_at = reach.in_at;
        if let Some(other) = descended {
            stack.push(Reach {
                node: other,
                subtree: subtree::find(trees, g.nodes[other as usize].subtree as usize),
                from: Some(reach.node),
                out_at: 0,
                in_at: 0,
            });
        } else {
            stack.pop();
        }
    }
    best
}

/// The out-edge half of the inter-tree walk at one node: descend a tree edge, or record a
/// candidate that crosses out of the subtree. `reach` carries the cursor in and out, and
/// the returned node is where to descend to.
fn scan_out(
    g: &Fast,
    trees: &mut [Subtree],
    reach: &mut Reach,
    best: &mut Option<u32>,
) -> Option<u32> {
    let n = reach.node as usize;
    while reach.out_at < g.out[n].len() {
        let edge = g.out[n][reach.out_at];
        reach.out_at += 1;
        let head = g.edges[edge as usize].head;
        if g.edges[edge as usize].tree_index >= 0 {
            if Some(head) != reach.from {
                return Some(head);
            }
        } else if subtree::find(trees, g.nodes[head as usize].subtree as usize) != reach.subtree
            && best.is_none_or(|b| slack(g, edge) < slack(g, b))
        {
            *best = Some(edge);
        }
    }
    None
}

/// The in-edge half of the inter-tree walk, mirroring [`scan_out`] with the two ends the
/// other way round — the reference says so at `ns.c:500` and it has to, or the two halves
/// would search different graphs.
fn scan_in(
    g: &Fast,
    trees: &mut [Subtree],
    reach: &mut Reach,
    best: &mut Option<u32>,
) -> Option<u32> {
    let n = reach.node as usize;
    while reach.in_at < g.inn[n].len() {
        let edge = g.inn[n][reach.in_at];
        reach.in_at += 1;
        let tail = g.edges[edge as usize].tail;
        if g.edges[edge as usize].tree_index >= 0 {
            if Some(tail) != reach.from {
                return Some(tail);
            }
        } else if subtree::find(trees, g.nodes[tail as usize].subtree as usize) != reach.subtree
            && best.is_none_or(|b| slack(g, edge) < slack(g, b))
        {
            *best = Some(edge);
        }
    }
    None
}

/// `merge_trees` (`ns.c:593-615`): join the two subtrees at `edge`, sliding whichever one
/// is already off the heap by the edge's slack so the edge lands tight.
fn merge_trees(
    g: &mut Fast,
    ctx: &mut Ctx,
    trees: &mut [Subtree],
    edge: u32,
) -> Result<usize, Error> {
    let tail = g.edges[edge as usize].tail;
    let head = g.edges[edge as usize].head;
    let tail_set = subtree::find(trees, g.nodes[tail as usize].subtree as usize);
    let head_set = subtree::find(trees, g.nodes[head as usize].subtree as usize);
    let (moving, delta) = if trees[tail_set].heap_index.is_none() {
        (tail_set, slack(g, edge))
    } else {
        (head_set, -slack(g, edge))
    };
    if delta != 0 {
        tree_adjust(g, trees[moving].rep, None, delta);
    }
    add_tree_edge(g, ctx, edge)?;
    Ok(subtree::union(trees, tail_set, head_set))
}

/// `tree_adjust` (`ns.c:576-591`): slide every node of a tight subtree by `delta`, walking
/// the tree away from `from`. Explicit stack, not recursion: a subtree can hold every node
/// of a 100 000-node graph.
fn tree_adjust(g: &mut Fast, v: u32, from: Option<u32>, delta: i32) {
    let mut stack = vec![(v, from)];
    while let Some((node, came_from)) = stack.pop() {
        g.nodes[node as usize].rank += delta;
        for &edge in &g.nodes[node as usize].tree_in.clone() {
            let w = g.edges[edge as usize].tail;
            if Some(w) != came_from {
                stack.push((w, Some(node)));
            }
        }
        for &edge in &g.nodes[node as usize].tree_out.clone() {
            let w = g.edges[edge as usize].head;
            if Some(w) != came_from {
                stack.push((w, Some(node)));
            }
        }
    }
}

/// `SLACK(e)` (`ns.c:43`): the room the edge has over its minimum.
fn slack(g: &Fast, edge: u32) -> i32 {
    let record = &g.edges[edge as usize];
    g.nodes[record.head as usize].rank - g.nodes[record.tail as usize].rank - record.minlen
}

/// `SLACK(e) == 0`, the only edges `tight_subtree_search` follows.
fn tight(g: &Fast, edge: u32) -> bool {
    slack(g, edge) == 0
}