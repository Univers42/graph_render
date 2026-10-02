//! The ink measurement: how much of a drawing a bundler saved, and what one pass costs to
//! find out. The quality claim a bundler makes is a number, and this is where the number
//! comes from.
//!
//! **Ink is a cell count, not a length.** Bundling cannot shorten the drawing: every path
//! still runs from its source to its target, and a curve is never shorter than the
//! segment, so the total drawn length *rises* under bundling by construction. What falls
//! is how much of the page the strokes cover — a bundle is one line drawn many times over,
//! so the same cells are marked once. [`Ink::cells`] is that count and [`Ink::length`] is
//! reported beside it, never instead of it.
//!
//! **The walk is clipped to the box, and used to be marked outside it.** A segment was
//! walked in half-cell steps along its whole length, and every step outside the box landed
//! in a border cell through the clamp in [`cell`], so a stroke leaving the drawing marked
//! cells it never crossed and cost half a cell of walk per unit it travelled — an edge point
//! at `1e7` on a box of 1 is 2.56e9 steps. [`Raster::mark`] now walks the part inside the
//! box ([`clip`]) and marks nothing outside it, which is the honest count: an ink saving is
//! a claim about the drawing, and a point at 1e7 is not in it. A cell *inside* the box is
//! marked exactly as before, so no figure in `docs/measurements/phase08-ink.md` moves, and
//! `Ink::length` is unaffected — it is still every segment's whole length, so the two
//! numbers keep describing the same stroke.
//!
//! **Ponytail (raster).** [`INK_RESOLUTION`] is a stated resolution, not a truth. A coarse
//! raster calls two strokes a hair apart one cell and reports no saving where a fine one
//! sees a real one; a fine one splits one stroke across cells and reports a saving on a
//! drawing that did not get any tighter. Failing input: a bundle whose members sit less
//! than one cell apart. Direction: cosmetic either way — the geometry is unchanged, only
//! the number moves. Escape hatch: [`INK_RESOLUTION`], and re-running the measurement at
//! another resolution is the check a host that cares should make.

use crate::index::Topology;
use crate::layout::Geometry;
use graph_contract::geometry::NodeGeometry;

mod clip;

#[cfg(test)]
mod tests;

/// Cells per axis of the ink raster: the nodes' bounding frame, square-divided this many
/// times. The number behind every ink figure in `docs/measurements/phase08-ink.md`.
pub const INK_RESOLUTION: u32 = 128;

/// The two ink numbers for one geometry, from one walk of its edge paths.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ink {
    /// Cells of the [`INK_RESOLUTION`] raster that at least one drawn segment crosses,
    /// each counted once however many edges cross it.
    pub cells: u32,
    /// The total drawn length, every edge counted in full. Bundling raises it; it is
    /// reported so the cell count cannot be mistaken for a length saving.
    pub length: f64,
}

/// The ink `geometry` draws over `topology`'s edges. A `Line` edge is the segment between
/// its two nodes, a `Polyline` or a `Curve` the segments through its interior points, so
/// straight and bundled geometry are measured the same way — the measurement does not
/// assume a layout, which is the same claim composability makes.
pub fn ink(topology: &Topology, geometry: &Geometry) -> Ink {
    let (x, y) = centres(&geometry.nodes);
    let mut raster = Raster::over(x, y);
    let mut length = 0.0;
    for e in 0..topology.edge_count() {
        let endpoints = &topology.edges();
        let (from, to) = (
            endpoints.source[e as usize] as usize,
            endpoints.target[e as usize] as usize,
        );
        let (mut px, mut py) = (x[from], y[from]);
        for point in crate::post::row(&geometry.edges, e).as_chunks::<2>().0 {
            length += raster.mark(px, py, point[0], point[1]);
            px = point[0];
            py = point[1];
        }
        length += raster.mark(px, py, x[to], y[to]);
    }
    Ink {
        cells: raster.occupied(),
        length,
    }
}

/// The uniform grid [`ink`] counts on: the nodes' bounding box divided [`INK_RESOLUTION`]
/// times per axis. A point exactly on a cell boundary falls in the higher cell, which the
/// truncation of a non-negative value gives for free, and the last row and column stay
/// reachable because every cell index is clamped into the grid.
struct Raster {
    lo: (f32, f32),
    scale: (f32, f32),
    cells: Vec<bool>,
}

impl Raster {
    /// The grid over the nodes' bounding box. A degenerate axis (every node on one
    /// coordinate) is scaled by `1`, so the division stays finite and the axis is one cell.
    ///
    /// Ponytail (tiny box): an extent under `INK_RESOLUTION / f32::MAX` (~3.8e-37) overflows
    /// the scale to `+inf`, which made the half-cell step 0 and the walk `u32::MAX` steps
    /// long. Such an axis is degenerate too. Failing input: a drawing under ~3.8e-37 across.
    /// Direction: under-reports, one cell where the drawing may cross up to 128. Escape
    /// hatch: measure the drawing at a usable scale.
    fn over(x: &[f32], y: &[f32]) -> Raster {
        let axis = |a: &[f32]| {
            let (low, high) = span(a);
            let scale = INK_RESOLUTION as f32 / (high - low);
            if high > low && scale.is_finite() {
                (low, scale)
            } else {
                (low, 1.0)
            }
        };
        let (lo_x, scale_x) = axis(x);
        let (lo_y, scale_y) = axis(y);
        Raster {
            lo: (lo_x, lo_y),
            scale: (scale_x, scale_y),
            cells: vec![false; (INK_RESOLUTION * INK_RESOLUTION) as usize],
        }
    }

    /// Marks every cell the segment `a`–`b` crosses, walked in half-cell steps so none is
    /// stepped over, and returns the segment's length.
    ///
    /// The walk covers the part of the segment inside the box only. `steps` is
    /// `max(|dx|, |dy|)` at half a cell, so an endpoint at 1e7 against a box of 1 was
    /// 2.56e9 iterations of a loop whose every mark fell in the same clamped border cell.
    /// The length is the whole segment's, clipped or not: the cell count is the number that
    /// falls inside the box, the length is the stroke.
    fn mark(&mut self, ax: f32, ay: f32, bx: f32, by: f32) -> f64 {
        let length = f64::from(libm::hypotf(bx - ax, by - ay));
        let Some(ts) = clip::clip((ax, ay), (bx, by), self.lo, self.hi()) else {
            return length;
        };
        let ((x0, y0), (x1, y1)) = clip::ends((ax, ay), (bx, by), ts);
        let steps = self.steps(x1 - x0, y1 - y0);
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            self.mark_point(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
        }
        length
    }

    /// The box's far corner: `lo` plus the whole drawing on each axis. A degenerate axis
    /// (scale `1`) reaches `INK_RESOLUTION` past its `lo`, which is as far as its cells
    /// index, so no walk spends itself past the last one.
    ///
    /// Ponytail (far corner): `INK_RESOLUTION / scale` is a division of the scale, not the
    /// subtraction `high - low` it stands for, so on an axis whose scale was rounded the
    /// corner sits a few ulps off the drawing's edge. Failing input: an axis where the node
    /// spread is not exactly representable. Direction: one segment-endpoint's cell at most,
    /// and the endpoint is the drawing's own extreme, so it lands in the last cell either
    /// way. Escape hatch: build the corner from the span.
    fn hi(&self) -> (f32, f32) {
        (
            self.lo.0 + INK_RESOLUTION as f32 / self.scale.0,
            self.lo.1 + INK_RESOLUTION as f32 / self.scale.1,
        )
    }

    /// Half-cell steps a segment of extent `(dx, dy)` is walked in, at least one.
    fn steps(&self, dx: f32, dy: f32) -> u32 {
        let step = 0.5 / self.scale.0.max(self.scale.1);
        (dx.abs().max(dy.abs()) / step).ceil().max(1.0) as u32
    }

    /// Marks the one cell a point falls in.
    fn mark_point(&mut self, x: f32, y: f32) {
        let cx = cell((x - self.lo.0) * self.scale.0);
        let cy = cell((y - self.lo.1) * self.scale.1);
        self.cells[(cy * INK_RESOLUTION + cx) as usize] = true;
    }

    /// How many cells are marked.
    fn occupied(&self) -> u32 {
        self.cells.iter().filter(|marked| **marked).count() as u32
    }
}

/// `(low, high)` of a column, or `(0, 1)` when it is empty: an empty drawing has no box,
/// and a zero span would make every cell index NaN.
fn span(a: &[f32]) -> (f32, f32) {
    if a.is_empty() {
        return (0.0, 1.0);
    }
    let mut low = f32::INFINITY;
    let mut high = f32::NEG_INFINITY;
    for value in a {
        low = low.min(*value);
        high = high.max(*value);
    }
    (low, high)
}

/// One coordinate's cell index, clamped into `0..INK_RESOLUTION`. A negative coordinate (a
/// point left of the box, which a curved path may reach) clamps to the first cell rather
/// than wrapping into the last.
fn cell(value: f32) -> u32 {
    if value > 0.0 { value as u32 } else { 0 }.min(INK_RESOLUTION - 1)
}

/// [`crate::post::centres`], under this module's name: [`ink`] reads its node centres
/// through it. Kept public because `post::ink::centres` is published surface.
pub fn centres(nodes: &NodeGeometry) -> (&[f32], &[f32]) {
    crate::post::centres(nodes)
}
