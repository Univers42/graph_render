//! `set_ycoords` (`position.c:730-819`): the y coordinate of every rank, from the tallest
//! node on each rank and the gap between ranks.
//!
//! The pass is two measurements and one copy:
//!
//! 1. **Rank heights.** Every rank's half-height is the largest `node height / 2` on it. The
//!    reference keeps two of each — `ht1`/`ht2` for everything on the rank and `pht1`/`pht2`
//!    for the primitive (non-dummy) nodes only — and here they are one number, because this
//!    port has no cluster whose label would push the two apart.
//! 2. **Rank lines.** The *lowest* rank's line is its own half-height above the origin, and
//!    every rank above it is the rank below plus the two half-heights and `ranksep`. Rank 0
//!    ends up at the **top**, which is the whole direction of this pass: the rank pass numbers
//!    layers downwards and the drawing is printed with the largest y at the top.
//! 3. **The copy.** Every node takes the line of the rank it sits on, so a rank is one
//!    arithmetic series rather than one per node.
//!
//! Integer where the reference has it: none of it. The rank heights and the gap are `double`
//! there and here, and the *only* integers in the pass are the rank numbers.
//!
//! Ponytail: the gap between two ranks is `ht2(below) + ht1(above) + ranksep`, which is the
//! reference's `d0`; it also computes a `d1` for a cluster boundary and takes the larger. With
//! no clusters `d1` is the same two half-heights plus a fixed eight points, so `d0` is always
//! the larger and `d1` drops out. Failing input: a DOT graph with `subgraph cluster_*`.
//! Direction: cluster rows would be spaced by their label's height rather than by this gap.
//! Escape hatch: none, the motor's topology is a flat node set.
//!
//! Ponytail: an empty rank. The reference leaves such a rank's line unset and then reads the
//! *previous* rank's node back to continue the chain, which is a coordinate nobody wrote; here
//! the chain of lines is unbroken and only the per-node copy skips a row with nothing on it.
//! Failing input: a graph the rank pass leaves with a gap — `TB_balance` moves a node, it never
//! empties a rank the simplex filled, so this is a guard and not an expected shape. Direction:
//! none on the drawing; the two agree wherever a rank is occupied. Escape hatch: none needed.

use super::Rows;
use super::super::RANKSEP;
use super::super::fast::Fast;

/// The half-height of `row`: the tallest node on it, halved. The reference's `ht1` and `ht2`
/// are this number — it assumes the box is symmetric about its centre line, which every node
/// box this port builds is.
pub fn half_height(g: &Fast, row: &[u32]) -> f64 {
    row.iter()
        .map(|&node| g.nodes[node as usize].ht / 2.0)
        .fold(0.0, f64::max)
}

/// Give every node its rank's y coordinate: measure the ranks, stack the lines, copy them out.
pub fn run(g: &mut Fast, rows: &Rows) {
    let lines = rank_lines(g, rows);
    for (r, row) in rows.iter().enumerate() {
        for &node in row {
            g.nodes[node as usize].coord.y = lines[r];
        }
    }
}

/// The y coordinate of every rank's line, indexed by rank.
///
/// The lowest rank sits at its own half-height and each rank above it is one gap higher, so
/// the answer is read from the bottom up and the gap of a rank is the two half-heights around
/// it plus `ranksep`.
fn rank_lines(g: &Fast, rows: &Rows) -> Vec<f64> {
    let heights: Vec<f64> = rows.iter().map(|row| half_height(g, row)).collect();
    let mut lines = vec![0.0; rows.len()];
    let Some(bottom) = rows.highest_rank() else {
        return lines;
    };
    lines[bottom] = heights[bottom];
    for r in (0..bottom).rev() {
        lines[r] = lines[r + 1] + heights[r + 1] + heights[r] + RANKSEP;
    }
    lines
}
