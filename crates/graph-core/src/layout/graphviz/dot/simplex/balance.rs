//! The three ways `rank2` finishes: `scan_and_normalize`, `TB_balance` and `LR_balance`
//! (`ns.c:748-796, 814-888`), and `freeTreeList` (`ns.c:766-776`).
//!
//! **Normalising is not balancing.** Every simplex answer is a ranking *up to a shift*, so
//! the first thing the pass does is put the lowest real node on rank 0. That alone is
//! `balance == 0`, and it is the only one of the three `rank1` could have used. It does not
//! use it: `rank1` passes `TB_balance` (`rank.c:462`), which additionally slides every node
//! whose in-weight equals its out-weight onto the least crowded rank it is free to take.
//! That is a real move, not a shift — it changes which nodes share a layer, and therefore
//! the drawing — and the difference between `None` and `TopBottom` is where a first port
//! gets its shapes wrong.
//!
//! `LR_balance` is the x-coordinate counterpart and belongs to `dot_position`, which runs
//! the same engine over its auxiliary graph. It is here because the engine is one engine.
//!
//! Determinism: `TB_balance` visits the nodes in rank order, and equal ranks keep the order
//! they were given in — a stable sort, which is what `LIST_SORT`'s merge sort does in the C
//! and what a `qsort` is not obliged to. Since the pass counts nodes per rank as it goes,
//! that order is an input, not an implementation detail.

use super::super::fast::{Fast, Kind};
use super::{Balance, Ctx};

/// `rank2`'s tail (`ns.c:1007-1019`): run the balance pass the parameters asked for. Each
/// of the three releases the tree lists on the way out, in the reference's way.
pub fn run(g: &mut Fast, ctx: &Ctx, nodes: &[u32], balance: Balance) {
    match balance {
        Balance::TopBottom => {
            tb_balance(g, nodes);
            free_tree(g, nodes);
        }
        Balance::LeftRight => lr_balance(g, ctx, nodes),
        Balance::None => {
            scan_and_normalize(g, nodes);
            free_tree(g, nodes);
        }
    }
}

/// `freeTreeList` (`ns.c:766-776`): the tree lists are scratch, so every pass that is done
/// with them empties them and clears the mark it was using.
pub fn free_tree(g: &mut Fast, nodes: &[u32]) {
    for &n in nodes {
        let node = &mut g.nodes[n as usize];
        node.tree_in.clear();
        node.tree_out.clear();
        node.mark = false;
    }
}

/// `scan_and_normalize` (`ns.c:748-761`): the lowest *real* node to rank 0, and the highest
/// real rank back. Virtual nodes are not counted, so a component whose sinks are all
/// virtual keeps its real nodes where the simplex put them.
///
/// A component with no real node at all has no rank to normalise against; the reference
/// reads `INT_MAX - INT_MIN` here, so this returns 0 rather than overflowing.
pub fn scan_and_normalize(g: &mut Fast, nodes: &[u32]) -> i32 {
    let mut low = i32::MAX;
    let mut high = i32::MIN;
    for &n in nodes {
        if g.nodes[n as usize].kind == Kind::Normal {
            low = low.min(g.nodes[n as usize].rank);
            high = high.max(g.nodes[n as usize].rank);
        }
    }
    if low > high {
        return 0;
    }
    for &n in nodes {
        g.nodes[n as usize].rank -= low;
    }
    high - low
}

/// `LR_balance` (`ns.c:778-796`): for every tree edge whose cut value is zero, look for an
/// entering edge and halve the slack if it is more than one. A relaxation, not a pivot: it
/// is what keeps `dot_position`'s x-coordinates close to where the rank pass put them.
fn lr_balance(g: &mut Fast, ctx: &Ctx, nodes: &[u32]) {
    for &edge in &ctx.tree_edge {
        if g.edges[edge as usize].cutvalue != 0 {
            continue;
        }
        let Some(entering) = super::pivot::enter_edge(g, edge) else {
            continue;
        };
        let delta = slack(g, entering);
        if delta <= 1 {
            continue;
        }
        let tail = g.edges[edge as usize].tail;
        let head = g.edges[edge as usize].head;
        if g.nodes[tail as usize].lim < g.nodes[head as usize].lim {
            super::pivot::rerank(g, tail, delta / 2);
        } else {
            super::pivot::rerank(g, head, -(delta / 2));
        }
    }
    free_tree(g, nodes);
}

/// `TB_balance` (`ns.c:814-888`): move every real node whose in- and out-weight agree to
/// the least populated rank it may legally sit on, visiting the nodes in rank order and
/// counting as it goes.
///
/// The legal range is `[low, high]` from the node's own edges — no in-neighbour may end up
/// above it and no out-neighbour below it — and the choice inside that range is the
/// emptiest rank, with the node's current rank as the tie-break because the scan starts at
/// `low` and only moves on a strict `<`. That is what makes a node that *could* stay where
/// it is usually stay there.
pub fn tb_balance(g: &mut Fast, nodes: &[u32]) {
    let max_rank = scan_and_normalize(g, nodes);
    let mut counts = vec![0usize; usize::try_from(max_rank + 1).expect("a rank count fits usize")];
    let mut order: Vec<u32> = nodes.to_vec();
    order.sort_by_key(|&n| g.nodes[n as usize].rank);
    for &n in &order {
        if g.nodes[n as usize].kind == Kind::Normal {
            counts[rank_slot(g, n, max_rank)] += 1;
        }
    }
    for n in order {
        if g.nodes[n as usize].kind != Kind::Normal {
            continue;
        }
        if let Some(choice) = emptiest(g, n, max_rank, &counts) {
            let from = rank_slot(g, n, max_rank);
            counts[from] -= 1;
            counts[choice] += 1;
            g.nodes[n as usize].rank = i32::try_from(choice).expect("a rank fits i32");
        }
    }
}

/// `TB_balance`'s inner move, `None` when the node's in- and out-weights differ and nothing
/// may be done to it.
fn emptiest(g: &Fast, n: u32, max_rank: i32, counts: &[usize]) -> Option<usize> {
    let mut low = 0;
    let mut high = max_rank;
    let mut in_weight = 0;
    let mut out_weight = 0;
    for &edge in &g.inn[n as usize] {
        in_weight += g.edges[edge as usize].weight;
        low = low.max(g.nodes[g.edges[edge as usize].tail as usize].rank
            + g.edges[edge as usize].minlen);
    }
    for &edge in &g.out[n as usize] {
        out_weight += g.edges[edge as usize].weight;
        high = high.min(g.nodes[g.edges[edge as usize].head as usize].rank
            - g.edges[edge as usize].minlen);
    }
    let low = usize::try_from(low).expect("a rank is never negative here");
    let high = usize::try_from(high).expect("a rank is never negative here");
    if in_weight != out_weight {
        return None;
    }
    let mut choice = low;
    for at in low + 1..=high {
        if counts[at] < counts[choice] {
            choice = at;
        }
    }
    Some(choice)
}

/// The node's current rank as a slot in the count vector. A virtual node can sit below rank
/// 0, which is why the reference clamps `low` and why this clamps the slot: the count vector
/// is indexed by real ranks only.
fn rank_slot(g: &Fast, n: u32, max_rank: i32) -> usize {
    g.nodes[n as usize]
        .rank
        .clamp(0, max_rank)
        .try_into()
        .expect("a rank fits usize")
}

/// `SLACK(e)` (`ns.c:43`).
fn slack(g: &Fast, edge: u32) -> i32 {
    let record = &g.edges[edge as usize];
    g.nodes[record.head as usize].rank - g.nodes[record.tail as usize].rank - record.minlen
}