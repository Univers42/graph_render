//! `rank2` (`ns.c:951-1027`): Graphviz's network simplex, the pass that decides which
//! layer every node of a component sits on.
//!
//! It is reimplemented, not translated (`docs/decisions/graphviz-oracle.md`): the C walks
//! with pointer-typed `LIST`s and a borrowed `ND_par`, the Rust with dense edge indices
//! and typed per-node fields, and the recursion in `tree_adjust` / `rerank` is an explicit
//! stack because a component can hold every node of a 100 000-node graph. What is *not*
//! free to change is the order every one of those walks visits things in, because the
//! tree the pivot loop builds is only one of the optimal trees, and which one comes out
//! decides the drawing.
//!
//! The pass in the reference's order:
//!
//! 1. [`init_graph`] — clear the tree lists, zero the cut values, and ask whether the
//!    ranks already in hand satisfy every edge.
//! 2. [`init_rank`] if they do not: longest path from the sources.
//! 3. [`tree::feasible_tree`] — a maximal tight spanning tree, built from the maximal
//!    tight subtrees merged smallest first, then the initial cut values.
//! 4. The pivot loop — [`pivot::leave_edge`], [`enter::enter_edge`], [`pivot::update`] —
//!    until no tree edge has a negative cut value.
//! 5. One of the three balance passes in [`balance`].
//!
//! The children are the reference's own split, one module per part of `ns.c`: [`tree`] and
//! [`tight`] build the warm start, [`cutval`] and [`xval`] number the tree and turn weights
//! into cut values, [`pivot`] and [`enter`] are the loop, [`balance`] finishes, and
//! [`subtree`] is the union find and the heap the warm start merges through.
//!
//! **`balance` is a parameter, not a constant**, because `dot` runs this twice: `rank1`
//! ranks with `TB_balance` and `dot_position` runs the same engine over its auxiliary
//! graph with `LR_balance`. The two share every line above this one.
//!
//! Determinism: every value is an integer, every walk is in dense-index or insertion
//! order, and `Ctx::tree_edge`'s order is the order the edges joined the tree. Nothing
//! reads a clock, a hash order or a random number (`prompt.md` §6 D1-D10).

mod balance;
mod cutval;
mod enter;
mod pivot;
mod subtree;
mod tight;
mod tree;
mod xval;

#[cfg(test)]
mod checks;

use std::collections::VecDeque;

use super::fast::Fast;
use cutval::init_cutvalues;
use pivot::leave_edge;
use subtree::NO_TREE;
use tree::feasible_tree;

/// `enum { SEARCHSIZE = 30 }` (`ns.c:55`), the default `leave_edge` search cut-off: how
/// many tree edges with a negative cut value one call looks at before taking the best of
/// them and starting again.
pub const SEARCH_SIZE: i32 = 30;

/// What `rank2` can fail at. Both are the reference's `return 1` and `return 2`, and both
/// mean the input graph is not what the pass assumes rather than that the pass went wrong.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// `feasible_tree` found no edge between two tight trees: the component is not
    /// connected, so no spanning tree exists.
    Disconnected,
    /// An edge was asked to join a tree it is already in, or the two `treeupdate` walks
    /// met at different lowest common ancestors. Both are the reference's `return 2`.
    Tree,
    /// A cut value did not fit `int`, which `x_cutval` reports as an overflow and the
    /// reference answers by exiting (`ns.c:1060-1067`). Reachable only by a graph whose
    /// merged edge weights sum past two billion.
    Overflow,
}

/// Which balance pass runs after the pivot loop — `rank2`'s `balance` argument.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Balance {
    /// `balance == 0`: normalise the ranks so the lowest is 0, and nothing else. `dot`
    /// runs this only when the graph has clusters, which this port has not.
    None,
    /// `balance == 1`, `TB_balance`: move every node whose in-weight equals its out-weight
    /// to the least populated rank it may legally sit on. **`rank1` runs this, so the
    /// ranks `dot` prints are the balanced ones** — a port that only normalises gets the
    /// right number of layers in the wrong places.
    TopBottom,
    /// `balance == 2`, `LR_balance`: halve each entering edge's slack, once, without
    /// pivoting. `dot_position` runs this over the auxiliary graph; the rank pass never
    /// does, but one engine serves both.
    LeftRight,
}

/// `rank2`'s three tunables, gathered into one so the call stays inside the four-parameter
/// limit and so a caller cannot set one without seeing the other two.
pub struct Params {
    /// Which balance pass to run at the end.
    pub balance: Balance,
    /// `maxiter`: the pivot cap, and `0` means rank what `feasible_tree` reached and stop.
    pub maxiter: i32,
    /// `search_size`: `leave_edge`'s cut-off, negative for [`SEARCH_SIZE`].
    pub search_size: i32,
}

impl Params {
    /// What `rank1` (`rank.c:453-464`) passes with no `nslimit1` attribute set: top-bottom
    /// balance, no iteration cap, the default search size.
    pub fn top_bottom() -> Self {
        Self {
            balance: Balance::TopBottom,
            maxiter: i32::MAX,
            search_size: SEARCH_SIZE,
        }
    }

    /// What `dot_position` (`position.c:142`) passes with no `nslimit` attribute set: the
    /// left-right balance, no iteration cap, the default search size.
    ///
    /// The two constructors are the whole reason this struct exists: the engine is one engine
    /// run twice over two different graphs, and a caller cannot reach one of those two
    /// configurations without seeing the other.
    pub fn left_right() -> Self {
        Self {
            balance: Balance::LeftRight,
            maxiter: i32::MAX,
            search_size: SEARCH_SIZE,
        }
    }
}

/// The simplex's own state: the reference's `network_simplex_ctx_t` minus the graph
/// pointer, which every field reaches through the `Fast` the pass is handed.
#[derive(Default)]
pub struct Ctx {
    /// `Tree_edge`: the spanning tree's edges, in the order they joined. `leave_edge`
    /// scans this in place and rotates, and `exchange` rewrites one slot of it, so the
    /// order is part of the answer.
    pub tree_edge: Vec<u32>,
    /// `S_i`, where `leave_edge` resumes — the reference's rotating search index, left
    /// where the cut-off found the edge so the next call starts nearby.
    pub s_i: usize,
    /// `N_nodes`: the component's size.
    pub n_nodes: usize,
    /// `N_edges`: the component's edge count, the reference's own diagnostic.
    pub n_edges: usize,
    /// `Search_size`, as a count rather than an attribute value.
    pub search_size: usize,
}

impl Ctx {
    /// An empty context; `init_graph` fills in the counts.
    pub fn new() -> Self {
        Self {
            tree_edge: Vec::new(),
            s_i: 0,
            search_size: SEARCH_SIZE as usize,
            ..Self::default()
        }
    }
}

/// `rank2` (`ns.c:951-1027`): rank `nodes`, one connected component, in place.
///
/// `nodes` is `GD_nlist` — the component's node list in `decompose`'s order — and it is
/// also the boundary of the pass: every walk reads a node's own adjacency lists, so a
/// component's edges never need collecting and a node outside it is never reached.
pub fn rank2(g: &mut Fast, nodes: &[u32], params: &Params) -> Result<(), Error> {
    let mut ctx = Ctx::new();
    if !init_graph(&mut ctx, g, nodes) {
        init_rank(g, nodes);
    }
    if params.search_size >= 0 {
        ctx.search_size = params.search_size as usize;
    }
    feasible_tree(g, &mut ctx, nodes)?;
    #[cfg(test)]
    checks::check(g, nodes, &ctx, "after feasible_tree");
    if params.maxiter <= 0 {
        balance::free_tree(g, nodes);
        return Ok(());
    }
    let mut iter = 0;
    while let Some(e) = leave_edge(&mut ctx, g) {
        // The reference hands `enter_edge`'s NULL straight to `update`, which would
        // dereference it. A negative cut value always has an entering edge — that is what
        // the cut value measures — so this is the reference's implicit crash guard made
        // explicit rather than a case that can happen.
        let Some(f) = enter::enter_edge(g, e) else {
            break;
        };
        pivot::update(g, &mut ctx, e, f)?;
        iter += 1;
        #[cfg(test)]
        checks::check(g, nodes, &ctx, "in the pivot loop");
        if iter >= params.maxiter {
            break;
        }
    }
    balance::run(g, &ctx, nodes, params.balance);
    Ok(())
}

/// The slack of an edge: the room it has over its minimum (`ns.c:43`).
pub(super) fn slack(g: &Fast, edge: u32) -> i32 {
    tree::slack(g, edge)
}

/// `rerank` (`ns.c:691-702`): move `v`'s whole tree subtree by `-delta`, skipping the edge
/// each node hangs from. Shared with `LR_balance`, which is the only other caller.
///
/// Explicit stack, not recursion: a subtree can be the whole component, and this port does
/// not put the reference's call depth on the machine stack. The skip is per node, so a stale
/// `par` on any one of them would walk back up and re-visit the subtree forever.
pub(super) fn rerank(g: &mut Fast, v: u32, delta: i32) {
    let mut stack = vec![(v, g.nodes[v as usize].par)];
    while let Some((node, skip)) = stack.pop() {
        g.nodes[node as usize].rank -= delta;
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

/// `init_graph` (`ns.c:890-921`): reset the per-node simplex state and report whether the
/// ranks already in hand satisfy every edge. The answer decides whether the pass starts
/// from [`init_rank`] or trusts them.
fn init_graph(ctx: &mut Ctx, g: &mut Fast, nodes: &[u32]) -> bool {
    ctx.n_nodes = 0;
    ctx.n_edges = 0;
    for &n in nodes {
        let node = &mut g.nodes[n as usize];
        node.mark = false;
        node.subtree = NO_TREE;
        node.tree_in.clear();
        node.tree_out.clear();
        ctx.n_nodes += 1;
        ctx.n_edges += g.out[n as usize].len();
    }
    let mut feasible = true;
    for &n in nodes {
        let incoming = g.inn[n as usize].clone();
        for &edge in &incoming {
            let record = &mut g.edges[edge as usize];
            record.cutvalue = 0;
            record.tree_index = -1;
            let span = g.nodes[record.head as usize].rank - g.nodes[record.tail as usize].rank;
            if span < record.minlen {
                feasible = false;
            }
        }
        g.nodes[n as usize].priority = i32::try_from(incoming.len()).expect("in-degree fits i32");
    }
    feasible
}

/// `init_rank` (`ns.c:145-177`): longest-path ranking from the sources, over a work queue
/// the in-degrees drain. A node is queued once its last in-neighbour is ranked, so its own
/// maximum is final when it is popped. Integer throughout, so the result is exact.
fn init_rank(g: &mut Fast, nodes: &[u32]) {
    let mut queue: VecDeque<u32> = nodes
        .iter()
        .copied()
        .filter(|&v| g.nodes[v as usize].priority == 0)
        .collect();
    while let Some(v) = queue.pop_front() {
        g.nodes[v as usize].rank = 0;
        for &edge in &g.inn[v as usize].clone() {
            let tail = g.edges[edge as usize].tail;
            let wanted = g.nodes[tail as usize].rank + g.edges[edge as usize].minlen;
            if wanted > g.nodes[v as usize].rank {
                g.nodes[v as usize].rank = wanted;
            }
        }
        for &edge in &g.out[v as usize].clone() {
            let head = g.edges[edge as usize].head;
            g.nodes[head as usize].priority -= 1;
            if g.nodes[head as usize].priority <= 0 {
                queue.push_back(head);
            }
        }
    }
}
