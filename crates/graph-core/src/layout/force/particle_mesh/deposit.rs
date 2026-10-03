//! The charge deposit as two range kernels, so the workers share it and every cell still
//! sums its terms in the order one thread would.
//!
//! [`Stencils`] finds each sorted slot's lower-left cell, one output per slot. [`Deposit`]
//! then divides the cells, not the nodes: a range walks every slot in grid order and adds
//! only the weights that land inside it. A cell's terms arrive in grid order whichever range
//! holds it, which is the order of the one-thread loop, so the density is that loop's, bit
//! for bit.
//!
//! Caveat: the ranges are cut by cell count, not by charge. Every range reads every slot's
//! cell (one `u32` each), and the ranges over a layout's crowded middle rows do most of the
//! adds, so a layout whose nodes crowd a few rows leaves the other workers idle. Escape
//! hatch: one thread is still exact; a cut by charge needs a partition the runner does not
//! own.

use super::fft::C;
use super::frame::{self, Frame};
use crate::exec::StepRange;
use std::ops::Range;

/// The cell of a slot whose node is not finite: it deposits nothing, and it fails every
/// range's `at < end` test before any addition can overflow.
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

/// The density over rows `0..cells`: each finite node's unit charge, CIC weighted.
pub(super) struct Deposit<'a> {
    pub(super) stencils: &'a Stencils<'a>,
    /// [`Stencils`]' output.
    pub(super) at: &'a [u32],
}

impl StepRange for Deposit<'_> {
    type Out = C;

    fn len(&self) -> u32 {
        (self.stencils.frame.cells * self.stencils.side) as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [C]) {
        let (start, end) = (range.start as usize, range.end as usize);
        let side = self.stencils.side;
        for (&at, &i) in self.at.iter().zip(self.stencils.order) {
            let at = at as usize;
            if at >= end || at + side + 1 < start {
                continue;
            }
            let Some((_, (fx, fy))) = self.stencils.of(i) else {
                continue;
            };
            for (cell, w) in [at, at + 1, at + side, at + side + 1]
                .into_iter()
                .zip(weights(fx, fy))
            {
                if (start..end).contains(&cell) {
                    out[cell - start].re += w;
                }
            }
        }
    }
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
