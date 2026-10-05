//! The frame: `dot_compute_bb` (`position.c:831-876`) followed by `translate_drawing`
//! (`postproc.c:153-168`) at the default `rankdir=TB`.
//!
//! At the default ratio `set_aspect` (`position.c:904-971`) is `rec_bb` and nothing else — no
//! `ratio_kind`, no drawing size, so no rescale — so this module is that whole function and not
//! a part of it.
//!
//! Every coordinate so far is relative to nothing in particular: the x simplex is free to put
//! the drawing anywhere along its line, and `set_ycoords` counts up from the lowest rank's own
//! half-height. `-Tplain` prints a coordinate whose node's **box** has its lower-left corner at
//! the origin, so the last thing the pipeline does is shift everything by the drawing's own
//! lower-left corner. This is not presentation: the closed cases are compared byte for byte
//! against what `-Tplain` printed, so the shift has to be applied on this side too or nothing
//! can match.
//!
//! Two numbers make the shift, both measured from real nodes only:
//!
//! - **x**: the smallest `coord.x - lw` over every rank's leftmost *real* node, paired with the
//!   largest `coord.x + rw` over every rank's rightmost one. A chain dummy is skipped when
//!   measuring the edge of the drawing, because a dummy has a one-point box and would put the
//!   frame inside the gutter between two real boxes.
//! - **y**: zero, because `set_ycoords` already put the lowest rank's line at its own
//!   half-height — see [`ycoords`] — and the reference's own `GD_ht1` for a root graph with no
//!   clusters is that same half-height, so `coord.y - GD_ht1` is zero for the bottom rank and
//!   the shift moves nothing vertically.
//!
//! **The vertical zero is measured, not read.** One node on the default box prints its centre
//! at `(27, 18)` points and a one-node graph with `height=1` prints `(27, 36)`; both are the
//! rank line `set_ycoords` produced, unchanged, which is what a zero shift says. A graph with a
//! cluster is the input that would move it, and this port has no clusters.
//!
//! Determinism: the minimum and the maximum are over the ranks in rank order and each rank's
//! own slot order, with the node index as the tie-break, so the frame is a function of the
//! drawing and not of a traversal (`prompt.md` §6 D2, D5).

use super::super::fast::Fast;
use super::Rows;
use super::ycoords;

/// The drawing's own lower-left corner, in points: `(x, y)`.
pub fn lower_left(g: &Fast, rows: &Rows) -> (f64, f64) {
    let mut low = f64::INFINITY;
    for row in rows.iter() {
        let Some(node) = Rows::first_real(row, g) else {
            continue;
        };
        low = low.min(g.nodes[node as usize].coord.x - g.nodes[node as usize].lw);
    }
    (if low.is_finite() { low } else { 0.0 }, 0.0)
}

/// The drawing's own upper-right corner, in points: `(x, y)`. The mirror of [`lower_left`], and
/// kept because it is what `-Tplain`'s own `graph` line is — the width and height it prints are
/// this corner less [`lower_left`].
pub fn upper_right(g: &Fast, rows: &Rows) -> (f64, f64) {
    let mut high = f64::NEG_INFINITY;
    for row in rows.iter() {
        let Some(node) = Rows::last_real(row, g) else {
            continue;
        };
        high = high.max(g.nodes[node as usize].coord.x + g.nodes[node as usize].rw);
    }
    let top = rows
        .lowest_rank()
        .map_or(0.0, |r| g.nodes[rows.row(r)[0] as usize].coord.y);
    (if high.is_finite() { high } else { 0.0 }, top)
}

/// `translate_drawing` at `rankdir=TB`: move every node's centre so the drawing's lower-left
/// corner is the origin. The reference also rotates and rewrites the node's box half-widths
/// when `rankdir` is not `TB`; at `TB` it only shifts, so this is the whole of it.
pub fn run(g: &mut Fast, rows: &Rows) {
    let (x, y) = lower_left(g, rows);
    for node in &mut g.nodes {
        node.coord.x -= x;
        node.coord.y -= y;
    }
}

/// The height of the lowest rank's line above the origin: the reference's `LL.y`, kept so a test
/// can state the zero shift as an equality rather than as a constant.
///
/// `Ponytail:` the reference subtracts the *graph's* reserved height below its lowest rank, which
/// only a cluster label or a root graph label ever makes non-zero. Failing input: a DOT graph
/// with `subgraph cluster_*` or a graph `label`. Direction: a cluster's box would sit below the
/// drawing's own boxes and the shift would move the nodes up by the cluster's border. Escape
/// hatch: none, the motor's topology is a flat node set.
pub fn lowest_offset(g: &Fast, rows: &Rows) -> f64 {
    let Some(r) = rows.highest_rank() else {
        return 0.0;
    };
    let row = rows.row(r);
    g.nodes[row[0] as usize].coord.y - ycoords::half_height(g, row)
}
