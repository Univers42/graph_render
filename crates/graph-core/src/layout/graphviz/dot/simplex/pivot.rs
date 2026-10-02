//! The pivot loop: `leave_edge`, `enter_edge` and `update` (`ns.c:179-297, 675-746`),
//! with the helpers `treeupdate`, `rerank`, `invalidate_path` and `exchange_tree_edges`.
//!
//! One iteration is a swap on a spanning tree of a small graph. `leave_edge` picks the tree
//! edge whose cut value says a node on its far side could be better off on the near side;
//! `enter_edge` picks the non-tree edge that fixes it with the least slack; `update` does
//! the swap, shifts the ranks on the one side of the leaving edge, and re-numbers the
//! depth-first intervals under the lowest common ancestor of the two ends. When
//! `leave_edge` finds nothing the tree is a minimum-weight tree and the ranking is optimal.
//!
//! **`Search_size` is why the answer is reproducible.** `leave_edge` does not scan the whole
//! tree: it looks at `Search_size` tree edges from a rotating index, takes the best of
//! those, and resumes from where it stopped. That is a heuristic on *which* improving edge
//! to take first, and the loop converges either way — but the tree it converges onto is a
//! face of the polytope rather than a point, so a different search order can land on a
//! different optimum and a different drawing. The index is carried in `Ctx::s_i` for
//! exactly this reason, and `Pivot` below names the cap so the two halves of the file
//! cannot drift apart on it.
//!
//! Determinism: every list is scanned in insertion order, `Ctx::tree_edge`'s order is the
//! order edges joined the tree, and the depth-first search in `enter_edge` uses an explicit
//! stack in the reference's push order (`prompt.md` §6 D1-D10).

use super::super::fast::{zap, Fast};
use super::Ctx;
use super::{Error};

/// One node of an `enter_edge` search, waiting on the stack.
#[derive(Clone, Copy)]
struct Search {
    node: u32,
}

/// `leave_edge` (`ns.c:179-213`): the tree edge with the most negative cut value among the
/// next `Search_size` ones from the rotating index, wrapping past the start of the list
/// once. `None` is the loop's exit condition and means the tree is optimal.
///
/// `ctx.s_i` is left where the cut-off stopped, which is what makes the search rotate
/// rather than rescan — the reference's whole reason for carrying the index.
pub fn leave_edge(ctx: &mut Ctx, g: &Fast) -> Option<u32> {
    let start = ctx.s_i;
    let mut found: Option<u32> = None;
    let mut seen = 0;
    while ctx.s_i < ctx.tree_edge.len() {
        if scan_one(ctx, g, &mut found, &mut seen) {
            return found;
        }
        ctx.s_i += 1;
    }
    if start == 0 {
        return found;
    }
    ctx.s_i = 0;
    while ctx.s_i < start {
        if scan_one(ctx, g, &mut found, &mut seen) {
            return found;
        }
        ctx.s_i += 1;
    }
    found
}

/// One step of [`leave_edge`]'s scan: remember the edge if its cut value is negative and it
/// beats what is held. Returns whether the search is finished, which it is once
/// `Search_size` negatives have been seen — and the reference returns at that point
/// *without* advancing the index, so the next call starts on the same edge.
fn scan_one(ctx: &mut Ctx, g: &Fast, found: &mut Option<u32>, seen: &mut usize) -> bool {
    let edge = ctx.tree_edge[ctx.s_i];
    if g.edges[edge as usize].cutvalue >= 0 {
        return false;
    }
    *found = better(g, *found, edge);
    *seen += 1;
    *seen >= ctx.search_size
}

/// Of the two candidates, the one with the smaller cut value; the first found when they are
/// equal, which is `ns.c:188` (`>` and not `>=`) and so keeps the earlier edge.
fn better(g: &Fast, have: Option<u32>, edge: u32) -> Option<u32> {
    let Some(other) = have else {
        return Some(edge);
    };
    if g.edges[edge as usize].cutvalue < g.edges[other as usize].cutvalue {
        Some(edge)
    } else {
        Some(other)
    }
}

/// `enter_edge` (`ns.c:282-297`) over `dfs_enter_outedge` / `dfs_enter_inedge`
/// (`ns.c:215-280`): the non-tree edge to bring onto the tree. `e` is the tree edge the
/// tree is to be broken at, the "down node" is whichever end has the larger interval, and
/// the search runs outwards from it looking for the minimum-slack edge that leaves the
/// subtree's interval — the one that can pay for the slack `leave_edge` found.
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
                if !(low <= out && out <= lim) && (worst > slack_of(g, edge) || best.is_none()) {
                    best = Some(edge);
                    worst = slack_of(g, edge);
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

/// `update` (`ns.c:707-746`): `e` leaves the tree and `f` joins it. Shift the ranks on the
/// far side of `e` by `f`'s slack, move `e`'s cut value onto the path between the two ends
/// of `f`, swap the two edges in the tree, and re-number the depth-first intervals under
/// the lowest common ancestor.
///
/// The rank shift picks its side by counting the leaving edge's endpoints' tree degrees
/// (`ns.c:712-727`): a degree of one means a leaf, where either end names the same
/// subtree, and only a genuine interior node needs the interval comparison. The three
/// cases are the reference's, in its order.
pub fn update(g: &mut Fast, ctx: &mut Ctx, e: u32, f: u32) -> Result<(), Error> {
    let delta = slack_of(g, f);
    if delta > 0 {
        let tail = g.edges[e as usize].tail;
        let head = g.edges[e as usize].head;
        let (up, down) = if degree(g, tail) == 1 {
            (tail, false)
        } else if degree(g, head) == 1 {
            (head, true)
        } else {
            (
                if g.nodes[tail as usize].lim < g.nodes[head as usize].lim {
                    tail
                } else {
                    head
                },
                false,
            )
        };
        rerank(g, up, if down { -delta } else { delta });
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
    super::cutval::range_update(g, lca, lca_par, lca_low);
    Ok(())
}

/// `treeupdate` (`ns.c:675-689`): walk from `v` up to the lowest common ancestor of `v` and
/// `w`, adding `cutvalue` to every edge on the path on `dir`'s side of the swap and
/// subtracting it on the other, and return the ancestor.
///
/// The reference dereferences `ND_par(v)` without a null check, which is safe only because
/// the root contains every node and so always satisfies the loop's exit test. Here the
/// missing parent returns the node itself, and `update` turns a disagreement between the two
/// walks into [`Error::Tree`] rather than a crash.
fn treeupdate(g: &mut Fast, v: u32, w: u32, cutvalue: i32, dir: bool) -> u32 {
    let mut at = v;
    while !seq(
        g.nodes[at as usize].low,
        g.nodes[w as usize].lim,
        g.nodes[at as usize].lim,
    ) {
        let Some(edge) = g.nodes[at as usize].par else {
            return at;
        };
        let tail = g.edges[edge as usize].tail;
        let head = g.edges[edge as usize].head;
        if (at == tail) == dir {
            g.edges[edge as usize].cutvalue += cutvalue;
        } else {
            g.edges[edge as usize].cutvalue -= cutvalue;
        }
        at = higher(g, tail, head);
    }
    at
}

/// `invalidate_path` (`ns.c:90-112`): mark the intervals from `to_node` up to the ancestor
/// stale, so the re-walk after a pivot cannot reuse them. A node whose `low` is already -1
/// ends the walk, which is how a shared tail is kept from being cleared twice.
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
        at = higher(g, tail, head);
    }
}

/// The endpoint of tree edge `(tail, head)` with the larger interval — the one further from
/// the root, and so the one the walks always move to.
fn higher(g: &Fast, tail: u32, head: u32) -> u32 {
    if g.nodes[tail as usize].lim > g.nodes[head as usize].lim {
        tail
    } else {
        head
    }
}

/// `rerank` (`ns.c:691-702`): move `v`'s whole subtree of the tree by `-delta`, skipping the
/// edge each node hangs from. Explicit stack, not recursion: a subtree can be the whole
/// component, and this port does not put the reference's call depth on the machine stack.
pub(super) fn rerank(g: &mut Fast, v: u32, delta: i32) {
    let mut stack = vec![(v, g.nodes[v as usize].par)];
    while let Some((node, skip)) = stack.pop() {
        g.nodes[node as usize].rank = g.nodes[node as usize].rank.wrapping_sub(delta);
        for &edge in &g.nodes[node as usize].tree_out.clone() {
            if Some(edge) != skip {
                stack.push((g.edges[edge as usize].head, Some(edge)));
            }
        }
        for &edge in &g.nodes[node as usize].tree_in.clone() {
            if Some(edge) != skip {
                stack.push((g.edges[edge as usize].tail, Some(edge)));
            }
        }
    }
}

/// `exchange_tree_edges` (`ns.c:114-143`): `f` takes `e`'s slot in the tree list and its
/// place at both endpoints, and `e` is unhooked — with the same move-last-into-the-hole
/// `zapinlist` makes, which is what [`zap`] is.
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

/// How many tree edges touch `v`: `ND_tree_in(v).size + ND_tree_out(v).size`, the
/// "is this node a leaf of the tree" test `update` starts from.
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
fn seq(low: i32, mid: i32, lim: i32) -> bool {
    low <= mid && mid <= lim
}