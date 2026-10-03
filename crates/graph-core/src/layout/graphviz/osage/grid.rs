//! The grid `arrayRects` builds (`lib/pack/pack.c:604-714`), in the reference's own steps.
//!
//! The reference makes three passes over the rectangles: a `qsort` by height + width, a
//! pass accumulating each column's width and each row's height, and a pass turning those
//! two arrays into edge positions. All three collapse here, because **every rectangle is
//! the same `54 x 36` point box**: the sort compares `width + height` and finds them all
//! equal, so it cannot change which rectangle lands in which cell (it does decide *which*
//! node lands in which cell, and that is fixed by the declaration order — the reference's
//! own `qsort` is stable for equal keys on glibc, which is measured in
//! `docs/measurements/p13-gv1-osage.md`).
//!
//! So the column widths are `CELL_W` each and the row heights `CELL_H` each, the two
//! prefix sums are exact multiples of them, and the reference's `round` on each placement
//! is the identity on an even integer. [`Grid::rect`] is that arithmetic in closed form;
//! `tests.rs` pins it against the sums themselves, which is what keeps this a port of
//! `arrayRects` rather than a second formula that happens to agree.

use super::{CELL_H, CELL_W, Columns, NODE_H, NODE_W};

/// A near-square grid of cells, filled row by row.
pub(super) struct Grid {
    /// Cells per row.
    pub(super) cols: u32,
    /// Rows of cells.
    pub(super) rows: u32,
}

impl Grid {
    /// The reference's grid size: `nc = ceil(sqrt(count))`, `nr = ceil(count / nc)`
    /// (`pack.c:631-633`). The fixtures set no `packmode`, so `PK_COL_MAJOR` is off and
    /// the **row-major** branch is the one taken: the column count comes from the square
    /// root and the row count is whatever is left over, so a 5-node graph is three columns
    /// by two rows and a 2-node graph is two columns by one row. `sz` is 0 because no
    /// `packmode` carries a count. The empty graph has no grid at all — the reference
    /// returns before packing.
    pub(super) fn of(count: u32) -> Self {
        if count == 0 {
            return Self { cols: 0, rows: 0 };
        }
        let side = libm::ceil(f64::sqrt(f64::from(count))) as u32;
        Self {
            cols: side,
            rows: count.div_ceil(side),
        }
    }

    /// The lower-left corner of node `index`'s box, before the drawing is moved onto the
    /// origin — the reference's `places[index]` (`pack.c:688-707`).
    ///
    /// `pack.c:672-684` turns the column widths into a prefix sum and the row heights into
    /// a suffix sum counted **up from the bottom row**, then centres each box in its cell:
    /// `x = round((widths[c] + widths[c+1] - w) / 2)` and
    /// `y = round((heights[r] + heights[r+1] - h) / 2)`. With `widths[c] = CELL_W * c` and
    /// `heights[r] = CELL_H * (rows - r)` those are `(CELL_W - NODE_W) / 2` past the cell's
    /// left edge and `(CELL_H + NODE_H) / 2` below its top edge. Both are whole numbers, so
    /// the `round` never rounds and this is `CELL_W`-exact — no tolerance anywhere.
    pub(super) fn rect(&self, index: u32) -> (f64, f64) {
        let column = f64::from(index % self.cols);
        let rows_down = f64::from(self.rows - index / self.cols);
        (
            CELL_W * column + (CELL_W - NODE_W) / 2.0,
            CELL_H * rows_down - (CELL_H + NODE_H) / 2.0,
        )
    }

    /// The gather (D10): every node at its own box's centre, and the lower-left corner of
    /// the packed drawing, which `osageinit.c:198-220` then translates to the origin.
    ///
    /// The fold is a `min` written in dense-index order. `min` is order-independent, so
    /// this is not a load-bearing reduction; it is written in index order anyway so that
    /// the bytes do not depend on how the loop was sliced.
    pub(super) fn fill(&self, out: &mut Columns) -> (f64, f64) {
        let mut low = (f64::MAX, f64::MAX);
        for index in 0..out.len() {
            let (x, y) = self.rect(index);
            out.set(index, x + NODE_W / 2.0, y + NODE_H / 2.0);
            low.0 = low.0.min(x);
            low.1 = low.1.min(y);
        }
        if out.len() == 0 {
            return (0.0, 0.0);
        }
        low
    }
}
