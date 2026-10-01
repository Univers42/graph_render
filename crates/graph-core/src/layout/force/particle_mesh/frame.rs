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

/// The smallest step tried: `h = 2^-8`. A tighter cluster than that is cut no finer.
const STEP_MIN: i32 = -32;

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

/// The frame over the finite positions in `x`/`y`, for a mesh of `side` cells per axis and
/// a force that vanishes at distance `dmax`. `None` when no position is finite, or when the
/// finite ones span more than an `f64` holds.
pub(super) fn place(x: &[f64], y: &[f64], side: usize, dmax: f64) -> Option<Frame> {
    let (lo, hi) = bounds(x, y)?;
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

/// The finite positions' bounding box, `None` if there is none.
pub(super) fn bounds(x: &[f64], y: &[f64]) -> Option<((f64, f64), (f64, f64))> {
    let mut lo = (f64::INFINITY, f64::INFINITY);
    let mut hi = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for (&px, &py) in x.iter().zip(y) {
        if px.is_finite() && py.is_finite() {
            lo = (lo.0.min(px), lo.1.min(py));
            hi = (hi.0.max(px), hi.1.max(py));
        }
    }
    (lo.0 <= hi.0).then_some((lo, hi))
}

/// The CIC stencil of one position: the lower cell per axis and the weight of the upper.
/// `None` for a non-finite position, which deposits nothing and reads no field.
pub(super) fn stencil(frame: &Frame, (px, py): (f64, f64)) -> Option<((usize, usize), (f64, f64))> {
    if !(px.is_finite() && py.is_finite()) {
        return None;
    }
    let axis = |v: f64, o: f64| {
        let u = (v - o) / frame.h;
        let cell = (libm::floor(u).max(0.0) as usize).min(frame.cells - 2);
        (cell, (u - cell as f64).clamp(0.0, 1.0))
    };
    let (cx, fx) = axis(px, frame.origin.0);
    let (cy, fy) = axis(py, frame.origin.1);
    Some(((cx, cy), (fx, fy)))
}

#[cfg(test)]
mod tests;
