//! Where the mesh sits this tick: its cell size, its origin, how many cells the nodes
//! occupy and how far the kernel reaches.
//!
//! The cell size is on a fixed ladder, `h = 2^(step/4)`, so it moves in quarter-octaves
//! and the kernel spectrum is rebuilt only when the layout's span crosses a rung, not on
//! every tick. The step chosen is the smallest one whose occupied cells `cells` plus the
//! kernel's reach `reach` fit in the side `P`: then no two offsets a convolution needs
//! alias each other modulo `P`, so the circular FFT convolution is the linear one.
//!
//! The origin is snapped to a multiple of `h`, so a node's cell and weights depend on its
//! own position and the rung, never on the subtraction order of a running minimum.

use crate::exec::{Runner, StepRange};
use std::ops::Range;

/// The smallest step tried: `h = 2^-8`. A tighter cluster than that is cut no finer.
const STEP_MIN: i32 = -32;

/// Nodes per block of the bounds fold: a block is one output of [`Blocks`], so the scratch
/// is `n / 4096` boxes.
pub(super) const BLOCK: u32 = 4096;

/// A bounding box, `(lo, hi)`.
pub(super) type Bounds = ((f64, f64), (f64, f64));

/// The box no position has widened yet.
const EMPTY: Bounds = (
    (f64::INFINITY, f64::INFINITY),
    (f64::NEG_INFINITY, f64::NEG_INFINITY),
);

/// The mesh's placement for one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Frame {
    /// The rung: `h = 2^(step/4)`.
    pub(super) step: i32,
    /// The cell size.
    pub(super) h: f64,
    /// The world position of cell `(0, 0)`.
    pub(super) origin: (f64, f64),
    /// Cells per axis the deposit may touch: indices `0..cells`.
    pub(super) cells: usize,
    /// The kernel's reach in cells per axis: offsets `-reach..=reach`.
    pub(super) reach: usize,
}

/// The frame over the finite positions' bounds `(lo, hi)`, for a mesh of `side` cells per
/// axis and a force that vanishes at distance `dmax`. `None` when the finite positions span
/// more than an `f64` holds.
pub(super) fn place((lo, hi): Bounds, side: usize, dmax: f64) -> Option<Frame> {
    let span = f64::max(hi.0 - lo.0, hi.1 - lo.1);
    if !span.is_finite() {
        return None;
    }
    let mut step = ((4.0 * libm::log2(span / side as f64)).floor() as i32).max(STEP_MIN);
    loop {
        if let Some(frame) = fit(step, span, side, dmax) {
            let snap = |v: f64| libm::floor(v / frame.h) * frame.h;
            return Some(Frame {
                origin: (snap(lo.0), snap(lo.1)),
                ..frame
            });
        }
        step += 1;
    }
}

/// The frame at rung `step` with the origin still unset, if it fits in `side`.
///
/// `cells` is `floor(span / h) + 3`: the origin snaps down by less than `h`, so the far
/// node's cell index is at most `floor(span / h) + 1`, and its CIC stencil reaches one
/// further. The deposit clamps to the same range, so a rounding at the boundary moves a
/// weight by one cell edge rather than past the end.
fn fit(step: i32, span: f64, side: usize, dmax: f64) -> Option<Frame> {
    let h = libm::exp2(f64::from(step) / 4.0);
    let cells = libm::floor(span / h) + 3.0;
    if cells > side as f64 {
        return None;
    }
    let cells = cells as usize;
    let reach = (libm::ceil(dmax / h) as usize).min(cells - 1);
    (cells + reach <= side).then_some(Frame {
        step,
        h,
        origin: (0.0, 0.0),
        cells,
        reach,
    })
}

/// The finite positions' bounding box, `None` if there is none, folded per block of
/// [`BLOCK`] nodes on `runner`'s workers, the blocks then folded in order. `blocks` is the
/// caller's scratch.
pub(super) fn bounds<R: Runner>(
    xy: (&[f64], &[f64]),
    runner: &R,
    workers: u32,
    blocks: &mut Vec<Bounds>,
) -> Option<Bounds> {
    runner.run(&Blocks { xy }, workers, blocks);
    let (lo, hi) = blocks.iter().fold(EMPTY, |acc, &b| widen(acc, b));
    (lo.0 <= hi.0).then_some((lo, hi))
}

/// Each block's bounds over its finite positions, [`EMPTY`] for a block with none.
struct Blocks<'a> {
    xy: (&'a [f64], &'a [f64]),
}

impl StepRange for Blocks<'_> {
    type Out = Bounds;

    fn len(&self) -> u32 {
        (self.xy.0.len() as u32).div_ceil(BLOCK)
    }

    fn step_range(&self, range: Range<u32>, out: &mut [Bounds]) {
        let n = self.xy.0.len();
        for (b, slot) in range.zip(out) {
            let start = b as usize * BLOCK as usize;
            let nodes = start..(start + BLOCK as usize).min(n);
            let (x, y) = (&self.xy.0[nodes.clone()], &self.xy.1[nodes]);
            *slot = x.iter().zip(y).fold(EMPTY, |acc, (&px, &py)| {
                let finite = px.is_finite() && py.is_finite();
                if finite {
                    widen(acc, ((px, py), (px, py)))
                } else {
                    acc
                }
            });
        }
    }
}

/// `acc` grown to cover `b`. The comparisons are strict, so of two equal ends the earlier
/// one stays: one fold over every node and the fold of its blocks' folds keep the same
/// one, `-0.0` against `+0.0` included, so the bounds do not depend on the worker count.
fn widen(acc: Bounds, b: Bounds) -> Bounds {
    let pick = |keep: f64, other: f64, wins: bool| if wins { other } else { keep };
    let lo = (
        pick(acc.0.0, b.0.0, b.0.0 < acc.0.0),
        pick(acc.0.1, b.0.1, b.0.1 < acc.0.1),
    );
    let hi = (
        pick(acc.1.0, b.1.0, b.1.0 > acc.1.0),
        pick(acc.1.1, b.1.1, b.1.1 > acc.1.1),
    );
    (lo, hi)
}

/// `p` in cell units from the origin, `(NaN, NaN)` for a non-finite `p`. A finite `p` never
/// scales to NaN: `h` is a finite power of two and the origin is finite, so `u` is finite or
/// infinite, and NaN marks a non-finite node alone.
///
/// The deposit keeps this per slot, so its read and the field read split one stored value
/// rather than each re-reading the node's position at random and dividing again.
pub(super) fn scaled(frame: &Frame, (px, py): (f64, f64)) -> (f64, f64) {
    if !(px.is_finite() && py.is_finite()) {
        return (f64::NAN, f64::NAN);
    }
    (
        (px - frame.origin.0) / frame.h,
        (py - frame.origin.1) / frame.h,
    )
}

/// The lower cell of scaled coordinate `u` on one axis, inside `0..cells - 1`.
// `as usize` truncates toward zero and saturates, which on `u >= 0` is `floor`, and sends a
// negative `u` to 0 as `floor(u).max(0.0)` did, without libm's software floor.
pub(super) fn cell(frame: &Frame, u: f64) -> usize {
    (u as usize).min(frame.cells - 2)
}

/// The CIC stencil of a [`scaled`] position: the lower cell per axis and the weight of the
/// upper. `None` for a non-finite node, which deposits nothing and reads no field.
pub(super) fn split(frame: &Frame, (ux, uy): (f64, f64)) -> Option<((usize, usize), (f64, f64))> {
    if ux.is_nan() {
        return None;
    }
    let axis = |u: f64| {
        let cell = cell(frame, u);
        (cell, (u - cell as f64).clamp(0.0, 1.0))
    };
    let (cx, fx) = axis(ux);
    let (cy, fy) = axis(uy);
    Some(((cx, cy), (fx, fy)))
}

/// The CIC stencil of one position, [`split`] over [`scaled`]: what the tests check the
/// kernels against.
#[cfg(test)]
pub(super) fn stencil(frame: &Frame, p: (f64, f64)) -> Option<((usize, usize), (f64, f64))> {
    split(frame, scaled(frame, p))
}

/// [`place`] over the finite positions of `x`/`y`, one thread: what the tests build a frame
/// with.
#[cfg(test)]
pub(super) fn place_over(xy: (&[f64], &[f64]), side: usize, dmax: f64) -> Option<Frame> {
    let found = bounds(xy, &crate::exec::Serial, 1, &mut Vec::new());
    found.and_then(|b| place(b, side, dmax))
}

#[cfg(test)]
mod tests;
