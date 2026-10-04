//! The SciGraphs placement of the same lattice: the first cell at the origin, and a pitch
//! of `scale / cols` instead of a fixed `spacing`.
//!
//! **A second kernel, not a second parameter.** `_grid_layout(num_nodes, scale)`
//! (`SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:11-20`) is
//! `x = (i % cols) * scale / cols`, `y = (i // cols) * scale / cols` — a Python
//! `int * float` then a `/ int`, both in `f64`, and the snapshot narrows once. The
//! registered stage in the parent cannot reach those bits through `GridParams::spacing`,
//! a `f32`: a `f32` pitch is one rounding and a second one on the way out, and
//! `fl32(k * fl32(scale / cols))` and `fl32(fl64(k * scale) / cols)` are a whole ULP apart
//! for `scale = 5.0, cols = 9, k = 3` — see
//! `a_f32_pitch_would_not_have_reached_these_bits`. So the multiply-then-divide happens
//! here in `f64`, and the registered `f32` kernel above is untouched.
//!
//! Exact, not a heuristic: IEEE-754 binary64 multiply and divide, then the single
//! narrowing into the `f32` `Geometry` the snapshot holds. **The bound that actually holds
//! is the guard's, not a size limit**: `cell * scale` is an unbounded `f64` product in
//! general, and what keeps it in range here is `scaled_cells`' second rule — a scale
//! whose widest coordinate is already inside `f32::MAX`, with `cell <= cols - 1` — so
//! neither the product nor the quotient is ever formed as `inf`. Same bytes on every
//! target; no transcendental, no reduction, nothing to order.

use super::{Grid, dimensions};
use crate::exec::{Runner, StepRange};
use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::StageError;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use std::ops::Range;

impl Grid {
    /// The SciGraphs lattice over `topology` at `scale`, its per-node gather handed to
    /// `runner` over `workers` workers — the same shape as [`Grid::run_with`], so
    /// `run_scaled(..., &Serial, 1)` is a serial stage and any other runner is a
    /// *schedule* of the same computation.
    ///
    /// The registered [`Stage::run`](crate::stage::Stage::run) is not this and does not move:
    /// it keeps the centred lattice at `GridParams::spacing`, which is what the pipeline's
    /// registered `layout.grid` and its snapshot hash are.
    pub fn run_scaled(
        topology: &Topology,
        scale: f64,
        runner: &impl Runner,
        workers: u32,
    ) -> Result<Geometry, StageError> {
        let mut pairs = Vec::new();
        runner.run(
            &scaled_cells(topology.node_count(), scale)?,
            workers,
            &mut pairs,
        );
        let (x, y): (Vec<f32>, Vec<f32>) = pairs.into_iter().unzip();
        Ok(Geometry::planar(
            NodeGeometry::Point { x, y },
            EdgeGeometry::Line,
            Vec::new(),
        ))
    }
}

/// The SciGraphs kernel over `n` nodes at `scale`, or the parameter error a bad scale is.
///
/// **Two rules, both about the coordinate and not about the drawing.** The scale is finite and
/// above 0 — the parent's `spacing` rule, so `run_scaled` refuses what `run` refuses — and the
/// widest coordinate the lattice can write, `(cols - 1) * scale / cols`, is inside the `f32`
/// range. The second rule is not decoration: the `as f32` in [`ScaledLattice::at`] overflows
/// to `inf` rather than saturating (measured, not assumed), so on this toolchain a finite scale
/// of `1e39` put `inf` in a `Geometry` and this comment used to promise it never would.
/// `cols - 1` bounds every cell index because `i / cols <= cols - 1` for `cols = ceil(sqrt(n))`,
/// and `cols <= 1` writes only the origin, so neither divides by zero. See
/// `a_scale_that_does_not_fit_the_f32_range_is_refused`.
fn scaled_cells(n: u32, scale: f64) -> Result<ScaledLattice, StageError> {
    if !(scale.is_finite() && scale > 0.0) {
        return Err(StageError::Param {
            name: "scale",
            rule: "finite and above 0",
        });
    }
    let (cols, _) = dimensions(n);
    let widest = f64::from(cols.saturating_sub(1)) * scale / f64::from(cols.max(1));
    if widest > f64::from(f32::MAX) {
        return Err(StageError::Param {
            name: "scale",
            rule: "widest coordinate inside the f32 range",
        });
    }
    Ok(ScaledLattice {
        count: n,
        cols,
        scale,
    })
}

/// Every node's cell, in index order: node `i` in column `i % cols`, row `i / cols`, the
/// first cell at the origin.
///
/// A [`StepRange`] like the parent's `Lattice`, and a separate one on purpose: one kernel
/// with both placements would put a branch in the registered path's hot loop to serve a
/// convention the registered path does not use.
#[derive(Debug)]
struct ScaledLattice {
    count: u32,
    cols: u32,
    scale: f64,
}

impl StepRange for ScaledLattice {
    type Out = (f32, f32);

    fn len(&self) -> u32 {
        self.count
    }

    fn step_range(&self, range: Range<u32>, out: &mut [(f32, f32)]) {
        for (i, slot) in range.zip(out) {
            *slot = (self.at(i % self.cols), self.at(i / self.cols));
        }
    }
}

impl ScaledLattice {
    /// `cell * scale / cols` in numpy's order: `f64` multiply, `f64` divide, then the one
    /// narrowing to the `f32` the snapshot holds.
    fn at(&self, cell: u32) -> f32 {
        (cell as f64 * self.scale / self.cols as f64) as f32
    }
}

#[cfg(test)]
mod tests;
