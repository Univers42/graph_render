//! The median pass: where each node would rather be, then every swap towards it.
//!
//! **The value.** Each node of the rank being ordered is given one number: where the middle
//! of its neighbours on the rank above (or below) sit. A node with one neighbour takes that
//! neighbour's number, a node with two takes their average, and a node with three or more
//! takes the middle of its sorted neighbour numbers — or, when the count is even, the
//! *weighted* middle, which is the number that would move the node the least total
//! distance. The weighting is by how far the two candidates are from the ends of the range:
//! a gap that is short pulls the answer less than a gap that is long, and when the two gaps
//! are equal the plain average is what comes out.
//!
//! **The scale.** A neighbour's number is its own position multiplied by 256, because the
//! weighted middle is a ratio of two differences of these numbers and integer differences
//! would throw away the fraction that makes it different from the plain middle. 256 is the
//! reference's scale and it is large enough that rounding it away is not possible.
//!
//! **A node with no neighbour has no opinion, and that is not the same as having one.** It
//! gets the sentinel -1, which is below every real number, and it is never moved by the
//! pass; only its position relative to a node that *does* have an opinion can change it. A
//! node keeps -1 also when its only neighbours are same-rank edges, which carry no crossing
//! weight — the median pass never reads those, because an edge inside a rank cannot say
//! which way a node should move.
//!
//! **The swaps.** The rank is walked once per position, and at each position the walk skips
//! the nodes with no opinion, compares the first node that has one with the next node that
//! has one after it, and swaps them if the left one would rather be further right. The walk
//! continues from the node it stopped at, not from the one it swapped, so each node takes
//! part in at most one comparison per pass. A reverse sweep swaps equal numbers too, which
//! is what makes the alternating sweeps escape a plateau.
//!
//! **The loop bound is the fixed-point one.** The outer loop counts down over the rank, and
//! the comparison window shrinks by one at the end of every round unless the rank holds a
//! node with no opinion, in which case it must not shrink: that node is the one that stops
//! the walk early, and a shorter window would stop it earlier still.
//!
//! Determinism: the value is a median over a list built in adjacency-list order and sorted by
//! value, the walk is over the rank's own window, and nothing here reads a clock, a hash
//! order or a random number (`prompt.md` §6 D1-D10).

use super::super::fast::Fast;
use super::ranks::Ranks;

/// The scale a neighbour's position is multiplied by before it is used as a value.
pub const MC_SCALE: i32 = 256;

/// The value of a node with no opinion: below every real value, and never moved.
const NO_OPINION: f64 = -1.0;

/// Give every node of rank `r0` its median value, and say whether any node was left without
/// an opinion. `r1` is the neighbouring rank the values come from: below `r0` for a downward
/// sweep, above it for an upward one.
pub fn medians(g: &mut Fast, ranks: &mut Ranks, r0: usize, r1: usize) -> bool {
    let downward = r1 > r0;
    let mut fixed = false;
    for i in 0..ranks.len(r0) {
        let node = ranks.get(r0, i);
        let values = neighbour_values(g, node, downward, ranks.scratch());
        g.nodes[node as usize].mval = median_of(values);
        // A node with no edge at all keeps no opinion whatever, and is what makes the swap
        // walk's window stop shrinking.
        fixed |= g.out[node as usize].is_empty() && g.inn[node as usize].is_empty();
    }
    fixed
}

/// The values of the neighbours of `node` on the rank the sweep is coming from, as scaled
/// positions.
///
/// Only edges that carry a crossing penalty contribute: an edge that does not is one the
/// crossing count ignores, so the pass has no reason to order by it.
fn neighbour_values<'a>(g: &Fast, node: u32, downward: bool, out: &'a mut Vec<i32>) -> &'a [i32] {
    out.clear();
    let edges = if downward { &g.out[node as usize] } else { &g.inn[node as usize] };
    for &edge in edges {
        let record = &g.edges[edge as usize];
        if record.xpenalty > 0 {
            let other = if downward { record.head } else { record.tail };
            out.push(MC_SCALE * g.nodes[other as usize].order);
        }
    }
    out
}

/// The median of `values`, in the reference's four cases.
///
/// The average of two values is taken in integers, exactly as the reference takes it: two
/// neighbouring positions differ by a multiple of the scale, so the division is exact for
/// every input the pass can produce and the truncation never shows. The weighted middle of
/// an even count is a ratio, and is the one place a non-integer appears.
fn median_of(values: &[i32]) -> f64 {
    let count = values.len();
    match count {
        0 => NO_OPINION,
        1 => f64::from(values[0]),
        2 => f64::from((values[0] + values[1]) / 2),
        _ => weighted_median(values, count),
    }
}

/// The middle of three or more values: the middle one for an odd count, and the middle
/// weighted by the distance to each end for an even one.
///
/// The sort is by value, and equal values are indistinguishable afterwards, so which of two
/// equal entries ends up at which index does not change the answer (`prompt.md` §6 D5).
fn weighted_median(values: &[i32], count: usize) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    if count % 2 == 1 {
        return f64::from(sorted[count / 2]);
    }
    let (low, high) = (sorted[count / 2 - 1], sorted[count / 2]);
    let left = i64::from(low - sorted[0]);
    let right = i64::from(sorted[count - 1] - high);
    if left == right {
        return f64::from((low + high) / 2);
    }
    // The two spans are the weights, so this is the middle pulled towards the nearer end.
    let weighted = f64::from(low) * right as f64 + f64::from(high) * left as f64;
    weighted / (left + right) as f64
}

/// Whether the sweep swaps equal values too, and whether the rank holds a node with no
/// opinion. The two travel together because both come from the sweep, not from the rank.
pub struct Sweep {
    /// A reverse sweep swaps two nodes that want the same place.
    pub reverse: bool,
    /// Some node of the rank has no opinion, so the comparison window does not shrink.
    pub fixed: bool,
}

/// Move every node of rank `r` towards the value the median pass gave it, returning whether
/// anything moved.
///
/// There are as many rounds as the rank has nodes, whatever the walk does with them: the
/// walk may converge long before that, and stopping early would be a different answer from
/// the one the round count gives.
pub fn reorder(g: &mut Fast, ranks: &mut Ranks, r: usize, sweep: &Sweep) -> bool {
    let rounds = ranks.len(r);
    let mut end = rounds;
    let mut changed = false;
    for _ in 0..rounds {
        changed |= one_round(g, ranks, r, &mut end, sweep);
    }
    if changed {
        ranks.rows[r].valid = false;
        if r > 0 {
            ranks.rows[r - 1].valid = false;
        }
    }
    changed
}

/// One pass of the swap walk over rank `r`, narrowing `end` by one unless the rank holds a
/// node with no opinion.
fn one_round(g: &mut Fast, ranks: &mut Ranks, r: usize, end: &mut usize, sweep: &Sweep) -> bool {
    let mut changed = false;
    let mut left = 0;
    while left < *end {
        while left < *end && g.nodes[ranks.get(r, left) as usize].mval < 0.0 {
            left += 1;
        }
        if left >= *end {
            break;
        }
        let mut right = left + 1;
        while right < *end && g.nodes[ranks.get(r, right) as usize].mval < 0.0 {
            right += 1;
        }
        if right < *end {
            let (l, rr) = (ranks.get(r, left), ranks.get(r, right));
            let (mine, theirs) = (g.nodes[l as usize].mval, g.nodes[rr as usize].mval);
            if mine > theirs || (mine >= theirs && sweep.reverse) {
                ranks.swap(g, l, rr);
                changed = true;
            }
        }
        left = right;
    }
    if !sweep.fixed && !sweep.reverse {
        *end -= 1;
    }
    changed
}