//! Counting the crossings between one band of two ranks, and the total over the graph.
//!
//! **The count is over the chain, not over the drawn edges.** A long edge is several links
//! through the chain's dummies, and each link carries the penalty the crossing is charged
//! against. Two edges therefore cross as many times as they do pairwise in the drawing,
//! which is the number the pass minimises and the number the sweep is compared on.
//!
//! **The sweep.** Rank `r`'s nodes are walked left to right. Every down edge of the node
//! being walked charges itself for each earlier edge that lands to the *right* of it on the
//! rank below — so an edge is charged once per crossing, against the edges that came before
//! it. The `count` array is the running total of penalties per landing slot, so the charge
//! for an edge landing at slot `k` is one sweep over the slots to the right of `k` that
//! anything has already reached, and `max` is the furthest slot reached so far, which is what
//! bounds that sweep. Charging before inserting is what stops an edge being charged against
//! itself.
//!
//! **Two edges landing on the same slot do not cross.** There is no tie-break there because
//! there is none to have: two edges from one node onto the same rank are the same edge.
//! Ports are the one place this would need one — the reference counts the inversions of two
//! edges *out of the same node* against each other's port positions separately, which is the
//! port-local half of the count — and this port has no ports, so that term is absent.
//!
//! Determinism: the sweep is over dense node order and the accumulation is in that order, so
//! the count is a sum in a fixed order (`prompt.md` §6 D3, D10).

use super::super::fast::{Fast, Kind};
use super::ranks::{Ranks, max_rank, row_of};

/// The crossings between one rank and the rank below it.
///
/// `top` is the upper rank's nodes in their order, `bottom` the lower rank's. The `count`
/// array is indexed by landing slot, so it needs one more slot than the lower rank has nodes.
pub fn rcross(g: &Fast, top: &[u32], bottom: &[u32]) -> i64 {
    let mut count = vec![0i64; bottom.len() + 1];
    let mut cross = 0i64;
    let mut max = 0usize;
    for &node in top {
        if max > 0 {
            for &edge in &g.out[node as usize] {
                let (landed, penalty) = landing(g, edge);
                if landed < max {
                    let reached = count[(landed + 1)..=max].iter().sum::<i64>();
                    cross += reached * i64::from(penalty);
                }
            }
        }
        for &edge in &g.out[node as usize] {
            let (landed, penalty) = landing(g, edge);
            max = max.max(landed);
            count[landed] += i64::from(penalty);
        }
    }
    cross
}

/// Where one down edge lands on the rank below, and what it is charged at: its node's
/// position within that rank, and its penalty.
fn landing(g: &Fast, edge: u32) -> (usize, i32) {
    let record = &g.edges[edge as usize];
    (
        g.nodes[record.head as usize].order as usize,
        record.xpenalty,
    )
}

/// The total crossings of the graph, one cached count per band.
///
/// The cache is what makes the sweep affordable — the median pass asks for the count after
/// every swap — and every swap that can change a band clears that band's flag, so a cached
/// count is only ever read while the two ranks it spans are exactly as they were counted.
pub fn ncross(g: &Fast, ranks: &mut Ranks) -> i64 {
    let mut total = 0i64;
    for r in 0..max_rank(g) {
        if ranks.rows[r].valid {
            total += ranks.rows[r].cache_nc;
            continue;
        }
        let count = rcross(g, ranks.window(r), ranks.window(r + 1));
        ranks.rows[r].cache_nc = count;
        ranks.rows[r].valid = true;
        total += count;
    }
    total
}

/// The total number of crossings in the order `g` is currently in.
///
/// This is the pass's output and the number the 1000-seed sweep compares, so it reads the
/// order off the nodes themselves rather than off a cache: the answer is a property of the
/// order, not of this function's bookkeeping.
pub fn crossings(g: &Fast) -> i64 {
    let rows = rank_rows(g);
    (0..rows.len().saturating_sub(1))
        .map(|r| rcross(g, &rows[r], &rows[r + 1]))
        .sum()
}

/// Every rank's nodes in their current order, gathered once.
///
/// This is the whole of the pass's output read back: one row per rank, each in the order its
/// nodes are numbered in. The sort key is the node's own number within its rank, which the
/// pass leaves distinct, so the key is a total order and the sort's tie-break never has to be
/// specified (`prompt.md` §6 D5).
pub fn rank_rows(g: &Fast) -> Vec<Vec<u32>> {
    let mut rows: Vec<Vec<u32>> = vec![Vec::new(); max_rank(g) + 1];
    for node in 0..u32::try_from(g.nodes.len()).expect("a node count fits u32") {
        rows[row_of(g, node)].push(node);
    }
    for row in &mut rows {
        row.sort_by_key(|&node| g.nodes[node as usize].order);
    }
    rows
}
/// Every real node's rank, the chain dummies dropped.
///
/// The dummies exist only because a long edge needs a node on each rank it crosses, so a
/// row read after the chains are built has more nodes in it than the drawing has; this is
/// the same list the rank pass alone produces, which is what makes the two comparable.
pub fn real_ranks(g: &Fast) -> Vec<i32> {
    g.nodes
        .iter()
        .filter(|node| node.kind == Kind::Normal)
        .map(|node| node.rank)
        .collect()
}

/// Every rank's **real** nodes in their current order, the chain dummies dropped.
///
/// This is the same thing the oracle's own order column holds — the plain format prints no
/// dummy — so a row read through here and a row the probe wrote are directly comparable, and
/// a crossing count computed from either is the same count.
pub fn real_rows(g: &Fast) -> Vec<Vec<u32>> {
    let rows = rank_rows(g);
    let mut real: Vec<Vec<u32>> = vec![Vec::new(); rows.len()];
    for (r, row) in rows.iter().enumerate() {
        real[r] = row
            .iter()
            .copied()
            .filter(|&node| g.nodes[node as usize].kind == Kind::Normal)
            .collect();
    }
    real
}
