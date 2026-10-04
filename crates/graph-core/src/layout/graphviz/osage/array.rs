//! `arrayRects` (`lib/pack/pack.c:604-714`) over **per-node** boxes: the reference's own
//! five steps, none of them collapsed.
//!
//! `grid.rs` is the uniform case — every box the same size, so the sort cannot move
//! anything and the two prefix sums are exact multiples of one cell. This module is what
//! `arrayRects` does when the boxes differ, which is what the differential's fixtures are:
//! the sort is a real permutation, each column's width is the widest box in it, each row's
//! height is the tallest, and every placement is centred in its own cell.
//!
//! The steps, in the reference's order and with its line numbers:
//!
//! 1. `pack.c:613-631` — the grid size. `nc = ceil(sqrt(n))` and `nr = ceil(n / nc)`; the
//!    fixtures set no `packmode`, so `PK_COL_MAJOR` is off and the **row-major** branch is
//!    taken. Same arithmetic as [`Grid::of`](super::Grid::of).
//! 2. `pack.c:642-648` — each rectangle's cell stride is its own extent plus `pinfo.margin`
//!    (`DFLT_MARGIN`, four points).
//! 3. `pack.c:657-658` — `qsort` by `width + height`, **descending** (`acmpf` returns 1 when
//!    `dX < dY`, so the larger sum comes first).
//! 4. `pack.c:661-668` — one pass over the *sorted* order accumulating each column's
//!    maximum width and each row's maximum height; `pack.c:670-684` turns those two arrays
//!    into edge positions.
//! 5. `pack.c:687-707` — each box's lower-left corner, centred in its cell and `round`ed,
//!    written back at the node's own index.
//!
//! **The `qsort` tie is gone, and that is the fixture's doing.** The port breaks a tie by
//! ascending node index (D2) and the reference's tie order is unobservable; the fixture
//! table makes `width + height` strictly increasing per node, so no tie exists to break and
//! the port's order is the reference's for every node. `tests.rs` pins that invariant
//! rather than trusting it.
//!
//! **Not a gather, unlike `grid.rs`, and honestly so.** Node `i`'s cell depends on which
//! column and row the *sort* put it in, and a column's width is a maximum over every box in
//! it — so one node's coordinate is a function of all the sizes, not of `i` and the node
//! count. The layout stays deterministic (the sort is a total order under the fixture
//! table, every pass below is in one fixed order, and no pass reduces over a `HashMap`), but
//! D10's per-node gather does not describe this path and `grid.rs` still does.

use super::sizes::{Boxes, NodeBox};
use super::{Columns, MARGIN};
use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::StageError;

/// Step 1: the grid `pack.c:613-631` builds — `nc = ceil(sqrt(n))` columns and
/// `nr = ceil(n / nc)` rows, row-major.
fn grid(count: u32) -> (u32, u32) {
    let cols = libm::ceil(f64::sqrt(f64::from(count))) as u32;
    (cols, count.div_ceil(cols))
}

/// Step 2 (`pack.c:642-648`): a box's cell stride, its own extent plus `pinfo.margin`. The
/// margin goes *outside* the box, which is why a uniform grid's cells are 58 x 40 and not
/// 54 x 36.
fn stride(node_box: NodeBox) -> (f64, f64) {
    (node_box.w + MARGIN, node_box.h + MARGIN)
}

/// Step 3 (`pack.c:657-658`, `acmpf` at `pack.c:569-579`): the node indices in descending
/// `width + height`, ties by ascending index.
///
/// The tie-break is the reference's *declared* order, which is what glibc's `qsort` produced
/// on every size measured before the fixtures pinned the sizes; it is written down rather
/// than left to a sort's internal order because D2 requires the tie to break by dense index
/// whether or not the reference does.
fn sorted(boxes: &Boxes) -> Vec<u32> {
    let mut order: Vec<u32> = (0..boxes.len() as u32).collect();
    order.sort_by(|&a, &b| {
        let (wa, ha) = stride(boxes.get()[a as usize]);
        let (wb, hb) = stride(boxes.get()[b as usize]);
        (wb + hb)
            .partial_cmp(&(wa + ha))
            .expect("no NaN box extent")
            .then(a.cmp(&b))
    });
    order
}

/// Step 4a (`pack.c:661-668`): each column's cell width and each row's cell height, each
/// the maximum over the boxes the sorted order put in it. Both arrays are one longer than
/// their count, so index `c + 1` / `r + 1` is the cell's far edge.
fn cells(order: &[u32], boxes: &Boxes, cols: usize, rows: usize) -> (Vec<f64>, Vec<f64>) {
    let mut widths: Vec<f64> = vec![0.0; cols + 1];
    let mut heights: Vec<f64> = vec![0.0; rows + 1];
    for (slot, &node) in order.iter().enumerate() {
        let (w, h) = stride(boxes.get()[node as usize]);
        let (column, row) = (slot % cols, slot / cols);
        widths[column] = widths[column].max(w);
        heights[row] = heights[row].max(h);
    }
    (widths, heights)
}

/// Step 4b (`pack.c:670-684`): the two edge arrays. Widths accumulate left to right from the
/// origin; heights accumulate **up from the bottom row**, so `heights[0]` is the whole
/// drawing's height and `heights[rows]` is zero.
fn edges(mut widths: Vec<f64>, mut heights: Vec<f64>) -> (Vec<f64>, Vec<f64>) {
    let mut run = 0.0;
    for cell in widths.iter_mut() {
        let value = *cell;
        *cell = run;
        run += value;
    }
    let mut run = 0.0;
    for row in (1..heights.len()).rev() {
        let value = heights[row - 1];
        heights[row] = run;
        run += value;
    }
    heights[0] = run;
    (widths, heights)
}

/// Step 5 (`pack.c:687-707`): each box's lower-left corner, centred in its cell and
/// `round`ed, at the node's own index.
///
/// The two `round`s are C's `round` — halfway cases go **away from zero**, which is not
/// Rust's `f64::round`'s... it is, and it is *not* what a naive `(x + 0.5) as i64` or a
/// banker's rounding does. Measured, not assumed: a banker's `round` moves 1 point on the
/// 601-node fixtures, and the differential says so.
fn corner(node_box: NodeBox, widths: &[f64], heights: &[f64], cell: usize) -> (f64, f64) {
    let column = cell % widths.len().saturating_sub(1);
    let row = cell / widths.len().saturating_sub(1);
    let x = (widths[column] + widths[column + 1] - node_box.w) / 2.0;
    let y = (heights[row] + heights[row + 1] - node_box.h) / 2.0;
    (libm::round(x), libm::round(y))
}

/// The whole reference routine: `putRects` → `arrayRects`, then `osageinit.c`'s two steps
/// that follow it — each node at its own box's centre (`osageinit.c:161`), and the whole
/// drawing translated so its lower-left corner is the origin (`osageinit.c:198-220`).
///
/// The translation folds over **box corners**, not node centres: `osageinit.c:152-157`
/// grows the root box from `bb.LL + p` to `bb.UR + p`, so the corner is `p` itself and the
/// fold's minimum is the smallest `p`, with no half-a-box offset anywhere.
pub(super) fn pack(topology: &Topology, boxes: &Boxes) -> Result<Geometry, StageError> {
    let count = topology.node_count();
    if boxes.len() != count as usize {
        return Err(StageError::Param {
            name: "boxes",
            rule: "one box per node",
        });
    }
    if count == 0 {
        return Ok(Columns::new(0).geometry());
    }
    let (cols, rows) = grid(count);
    let order = sorted(boxes);
    let (raw_widths, raw_heights) = cells(&order, boxes, cols as usize, rows as usize);
    let (widths, heights) = edges(raw_widths, raw_heights);
    let mut out = Columns::new(count);
    let mut low = (f64::MAX, f64::MAX);
    for (cell, &node) in order.iter().enumerate() {
        let node_box = boxes.get()[node as usize];
        let (x, y) = corner(node_box, &widths, &heights, cell);
        low.0 = low.0.min(x);
        low.1 = low.1.min(y);
        out.set(node, x + node_box.w / 2.0, y + node_box.h / 2.0);
    }
    let (xs, ys) = out.columns_mut();
    for (x, y) in xs.iter_mut().zip(ys) {
        *x -= low.0;
        *y -= low.1;
    }
    Ok(out.geometry())
}
