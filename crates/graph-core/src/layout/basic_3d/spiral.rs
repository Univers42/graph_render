//! `_spiral_layout_3d` (`SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:36-63`): a
//! conical spiral climbing from radius `scale*0.5` to `scale`, spaced evenly along its own
//! arc length. Closed form, no stream, no graph.
//!
//! **This is not `layout.spiral`.** That id is graph-core's planar Archimedean spiral, a
//! port of networkx's `spiral_layout` at its own `resolution = 0.35`, and SciGraphs has no
//! 2D spiral at all: `SPIRAL_3D` dispatches to this function (`dispatcher.py:105-106`) and
//! nothing else. Two different curves under two different names — do not "fix" this row by
//! editing `layout/spiral.rs`.
//!
//! The construction, as the reference writes it:
//!
//! ```text
//! turns = max(2, round(sqrt(n / (0.75*pi))))   basic.py:41
//! omega = 2*pi*turns                           basic.py:43
//! grid   = linspace(0, 1, 65536)                basic.py:45
//! speed  = sqrt((0.5s)^2 + (0.5s*(1+grid)*omega)^2 + (2s)^2)   basic.py:46-48
//! length = [0, cumsum(0.5*(speed[1:] + speed[:-1])*step)]      basic.py:50
//! wanted = linspace(0, length[-1], n)           basic.py:52-55
//! t      = interp(wanted, length, grid)         basic.py:56
//! x = r*cos(t*omega),  y = r*sin(t*omega),  z = s*(2t - 1),  r = s*0.5*(1 + t)
//! ```
//!
//! **Everything here is a port of a numpy primitive, and each has an arithmetic a naive
//! translation gets wrong.** `t` is the only value in this layout that is not closed form,
//! and reproducing it means reproducing four of them:
//!
//! - **`linspace` is `start + i*step`, and the reference additionally overwrites the last
//!   element with `stop`.** Not `stop*i/(num-1)`, and not `stop*(i/(num-1))`. Reproduced bit
//!   for bit over all 65 536 grid points. **The overwrite and the plain product agree here**:
//!   `65535 * step` is exactly `1.0`, measured and pinned, so there is no last-element
//!   special case to port — see `grid_at`.
//! - **`cumsum` is sequential.** `length[i] = length[i-1] + step`, 65 535 additions in a
//!   fixed order, never a pairwise or blocked reduction (which is what `np.sum` does). The
//!   order *is* the value here: a blocked sum of 65 535 terms lands a few ulp away and the
//!   interpolated `t` inherits it.
//! - **`interp` has its own slope formula**: with `j` the last index whose `xp[j] <= x`,
//!   `slope = (fp[j+1] - fp[j]) / (xp[j+1] - xp[j])` and the result is
//!   `slope*(x - xp[j]) + fp[j]`, both terms in that order. Reproduced bit for bit against
//!   `np.interp` on 200 000 unrelated monotone points.
//! - **`round` is half-to-even** (Python's `float.__round__`, and `np.float64.__round__`
//!   behind it), so `turns` at an exact `x.5` goes to the even neighbour, not away from zero.
//!
//! **Memory is the price of the arc.** The reference materialises two 65 536-element arrays
//! per call; this port materialises one (`length`, 512 KiB) and evaluates `grid[j]` in
//! closed form as `j*step`, which is the same number of bits for the reason above. Nothing
//! here is asymptotic in `n`: the table is a constant, and the `n` work is one binary search
//! per node plus three columns.

use super::{SCALE, in_space};
use crate::layout::Geometry;
use crate::stage::StageError;

/// The reference's parameter grid, `np.linspace(0.0, 1.0, 1 << 16)` (`basic.py:45`).
const GRID: usize = 1 << 16;

/// `SPIRAL_3D`'s capability id, which is also its hash-gate stage.
pub const ID: &str = "layout.basic3d.spiral";

/// `_spiral_layout_3d(n, scale)` (`basic.py:36-63`) over the node count.
///
/// **DIVERGENCE AT `n = 0`, and it is the reference that refuses.** The reference's guard is
/// `if num_nodes > 1` (`basic.py:52`), so `num_nodes = 0` falls into the *same* branch as
/// `num_nodes = 1`: `wanted = [0.5 * length[-1]]`, and `np.column_stack` of three scalars
/// returns shape `(1, 3)` — the single point, for a graph with no nodes. `apply_graph_layout`
/// then rejects it: `_check_positions` (`layouts/common.py:175-185`) raises `ValueError`
/// because the shape is not `(0, 3)`, and the dispatcher returns `False`. Measured in
/// `ge-python-oracle` against the submodule.
///
/// This port returns an **empty 3D geometry** — three empty columns — and never fails. That is
/// a real disagreement with the reference, taken deliberately: `run`'s whole contract is
/// `Result<Geometry, StageError>` and every function in `basic_3d` is total, so a caller
/// asking for an empty graph gets an empty drawing on all four ids rather than an error on
/// one of them. Unreachable from the conformance matrix, whose smallest fixture has 2 nodes.
/// The alternative — returning `Err` at `n = 0` — would be the only non-total layout in the
/// module, for an input no caller has.
pub(super) fn run(n: u32) -> Result<Geometry, StageError> {
    let (x, y, z) = columns(n);
    Ok(in_space(&x, &y, &z))
}

/// [`run`] at the `scale` the caller asks for. Every use of it is the reference's own
/// (`radial = 0.5*scale`, `axial = 2.0*scale`, `radius = scale*0.5*(1+t)`, `z =
/// scale*(2t-1)`, `basic.py:41-63`), so `run_scaled(n, SCALE) == run(n)` bit for bit.
///
/// Note what the scale does and does not reach: it scales the *speed*, and therefore the
/// arc length and the `t` column, but `t` itself is a `linspace` over that arc, so it is
/// unchanged by a common factor. That is the reference's own behaviour, not an accident of
/// this port, and the differential holds `t` at one scale only.
pub(super) fn run_scaled(n: u32, scale: f64) -> Result<Geometry, StageError> {
    let (x, y, z) = columns_scaled(n, scale);
    Ok(in_space(&x, &y, &z))
}

/// `turns = max(2, int(round(np.sqrt(num_nodes / (0.75 * np.pi)))))` (`basic.py:41`).
///
/// **The floor lifts the value at exactly five node counts, `n = 1..5`.** Measured with
/// numpy 2.3.3: the raw rounded value is 1 for `n = 1, 2, 3, 4, 5` and already 2 for
/// `n = 6..14`, so from 6 to 14 `max(2, ...)` is a no-op and the count is 2 because the
/// rounding already said so. The first count that rounds to 3 is `n = 15`. So the failing
/// input is small graphs, and only just — at `n = 7` (a size a reader reaches for) the floor
/// changes nothing and `turns` would be 2 without it. `round` is half-to-even, which
/// `floor(x + 0.5)` would not be at an exact `x.5`.
fn turns(n: u32) -> u32 {
    let estimate = f64::sqrt(f64::from(n) / (0.75 * core::f64::consts::PI));
    estimate.round_ties_even().max(2.0) as u32
}

/// `omega = 2*pi*turns` (`basic.py:43`), computed once per run and never accumulated per node.
fn omega(n: u32) -> f64 {
    2.0 * core::f64::consts::PI * f64::from(turns(n))
}

/// `linspace`'s own step, `delta/div` — one `f64` division (`basic.py:45`, read back at
/// `basic.py:49` as `grid[1] - grid[0]`, which is the same number of bits).
fn grid_step() -> f64 {
    1.0 / (GRID - 1) as f64
}

/// `grid[j]`: `linspace` computes `j*step + 0.0` (`basic.py:45`).
///
/// **There is no endpoint fix-up here, and there does not need to be one.** An earlier
/// version of this function carried an `if j == GRID - 1 { 1.0 }` branch on the belief that
/// `65535 * step` was short of `1.0`. It is not: `step` is `1.0/65535` rounded once, and
/// `65535.0 * step` is exactly `1.0` — `0x3ff0000000000000`, measured in `ge-python-oracle`,
/// and pinned by `the_grid_last_point_is_the_product_not_a_fix_up`. So the branch was dead
/// code defending against a number that was never wrong. The reference's overwrite of the
/// last element with `stop` and this function's plain product agree bit for bit, and the
/// product is kept because it is the shorter of the two.
fn grid_at(j: usize, step: f64) -> f64 {
    j as f64 * step
}

/// `speed[i]` (`basic.py:46-48`), the curve's speed along `t`. The reference's own operand
/// order, and the three summands added left to right.
fn speed_at(grid: f64, omega: f64, scale: f64) -> f64 {
    let radial = 0.5 * scale;
    let climb = radial * (1.0 + grid) * omega;
    let axial = 2.0 * scale;
    f64::sqrt(radial * radial + climb * climb + axial * axial)
}

/// `basic.py:50`: a leading `0.0` followed by `cumsum(0.5*(speed[1:] + speed[:-1])*step)`.
///
/// **Sequential, not blocked** — see the module header. 65 535 additions in the order the
/// index runs, so `running` is the only cross-iteration state and each term cannot inherit
/// a different rounding than the one before it (D10).
fn arc_length(step: f64, omega: f64, scale: f64) -> Vec<f64> {
    let mut length = Vec::with_capacity(GRID);
    let mut previous = speed_at(grid_at(0, step), omega, scale);
    let mut running = 0.0;
    length.push(running);
    for j in 1..GRID {
        let current = speed_at(grid_at(j, step), omega, scale);
        running += 0.5 * (current + previous) * step;
        length.push(running);
        previous = current;
    }
    length
}

/// `wanted[i]` (`basic.py:52-55`): a `linspace` over the arc length, or — at `n <= 1`, which
/// is one node and not two — **half** the arc, not the whole of it.
///
/// The two special cases are separate for a reason: `linspace(0, total, 1)` divides by
/// `n - 1 == 0`, and the reference guards it before that happens (`basic.py:52`). Reading
/// the guard as `n == 1` and the fix-up as arithmetic gives the same numbers; reading
/// either as "the ends are the ends" does not.
fn wanted_at(i: u32, n: u32, total: f64) -> f64 {
    if n <= 1 {
        return 0.5 * total;
    }
    if i == n - 1 {
        return total;
    }
    f64::from(i) * (total / f64::from(n - 1))
}

/// `np.interp(x, length, grid)` (`basic.py:56`), formula for formula.
///
/// `j` is the last index whose `length[j] <= x`; numpy's search returns that index and only
/// that index, so any search which finds it computes the same number. The arithmetic is the
/// part that has to be right — see the module header. Outside the arc the result is
/// numpy's `left`/`right` defaults, `fp[0]` and `fp[-1]`; at the top end that is the same
/// value numpy's own `j == lenxp - 1` branch returns without a slope, so `>=` here is
/// numpy's behaviour rather than a shortcut around it.
fn interp(x: f64, length: &[f64], step: f64) -> f64 {
    let last = length.len() - 1;
    if x < length[0] {
        return grid_at(0, step);
    }
    if x >= length[last] {
        return grid_at(last, step);
    }
    let j = length.partition_point(|v| *v <= x) - 1;
    let slope = (grid_at(j + 1, step) - grid_at(j, step)) / (length[j + 1] - length[j]);
    slope * (x - length[j]) + grid_at(j, step)
}

/// The `t` column (`basic.py:52-56`): where each node sits along the spiral, in `[0, 1]`.
///
/// Split out of [`columns`] because `t` is the whole of this layout's arc-length inversion
/// and every other value here is closed form applied to it. It is a legal gather (D10):
/// each entry depends on the shared table and on its own index, and on nothing else.
fn parameters(n: u32, scale: f64) -> Vec<f64> {
    let step = grid_step();
    let length = arc_length(step, omega(n), scale);
    let total = length[GRID - 1];
    (0..n)
        .map(|i| interp(wanted_at(i, n, total), &length, step))
        .collect()
}

/// The three `f64` columns at `n`, before any narrowing, in the reference's own order
/// (`np.column_stack`, `basic.py:61-63`).
fn columns(n: u32) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    columns_scaled(n, SCALE)
}

/// [`columns`] at an explicit `scale` (`basic.py:41-63`).
fn columns_scaled(n: u32, scale: f64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let omega = omega(n);
    let mut columns = (
        Vec::with_capacity(n as usize),
        Vec::with_capacity(n as usize),
        Vec::with_capacity(n as usize),
    );
    for t in parameters(n, scale) {
        let angle = t * omega;
        let radius = scale * 0.5 * (1.0 + t);
        columns.0.push(radius * libm::cos(angle));
        columns.1.push(radius * libm::sin(angle));
        columns.2.push(scale * (2.0 * t - 1.0));
    }
    columns
}

#[cfg(test)]
mod tests;
