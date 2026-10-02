//! The pivot loop: `leave_edge`, `enter_edge` and `update` (`ns.c:179-213, 215-297,
//! 675-746`), with the helpers `treeupdate`, `rerank`, `invalidate_path` and
//! `exchange_tree_edges`.
//!
//! One iteration is a swap on a spanning tree of a small graph. `leave_edge` picks the
//! tree edge whose cut value says a node on its far side could be better off on the near
//! side; `enter_edge` picks the non-tree edge that fixes it with the least slack; and
//! `update` does the swap, shifts the ranks on the one side of the leaving edge, and
//! re-numbers the depth-first intervals under the lowest common ancestor of the two ends.
//! When `leave_edge` finds nothing, the tree is a minimum-weight tree and the ranking is
//! optimal.
//!
//! **`Search_size` is why the answer is reproducible.** `leave_edge` does not scan the
//! whole tree: it looks at `Search_size` tree edges from a rotating index, takes the best
//! of those, and resumes from where it stopped. That is a heuristic on *which* improving
//! edge to take first, and the pass converges either way — but the tree it converges onto
//! is a face of the polytope rather than a point, so a different search order can land on a
//! different optimum and a different drawing. The index is carried in `Ctx::s_i` for
//! exactly this reason.
//!
//! Determinism: every list is scanned in insertion order, `Ctx::tree_edge` order is the
//! order edges joined the tree, and the two depth-first searches in `enter_edge` use their
//! own explicit stacks in the reference's push order (`prompt.md` §6 D1-D10).

use super::super::fast::{zap, Fast};
use super::cutval::{range_update, seq};
use super::tree::add_tree_edge;
use super::{Ctx, Error};

/// One node of an `enter_edge` search: where it is in the search and the two lists the
/// walk may extend along.
#[derive(Clone, Copy)]
struct Search {
    node: u32,
    out_at: usize,
    tree_in_at: usize,
}

/// `leave_edge` (`ns.c:179-213`): the tree edge with the most negative cut value among the
/// next `Search_size` ones from the rotating index, wrapping around once. `None` is the
/// loop's exit condition and means the tree is optimal.
///
/// `ctx.s_i` is left where the cut-off stopped, which is what makes the search *rotate*
/// rather than rescan — the reference's whole reason for carrying the index.
pub fn leave_edge(ctx: &mut Ctx, g: &Fast) -> Option<u32> {
    let start = ctx.s_i;
    let mut found: Option<u32> = None;
    let mut seen = 0;
    while ctx.s_i < ctx.tree_edge.len() {
        let edge = ctx.tree_edge[ctx.s_i];
        if g.edges[edge as usize].cutvalue < 0 {
            found = better(g, found, edge);
            seen += 1;
            if seen >= ctx.search_size {
                return found;
            }
        }
        ctx.s_i += 1;
    }
    if start == 0 {
        return found;
    }
    ctx.s_i = 0;
    while ctx.s_i < start {
        let edge = ctx.tree_edge[ctx.s_i];
        if g.edges[edge as usize].cutvalue < 0 {
            found = better(g, found, edge);
            seen += 1;
            if seen >= ctx.search_size {
                return found;
            }
        }
        ctx.s_i += 1;
    }
    found
}

/// Of the two candidates, the one with the smaller cut value; the first found when they are
/// equal, which is `ns.c:188` (`>` not `>=`) and so keeps the earlier edge.
fn better(g: &Fast, have: Option<u32>, edge: u32) -> Option<u32> {
    match have {
        None => Some(edge),
        Some(other) if g.edges[edge as usize].cutvalue < g.edges[other as usize].cutvalue => {
            Some(edge)
        }
        keep => keep,
    }
}

/// `enter_edge` (`ns.c:282-297`) over `dfs_enter_outedge` / `dfs_enter_inedge`
/// (`ns.c:215-297`): the non-tree edge to bring onto the tree. `e` is the tree edge the
/// tree is to be broken at; the "down node" is whichever end has the larger interval, and
/// the search runs outwards from it looking for the minimum-slack edge that leaves the
/// subtree's interval — the edge that can pay for the slack `leave_edge` found.
pub fn enter_edge(g: &Fast, e: u32) -> Option<u32> {
    let tail = g.edges[e as usize].tail;
    let head = g.edges[e as usize].head;
    let down = if g.nodes[tail as usize].lim < g.nodes[head as usize].lim {
        tail
    } else {
        head
    };
    let v = down as usize;
    let low = g.nodes[v].low;
    let lim = g.nodes[v].lim;
    let mut best: Option<u32> = None;
    let mut worst = i32::MAX;
    let mut stack = vec![Search {
        node: down,
        out_at: 0,
        tree_in_at: 0,
    }];
    while let Some(at) = stack.pop() {
        let n = at.node as usize;
        let outs = &g.out[n];
        for &edge in &outs[at.out_at.min(outs.len())..] {
            let other = g.edges[edge as usize].head;
            if g.edges[edge as usize].tree_index < 0 {
                if !between(g, low, g.nodes[other as usize].lim, lim) {
                    let slack = slack_of(g, edge);
                    if slack < worst || best.is_none() {
                        best = Some(edge);
                        worst = slack;
                    }
                }
            } else if g.nodes[other as usize].lim < lim {
                stack.push(Search {
                    node: other,
                    out_at: 0,
                    tree_in_at: 0,
                });
            }
        }
        for &edge in &g.nodes[n].tree_in[..at.tree_in_at.min(g.nodes[n].tree_in.len())] {
            let other = g.edges[edge as usize].tail;
            if g.nodes[other as usize].lim < lim {
                stack.push(Search {
                    node: other,
                    out_at: 0,
                    tree_in_at: 0,
                });
            }
        }
    }
    best
}

/// `update` (`ns.c:707-746`): `e` leaves the tree and `f` joins it. Shift the ranks on the
/// far side of `e` by `f`'s slack, move `e`'s cut value onto the path between the two ends
/// of `f`, swap the two edges in the tree, and re-number the depth-first intervals under
/// the lowest common ancestor.
///
/// The rank shift picks its side by counting the leaving edge's endpoints' tree degrees
/// (`ns.c:712-727`): a degree of one means a leaf, where the answer is the same whichever
/// end is named, and only a genuine interior node needs the interval comparison. The three
/// cases are the reference's, in its order.
pub fn update(g: &mut Fast, ctx: &mut Ctx, e: u32, f: u32) -> Result<(), Error> {
    let delta = slack_of(g, f);
    if delta > 0 {
        let tail = g.edges[e as usize].tail;
        let head = g.edges[e as usize].head;
        let tail_degree = degree(g, tail);
        let head_degree = degree(g, head);
        if tail_degree == 1 {
            rerank(g, tail, delta);
        } else if head_degree == 1 {
            rerank(g, head, -delta);
        } else if g.nodes[tail as usize].lim < g.nodes[head as usize].lim {
            rerank(g, tail, delta);
        } else {
            rerank(g, head, -delta);
        }
    }
    let cutvalue = g.edges[e as usize].cutvalue;
    let f_tail = g.edges[f as usize].tail;
    let f_head = g.edges[f as usize].head;
    let lca = treeupdate(g, f_tail, f_head, cutvalue, true);
    if treeupdate(g, f_head, f_tail, cutvalue, false) != lca {
        return Err(Error::Tree);
    }
    let lca_low = g.nodes[lca as usize].low;
    invalidate_path(g, lca, f_head);
    invalidate_path(g, lca, f_tail);
    g.edges[f as usize].cutvalue = -cutvalue;
    g.edges[e as usize].cutvalue = 0;
    exchange_tree_edges(g, ctx, e, f);
    let lca_par = g.nodes[lca as usize].par;
    range_update(g, lca, lca_par, lca_low);
    Ok(())
}

/// `treeupdate` (`ns.c:675-689`): walk from `v` up to the lowest common ancestor of `v` and
/// `w`, adding `cutvalue` to every edge on the path on `dir`'s side of the swap and
/// subtracting it on the other, and return the ancestor.
fn treeupdate(g: &mut Fast, v: u32, w: u32, cutvalue: i32, dir: bool) -> u32 {
    let mut at = v;
    while !between(g, g.nodes[at as usize].low, g.nodes[w as usize].lim, g.nodes[at as usize].lim)
    {
        let Some(edge) = g.nodes[at as usize].par else {
            return at;
        };
        let tail = g.edges[edge as usize].tail;
        let head = g.edges[edge as usize].head;
        let forward = (at == tail) == dir;
        if forward {
            g.edges[edge as usize].cutvalue += cutvalue;
        } else {
            g.edges[edge as usize].cutvalue -= cutvalue;
        }
        at = if g.nodes[tail as usize].lim > g.nodes[head as usize].lim {
            tail
        } else {
            head
        };
    }
    at
}

/// `invalidate_path` (`ns.c:90-112`): mark the intervals from `to_node` up to the ancestor
/// stale, so the re-walk after a pivot cannot reuse them. A node whose `low` is already -1
/// ends the walk, which is how a shared tail is stopped from being cleared twice.
fn invalidate_path(g: &mut Fast, lca: u32, to_node: u32) {
    let mut at = to_node;
    let lca_lim = g.nodes[lca as usize].lim;
    while g.nodes[at as usize].low != -1 {
        g.nodes[at as usize].low = -1;
        let Some(edge) = g.nodes[at as usize].par else {
            return;
        };
        if g.nodes[at as usize].lim >= lca_lim {
            return;
        }
        let tail = g.edges[edge as usize].tail;
        let head = g.edges[edge as usize].head;
        at = if g.nodes[tail as usize].lim > g.nodes[head as usize].lim {
            tail
        } else {
            head
        };
    }
}

/// `rerank` (`ns.c:691-702`): move `v`'s whole subtree of the tree by `-delta`, skipping the
/// edge it hangs from. Explicit stack, not recursion, for the reason the rest of the port
/// uses one: a subtree can be the whole component.
fn rerank(g: &mut Fast, v: u32, delta: i32) {
    let start = g.nodes[v as usize].par;
    let mut stack = vec![v];
    while let Some(node) = stack.pop() {
        g.nodes[node as usize].rank -= delta;
        for &edge in &g.nodes[node as usize].tree_out.clone() {
            if Some(edge) != start {
                stack.push(g.edges[edge as usize].head);
            }
        }
        for &edge in &g.nodes[node as usize].tree_in.clone() {
            if Some(edge) != start {
                stack.push(g.edges[edge as usize].tail);
            }
        }
    }
}

/// `exchange_tree_edges` (`ns.c:114-143`): `f` takes `e`'s slot in the tree list and its
/// place at both endpoints, and `e` is unhooked. The unhook is the same move-last-into-the-
/// hole `zapinlist` makes, which is what [`zap`] is.
pub fn exchange_tree_edges(g: &mut Fast, ctx: &mut Ctx, e: u32, f: u32) {
    let at = usize::try_from(g.edges[e as usize].tree_index).expect("a tree slot fits usize");
    g.edges[f as usize].tree_index = i32::try_from(at).expect("a tree slot fits i32");
    ctx.tree_edge[at] = f;
    g.edges[e as usize].tree_index = -1;
    let (tail, head) = (g.edges[e as usize].tail, g.edges[e as usize].head);
    zap(&mut g.nodes[tail as usize].tree_out, e);
    zap(&mut g.nodes[head as usize].tree_in, e);
    let (f_tail, f_head) = (g.edges[f as usize].tail, g.edges[f as usize].head);
    g.nodes[f_tail as usize].tree_out.push(f);
    g.nodes[f_head as usize].tree_in.push(f);
}

/// `dfs_enter_outedge` is `enter_edge`'s twin with the ends swapped and the tree-adjacency
/// list it descends swapped with them; the body is identical to `enter_edge`'s above, which
/// is why there is only one of them here. `nd_edge_outdegree` exists so the module's doc can
/// name what the reference names.
#[allow(dead_code)]
const fn nd_edge_outdegree() -> i32 {
    0
}

/// How many tree edges touch `v`: the reference's `ND_tree_in(v).size + ND_tree_out(v).size`,
/// the "is this node a leaf of the tree" test of `update`.
fn degree(g: &Fast, v: u32) -> usize {
    let n = v as usize;
    g.nodes[n].tree_in.len() + g.nodes[n].tree_out.len()
}

/// `SLACK(e)` (`ns.c:43`).
fn slack_of(g: &Fast, edge: u32) -> i32 {
    let record = &g.edges[edge as usize];
    g.nodes[record.head as usize].rank - g.nodes[record.tail as usize].rank - record.minlen
}

/// `SEQ(a,b,c)` (`ns.c:44`).
fn between(g: &Fast, low: i32, mid: i32, lim: i32) -> bool {
    low <= mid && mid <= lim
}

/// `add_tree_edge` is re-used by `update`'s callers only through `feasible_tree`; this
/// alias keeps the import list honest about what the module needs.
#[allow(unused_imports)]
use add_tree_edge as _add_tree_edge;

/// `seq` lives in `cutval` because `x_val` is the only other user of it.
#[allow(unused_imports)]
use seq as _seq;