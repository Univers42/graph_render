//! The charge deposit as two range kernels, so the workers share it and every cell still
//! sums its terms in the order one thread would.
//!
//! [`Stencils`] scales each sorted slot's position to cell units, one output per slot, the
//! only pass that reads the positions; the rest of the tick splits that value. [`Rows`]
//! groups the slots by their lower-left cell's row, each row in slot order. [`Deposit`] then divides the
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

/// A slot's position in cell units, [`frame::scaled`]: NaN for a non-finite node, which is in
/// no row, deposits nothing and reads no field.
pub(super) type Scaled = (f64, f64);

/// Each sorted slot's [`Scaled`] position.
pub(super) struct Stencils<'a> {
    pub(super) frame: &'a Frame,
    pub(super) side: usize,
    pub(super) order: &'a [u32],
    pub(super) xy: (&'a [f64], &'a [f64]),
}

impl StepRange for Stencils<'_> {
    type Out = Scaled;

    fn len(&self) -> u32 {
        self.order.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [Scaled]) {
        let order = &self.order[range.start as usize..range.end as usize];
        for (u, &i) in out.iter_mut().zip(order) {
            let i = i as usize;
            *u = frame::scaled(self.frame, (self.xy.0[i], self.xy.1[i]));
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

    /// Room for `n` slots over a mesh `side` cells wide, reusing what a same-side mesh
    /// already holds: [`sort`] counts every start from zero and writes every slot it
    /// scatters, so a longer mesh needs the room and none of the contents.
    pub(super) fn grow(&mut self, side: usize, n: u32) {
        if self.starts.len() != side + 1 {
            self.starts.resize(side + 1, 0);
        }
        self.slots.resize(n as usize, 0);
    }

    /// Regroups the slots of `at` by the row of their lower-left cell in `frame`; a
    /// non-finite slot is in no row.
    pub(super) fn sort(&mut self, at: &[Scaled], frame: &Frame) {
        self.starts.fill(0);
        for &(_, uy) in at.iter().filter(|u| !u.0.is_nan()) {
            self.starts[frame::cell(frame, uy) + 1] += 1;
        }
        for row in 1..self.starts.len() {
            self.starts[row] += self.starts[row - 1];
        }
        // Each row's start is its cursor; after the scatter it holds the next row's start,
        // so one shift right restores the starts.
        for (slot, &(ux, uy)) in (0u32..).zip(at) {
            if !ux.is_nan() {
                let cursor = &mut self.starts[frame::cell(frame, uy)];
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
    pub(super) at: &'a [Scaled],
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
        let Some(((cx, cy), (fx, fy))) = frame::split(self.stencils.frame, self.at[slot as usize])
        else {
            return;
        };
        let side = self.stencils.side;
        let at = cy * side + cx;
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
