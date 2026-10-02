//! `init_cutvalues` (`ns.c:299-303`) and the two depth-first walks it is built from:
//! `dfs_range_init` / `dfs_range` (the subtree intervals every other walk tests) and
//! `dfs_cutval` (the cut value of each tree edge, in post-order).
//!
//! Each tree node carries three numbers: `par`, the tree edge it was reached from, and
//! `low` / `lim`, the first and last depth-first index in its subtree. `SEQ(a,b,c)` —
//! `low(v) <= lim(w) && lim(w) <= lim(v)`, `ns.c:44` — then says in one comparison whether
//! `w` is inside `v`'s subtree, and every test in the pass (`x_val`, the entering-edge
//! search, `treeupdate`) is that one question. The intervals are assigned so that they *are*
//! the answer, which is why `invalidate_path` exists: a pivot can leave a stale interval
//! behind, and a stale interval makes the next walk wrong.
//!
//! Determinism: both walks are depth-first in the reference's edge order — the tree-out list
//! then the tree-in list at each node — and the `dfs_range` reuse test is the reference's,
//! which takes a subtree whole rather than re-descending it. Nothing here reads a clock or a
//! hash order (`prompt.md` §6 D1-D10).

use super::super::fast::Fast;
use super::xval::cutval;
use super::Error;

/// One frame of a `dfs_range*` walk: a node, the tree edge it came from, the depth-first
/// index its subtree starts at, and its two tree-adjacency cursors. The reference's
/// `dfs_state_t` (`ns.c:1164-1169`).
#[derive(Clone, Copy)]
struct RangeFrame {
    node: u32,
    par: Option<u32>,
    lim: i32,
    out_at: usize,
    in_at: usize,
}

/// One frame of the `dfs_cutval` walk. The reference's `state_t` at `ns.c:1113-1118`.
#[derive(Clone, Copy)]
struct CutFrame {
    node: u32,
    par: Option<u32>,
    out_at: usize,
    in_at: usize,
}

/// `init_cutvalues`: the depth-first intervals over the whole tree, then the cut values in
/// post-order. The root is the head of the node list, which is what `GD_nlist` is.
pub fn init_cutvalues(g: &mut Fast, nodes: &[u32]) -> Result<(), Error> {
    if nodes.is_empty() {
        return Ok(());
    }
    range_init(g, nodes[0]);
    cut(g, nodes[0])
}

/// `dfs_range_init` (`ns.c:1176-1237`): assign `par`, `low` and `lim` from the tree root
/// outwards. `low` is a node's own depth-first index and `lim` the last one in its
/// subtree, so `low <= lim` and the root spans everything.
pub fn range_init(g: &mut Fast, root: u32) -> i32 {
    g.nodes[root as usize].par = None;
    g.nodes[root as usize].low = 1;
    let mut stack = vec![RangeFrame {
        node: root,
        par: None,
        lim: 1,
        out_at: 0,
        in_at: 0,
    }];
    run_range(g, &mut stack, false)
}

/// `dfs_range` (`ns.c:1242-1316`): the same walk restarted at `root`, except that a
/// subtree which already has the right parent and interval is taken whole rather than
/// re-descended. That reuse is the point: the pivot loop calls this once per exchange and
/// the alternative is re-walking the whole tree each time.
pub fn range_update(g: &mut Fast, root: u32, par: Option<u32>, low: i32) -> i32 {
    let n = root as usize;
    if g.nodes[n].par == par && g.nodes[n].low == low {
        return g.nodes[n].lim + 1;
    }
    g.nodes[n].par = par;
    g.nodes[n].low = low;
    let mut stack = vec![RangeFrame {
        node: root,
        par,
        lim: low,
        out_at: 0,
        in_at: 0,
    }];
    run_range(g, &mut stack, true)
}

/// The shared body of the two range walks: close each subtree, hand the next free index to
/// the parent, and stop. Returns the last index handed out plus one — the reference's `lim`.
fn run_range(g: &mut Fast, stack: &mut Vec<RangeFrame>, reuse: bool) -> i32 {
    let mut last = 0;
    while !stack.is_empty() {
        if step_out(g, stack, reuse) || step_in(g, stack, reuse) {
            continue;
        }
        let top = stack.len() - 1;
        let (node, lim) = (stack[top].node, stack[top].lim);
        g.nodes[node as usize].lim = lim;
        last = lim;
        stack.pop();
        if let Some(parent) = stack.last_mut() {
            parent.lim = last + 1;
        }
    }
    last + 1
}

/// The tree-out half of the current frame: descend into the next unvisited tree edge, by
/// pushing its head or — under `reuse` — by taking a subtree whose interval is already
/// correct whole and skipping over it. Returns whether the frame moved.
fn step_out(g: &mut Fast, stack: &mut Vec<RangeFrame>, reuse: bool) -> bool {
    let top = stack.len() - 1;
    while stack[top].out_at < g.nodes[stack[top].node as usize].tree_out.len() {
        let edge = g.nodes[stack[top].node as usize].tree_out[stack[top].out_at];
        stack[top].out_at += 1;
        if stack[top].par != Some(edge) {
            push_range(g, stack, Descend { edge, from_out: true, reuse });
            return true;
        }
    }
    false
}

/// The tree-in half of the current frame, mirroring [`step_out`] with the ends the other
/// way round. The reference runs both halves at `ns.c:1190` and `ns.c:1207`.
fn step_in(g: &mut Fast, stack: &mut Vec<RangeFrame>, reuse: bool) -> bool {
    let top = stack.len() - 1;
    while stack[top].in_at < g.nodes[stack[top].node as usize].tree_in.len() {
        let edge = g.nodes[stack[top].node as usize].tree_in[stack[top].in_at];
        stack[top].in_at += 1;
        if stack[top].par != Some(edge) {
            push_range(g, stack, Descend { edge, from_out: false, reuse });
            return true;
        }
    }
    false
}

/// The edge a frame is about to descend along and which of its two tree lists it came from.
/// `reuse` is the one bit of `dfs_range_init`'s caller that differs.
#[derive(Clone, Copy)]
struct Descend {
    edge: u32,
    from_out: bool,
    reuse: bool,
}

/// Descend the top frame along one edge, or absorb the subtree beyond it whole when `reuse`
/// says its `par` and `low` already match — in which case the frame's own limit steps past
/// it and the walk carries on without a push.
fn push_range(g: &mut Fast, stack: &mut Vec<RangeFrame>, at: Descend) {
    let top = stack.len() - 1;
    let other = if at.from_out {
        g.edges[at.edge as usize].head
    } else {
        g.edges[at.edge as usize].tail
    };
    let lim = stack[top].lim;
    if at.reuse
        && g.nodes[other as usize].par == Some(at.edge)
        && g.nodes[other as usize].low == lim
    {
        stack[top].lim = g.nodes[other as usize].lim + 1;
        return;
    }
    g.nodes[other as usize].par = Some(at.edge);
    g.nodes[other as usize].low = lim;
    stack.push(RangeFrame {
        node: other,
        par: Some(at.edge),
        lim,
        out_at: 0,
        in_at: 0,
    });
}

/// `dfs_cutval` (`ns.c:1110-1159`): walk the tree in post-order, and at each node's exit
/// compute the cut value of the edge it was reached by.
fn cut(g: &mut Fast, root: u32) -> Result<(), Error> {
    let mut stack = vec![CutFrame {
        node: root,
        par: None,
        out_at: 0,
        in_at: 0,
    }];
    while !stack.is_empty() {
        if cut_step(g, &mut stack, true) || cut_step(g, &mut stack, false) {
            continue;
        }
        let frame = stack.pop().expect("the frame just read");
        if let Some(edge) = frame.par {
            cutval(g, edge)?;
        }
    }
    Ok(())
}

/// One half of the post-order walk: descend along the next unvisited tree edge from the
/// node's tree-out list or its tree-in list.
fn cut_step(g: &mut Fast, stack: &mut Vec<CutFrame>, from_out: bool) -> bool {
    let top = stack.len() - 1;
    let node = stack[top].node as usize;
    let list = if from_out {
        &g.nodes[node].tree_out
    } else {
        &g.nodes[node].tree_in
    };
    while if from_out {
        stack[top].out_at < list.len()
    } else {
        stack[top].in_at < list.len()
    } {
        let at = if from_out {
            &mut stack[top].out_at
        } else {
            &mut stack[top].in_at
        };
        let edge = list[*at];
        *at += 1;
        if stack[top].par == Some(edge) {
            continue;
        }
        let other = if from_out {
            g.edges[edge as usize].head
        } else {
            g.edges[edge as usize].tail
        };
        stack.push(CutFrame {
            node: other,
            par: Some(edge),
            out_at: 0,
            in_at: 0,
        });
        return true;
    }
    false
}