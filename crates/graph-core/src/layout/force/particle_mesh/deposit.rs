//! The charge deposit as two range kernels, so the workers share it and every cell still
//! sums its terms in the order one thread would.
//!
//! [`Stencils`] finds each sorted slot's lower-left cell, one output per slot. [`Rows`]
//! groups the slots by that cell's row, each row in slot order. [`Deposit`] then divides the
//! cells, not the nodes: a row's cells take terms only from the slots of that row and the one
//! below, so a range merges those two lists back into slot order and adds the weights that
//! land inside it. A cell's terms arrive in slot order whichever range holds it, which is the
//! order of the one-thread loop, so the density is that loop's, bit for bit.
//!
//! Caveat: the ranges are cut by cell count, not by charge, and a range walks the whole of
//! both lists for every row it touches, even one it holds only a few cells of. A layout whose
//! nodes crowd a few rows makes those rows' ranges do most of the work and each one walk the
//! crowd again; at worst that is the old cost, every range reading every slot. Escape hatch:
//! one thread is still exact; a cut by charge needs a partition the runner does not own.

use super::fft::C;
use super::frame::{self, Frame};
use crate::exec::StepRange;
use std::ops::Range;

/// The cell of a slot whose node is not finite: it is in no row, so it deposits nothing.
const NONE: u32 = u32::MAX;

/// Each sorted slot's lower-left cell, `cy * side + cx`, or [`NONE`].
pub(super) struct Stencils<'a> {
    pub(super) frame: &'a Frame,
    pub(super) side: usize,
    pub(super) order: &'a [u32],
    pub(super) xy: (&'a [f64], &'a [f64]),
}

impl Stencils<'_> {
    fn of(&self, i: u32) -> Option<((usize, usize), (f64, f64))> {
        let i = i as usize;
        frame::stencil(self.frame, (self.xy.0[i], self.xy.1[i]))
    }
}

impl StepRange for Stencils<'_> {
    type Out = u32;

    fn len(&self) -> u32 {
        self.order.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [u32]) {
        let order = &self.order[range.start as usize..range.end as usize];
        for (cell, &i) in out.iter_mut().zip(order) {
            *cell = self
                .of(i)
                .map_or(NONE, |((cx, cy), _)| (cy * self.side + cx) as u32);
        }
    }
}

/// The finite slots grouped by their stencil's row, each row ascending: a counting sort over
/// [`Stencils`]' output, sized once per mesh so a tick allocates nothing.
pub(super) struct Rows {
    /// Row `r`'s slots are `slots[starts[r]..starts[r + 1]]`.
    starts: Vec<u32>,
    slots: Vec<u32>,
}

impl Rows {
    /// Room for `n` slots over a mesh `side` cells wide.
    pub(super) fn new(side: usize, n: u32) -> Rows {
        Rows {
            starts: vec![0; side + 1],
            slots: vec![0; n as usize],
        }
    }

    /// Regroups the slots of `at` by row; a [`NONE`] slot is in no row.
    pub(super) fn sort(&mut self, at: &[u32], side: usize) {
        self.starts.fill(0);
        for &cell in at.iter().filter(|&&cell| cell != NONE) {
            self.starts[cell as usize / side + 1] += 1;
        }
        for row in 1..self.starts.len() {
            self.starts[row] += self.starts[row - 1];
        }
        // Each row's start is its cursor; after the scatter it holds the next row's start,
        // so one shift right restores the starts.
        for (slot, &cell) in (0u32..).zip(at) {
            if cell != NONE {
                let cursor = &mut self.starts[cell as usize / side];
                self.slots[*cursor as usize] = slot;
                *cursor += 1;
            }
        }
        let last = self.starts.len() - 1;
        self.starts.copy_within(..last, 1);
        self.starts[0] = 0;
    }

    fn of(&self, row: usize) -> &[u32] {
        &self.slots[self.starts[row] as usize..self.starts[row + 1] as usize]
    }
}

/// The density over rows `0..cells`: each finite node's unit charge, CIC weighted.
pub(super) struct Deposit<'a> {
    pub(super) stencils: &'a Stencils<'a>,
    /// [`Stencils`]' output.
    pub(super) at: &'a [u32],
    /// That output grouped by row.
    pub(super) rows: &'a Rows,
}

impl StepRange for Deposit<'_> {
    type Out = C;

    fn len(&self) -> u32 {
        (self.stencils.frame.cells * self.stencils.side) as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [C]) {
        let (start, end) = (range.start as usize, range.end as usize);
        let side = self.stencils.side;
        let mut row = start / side;
        while row * side < end {
            let cells = (row * side).max(start)..((row + 1) * side).min(end);
            self.row(
                row,
                cells.clone(),
                &mut out[cells.start - start..cells.end - start],
            );
            row += 1;
        }
    }
}

impl Deposit<'_> {
    // `cells` of row `row`, whose terms come from the slots stenciled in this row and the one
    // below: both lists ascend, so merging them visits the slots in the one-thread order.
    fn row(&self, row: usize, cells: Range<usize>, out: &mut [C]) {
        let mut below = if row == 0 {
            &[][..]
        } else {
            self.rows.of(row - 1)
        };
        let mut here = self.rows.of(row);
        loop {
            let slot = match (below.first(), here.first()) {
                (Some(&a), Some(&b)) if a < b => take(&mut below, a),
                (_, Some(&b)) => take(&mut here, b),
                (Some(&a), None) => take(&mut below, a),
                (None, None) => return,
            };
            self.add(slot, &cells, out);
        }
    }

    // The weights of `slot` that land in `cells`, into `out`, which starts at `cells.start`.
    fn add(&self, slot: u32, cells: &Range<usize>, out: &mut [C]) {
        let slot = slot as usize;
        let Some((_, (fx, fy))) = self.stencils.of(self.stencils.order[slot]) else {
            return;
        };
        let (at, side) = (self.at[slot] as usize, self.stencils.side);
        for (cell, w) in [at, at + 1, at + side, at + side + 1]
            .into_iter()
            .zip(weights(fx, fy))
        {
            if cells.contains(&cell) {
                out[cell - cells.start].re += w;
            }
        }
    }
}

/// Drops the head of `list` and hands back `head`, the value it held.
fn take(list: &mut &[u32], head: u32) -> u32 {
    *list = &list[1..];
    head
}

/// The four CIC weights, in the cell order `(x, y), (x+1, y), (x, y+1), (x+1, y+1)`.
pub(super) fn weights(fx: f64, fy: f64) -> [f64; 4] {
    [
        (1.0 - fx) * (1.0 - fy),
        fx * (1.0 - fy),
        (1.0 - fx) * fy,
        fx * fy,
    ]
}

#[cfg(test)]
mod tests;
