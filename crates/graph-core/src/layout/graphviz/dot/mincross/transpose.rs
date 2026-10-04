//! The transverse pass: every neighbouring pair of one rank, considered for a swap.
//!
//! **What it looks at.** A rank's nodes are walked left to right and each *adjacent* pair is
//! asked one question: if these two changed places, would the number of crossings between
//! this rank and its neighbours go down? Only the two bands the pair touches are counted —
//! the edges arriving from above and the edges leaving below — because a swap cannot change
//! any other band. A swap happens on a strict improvement, and on a tie only in a reverse
//! pass, which is the same rule the median pass's swaps follow and for the same reason: a
//! plateau has to be broken somewhere.
//!
//! **The two counts.** For the band above, the crossings are the pairs of arriving edges
//! that are in the wrong order once the two nodes have changed places: an edge into the left
//! node and an edge into the right node cross if the left one's source is further right than
//! the right one's source. The band below is the same question on the way out. A pair that
//! crosses costs the same either way round, and a pair that crosses *both* ways is counted
//! twice, once per band — which is right, because two edges that cross do so once per band
//! they cross in.
//!
//! **Rounds until nothing moves.** A rank is a candidate for a swap in the round if it was
//! one in the previous round and moved, or if a neighbouring rank moved — so a change
//! propagates outward one rank per round. A rank that does not move drops out, and the pass
//! ends when a whole round moved nothing.
//!
//! **Nothing here consults a same-rank edge.** Two nodes of one rank joined by an edge whose
//! ends are on that rank have a fixed relative order in the drawing, and a pass that swapped
//! them would be contradicting the input. The reference asks a per-rank adjacency matrix
//! built from exactly those edges, and answers "no swap" when it finds one.
//!
//! Ponytail: this port answers "no swap" for every pair, because the matrix would be empty:
//! **none of the 1000 fixture seeds has an edge whose ends land on the same rank**, measured
//! over the probe's own rank column in `docs/measurements/p13-gv2-dot.md`. A graph that has
//! one is the direction in which this pass is wrong, and it is wrong there in the direction
//! of moving a node that the input pinned.
//!
//! Determinism: the pairs are adjacent in the rank's own window and the counts are sums over
//! adjacency lists in their own order, so every count is a sum in a fixed order
//! (`prompt.md` §6 D3).

use super::super::fast::Fast;
use super::ranks::Ranks;

/// Whether `v` and `w` are pinned in this order by an edge of their own, and so may not be
/// swapped. Always false here; see the module's `Ponytail` line.
fn order_is_pinned(_g: &Fast, _v: u32, _w: u32) -> bool {
    false
}

/// Swap neighbouring pairs in every rank until a whole round moves nothing.
pub fn transpose(g: &mut Fast, ranks: &mut Ranks, reverse: bool) {
    for r in 0..=ranks.max() {
        ranks.rows[r].candidate = true;
    }
    while {
        let mut moved = 0i64;
        for r in 0..=ranks.max() {
            if ranks.rows[r].candidate {
                moved += transpose_step(g, ranks, r, reverse);
            }
        }
        moved >= 1
    } {}
}

/// One round over one rank: what the swaps gained, and which ranks they changed.
fn transpose_step(g: &mut Fast, ranks: &mut Ranks, r: usize, reverse: bool) -> i64 {
    let (above, below) = (r > 0, ranks.len(r + 1) > 0);
    let mut gained = 0i64;
    ranks.rows[r].candidate = false;
    for i in 0..ranks.len(r).saturating_sub(1) {
        let (v, w) = (ranks.get(r, i), ranks.get(r, i + 1));
        if order_is_pinned(g, v, w) {
            continue;
        }
        let (mut here, mut swapped) = (0i64, 0i64);
        if above {
            here += crosses_from_above(g, v, w);
            swapped += crosses_from_above(g, w, v);
        }
        if below {
            here += crosses_below(g, v, w);
            swapped += crosses_below(g, w, v);
        }
        if swapped < here || (here > 0 && reverse && swapped == here) {
            ranks.swap(g, v, w);
            gained += here - swapped;
            ranks.rows[r].valid = false;
            ranks.rows[r].candidate = true;
            // A swap changes this rank and the two bands it touches, so those are the ranks
            // the next round has to look at again. `wrapping_sub` keeps rank 0 from naming a
            // rank above the top, and the bound drops the one below the bottom.
            let last = ranks.max();
            for touched in [r.wrapping_sub(1), r + 1].into_iter().filter(|t| *t <= last) {
                ranks.rows[touched].valid = false;
                ranks.rows[touched].candidate = true;
            }
        }
    }
    gained
}

/// The crossings the band above rank `r` would have with `v` left of `w`, against the two
/// exchanged. An edge into `v` crosses an edge into `w` when `v`'s comes from further right.
fn crosses_from_above(g: &Fast, v: u32, w: u32) -> i64 {
    let mut cross = 0i64;
    for &b in &g.inn[w as usize] {
        let into_w = &g.edges[b as usize];
        let source_w = g.nodes[into_w.tail as usize].order;
        let charged = i64::from(into_w.xpenalty);
        for &a in &g.inn[v as usize] {
            let into_v = &g.edges[a as usize];
            if g.nodes[into_v.tail as usize].order > source_w {
                cross += charged * i64::from(into_v.xpenalty);
            }
        }
    }
    cross
}

/// The crossings the band below rank `r` would have with `v` left of `w`, against the two
/// exchanged. An edge out of `v` crosses an edge out of `w` when `v`'s lands further right.
fn crosses_below(g: &Fast, v: u32, w: u32) -> i64 {
    let mut cross = 0i64;
    for &b in &g.out[w as usize] {
        let out_of_w = &g.edges[b as usize];
        let target_w = g.nodes[out_of_w.head as usize].order;
        let charged = i64::from(out_of_w.xpenalty);
        for &a in &g.out[v as usize] {
            let out_of_v = &g.edges[a as usize];
            if g.nodes[out_of_v.head as usize].order > target_w {
                cross += charged * i64::from(out_of_v.xpenalty);
            }
        }
    }
    cross
}