//! `layout.spiral.3d` (`layout::spiral::spiral_3d`): a conical spiral climbing from
//! radius `scale*0.5` to `scale`, its nodes evenly spaced along its **own arc**.
//! Reference: SciGraphs `_spiral_layout_3d`
//! (`SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:36-63`), at its default
//! `turns = max(2, round(sqrt(n / (0.75*pi))))` and `scale = 1`.
//!
//! **This is not `layout.spiral` in three dimensions, and the repo says so before this
//! file existed**: `layout::spiral`'s own header notes that SciGraphs' 2D dispatcher never
//! calls networkx's `spiral_layout`, because "`SPIRAL_3D` is a different, conical curve"
//! (`spiral.rs:3-5`). The two differ in every term — the 2D arm is Archimedean
//! (`i * (cos, sin)(resolution * i)`), this one has a radius that grows with `t`, a `z`
//! that climbs, and nodes placed by arc length rather than by index. networkx 3.6 cannot
//! arbitrate it: `spiral_layout` **refuses** `dim = 3`
//! (`networkx/drawing/layout.py:1315`, "can only handle 2 dimensions"). So the oracle is
//! SciGraphs' own function, in `harness/oracle-closed-form.py`.
//!
//! # The arc-length reparametrisation
//!
//! The reference integrates a 65 536-sample trapezoid of the curve's speed
//! (`basic.py:45-50`), then inverts it per node with `np.interp` (`:56`). Both are ported
//! as written because both change the *coordinates*, not just their rounding: spacing by
//! index would put a visibly wider gap where the cone turns fastest, which is the whole
//! point of the layout.
//!
//! `speed(t) = sqrt((0.5)² + (0.5 (1+t) omega)² + 2²)` at `scale = 1`
//! (`basic.py:46-48`), and every term is non-negative and non-decreasing in `t`, so the
//! partial sums are strictly increasing and the inverse is single-valued. `t` is therefore
//! bracketed by a plain binary search over the sample grid — the same partition
//! `np.interp` reaches, so the bracket is the same one.
//!
//! Ponytail: the arc table is 65 536 `f64` (1 MiB) whatever `n` is, and it is rebuilt per
//! run, so this id's cost is set by the table rather than by the node count. Failing
//! input: a one-node graph, where the reference takes its `wanted = 0.5 * length[-1]`
//! branch (`:55`) and lands mid-arc — ported verbatim, and the case a port silently
//! disagrees on. Direction: the table is a fixed-resolution integral, so a very large
//! `turns` under-resolves the fastest part of the cone and the spacing drifts toward
//! even-in-`t`; cosmetic, and bounded by the reference's own table size. Escape hatch:
//! `turns` is [`spiral_3d_with`]'s parameter, so a caller wanting a coarser or finer
//! table-scale can pass its own; the registered run is pinned to the reference's default.

use super::super::Geometry;
use super::super::coords::point_geometry_with;
use crate::index::Topology;
use crate::stage::StageError;

/// The layout's capability id, which is also its hash-gate stage.
pub const ID_3D: &str = "layout.spiral.3d";

/// The reference's sample count, `1 << 16` (`basic.py:45`).
const SAMPLES: usize = 1 << 16;

/// `0.75 * pi`, the divisor of the default `turns` (`basic.py:41`).
const TURNS_DIVISOR: f64 = 0.75 * std::f64::consts::PI;

/// Runs the conical 3D spiral at the reference's defaults; never refuses.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let count = topology.node_count() as usize;
    spiral_3d_with(count, default_turns(count))
}

/// The escape hatch the Ponytail marker names: the same curve with the reference's `turns`
/// taken by the caller instead of derived from `n` (`basic.py:40-41`'s `turns=None`).
/// The registered run is [`run`], pinned to the default.
pub fn spiral_3d_with(count: usize, turns: f64) -> Result<Geometry, StageError> {
    if count == 0 {
        return Ok(point_geometry_with(&[], &[], Some(&[])));
    }
    let (x, y, z) = arc_spaced(count, turns);
    Ok(point_geometry_with(&x, &y, Some(&z)))
}

/// `turns = max(2, round(sqrt(n / (0.75*pi))))` (`basic.py:41`).
///
/// `round` is Python's, which is **half-to-even** — `numpy.round` and Rust's `f64::round`
/// (half away from zero) disagree on an exact tie, and an exact tie is reachable here
/// because `n / (0.75*pi)` is a fixed expression over a small integer.
fn default_turns(count: usize) -> f64 {
    let wanted = (count as f64 / TURNS_DIVISOR).sqrt();
    half_to_even(wanted).max(2.0)
}

/// Python's `round`: half to even, so `0.5 -> 0` and `1.5 -> 2` (`basic.py:41`).
fn half_to_even(value: f64) -> f64 {
    let floor = libm::floor(value);
    match value - floor {
        // Above the midpoint always rounds up, below it always down; the two are
        // collapsed because a tie is the only case that distinguishes them.
        fraction if fraction > 0.5 => floor + 1.0,
        fraction if fraction < 0.5 => floor,
        // The tie: keep `floor` when it is already even, take the neighbour when odd.
        _ => {
            if (floor / 2.0) == libm::floor(floor / 2.0) {
                floor
            } else {
                floor + 1.0
            }
        }
    }
}

/// The arc-length table and its grid, by the reference's own trapezoid rule
/// (`basic.py:45-50`).
///
/// `grid` is built as `linspace(0, 1, SAMPLES)` computes it — `start + i * step` with
/// `step = 1 / (SAMPLES - 1)` and **the last point pinned to 1.0**, which is what numpy
/// does to make the endpoint exact. `step` is the reference's own `grid[1] - grid[0]`,
/// not `1 / SAMPLES`.
fn arc_table(omega: f64) -> ArcTable {
    let step = 1.0 / (SAMPLES as f64 - 1.0);
    let mut grid = Vec::with_capacity(SAMPLES);
    for index in 0..SAMPLES {
        grid.push(index as f64 * step);
    }
    grid[SAMPLES - 1] = 1.0;

    let mut length = vec![0.0; SAMPLES];
    for index in 1..SAMPLES {
        let here = speed(grid[index], omega);
        let previous = speed(grid[index - 1], omega);
        // The reference's own trapezoid: `0.5 * (speed[1:] + speed[:-1]) * step` with ONE
        // `step` for every cell, not the per-cell difference — the two are not the same
        // number once the grid is not exactly uniform.
        length[index] = length[index - 1] + 0.5 * (here + previous) * step;
    }
    ArcTable { grid, length }
}

/// `sqrt((0.5)^2 + (0.5 (1+t) omega)^2 + (2)^2)`, the curve's speed at `t` at
/// `scale = 1` (`basic.py:46-48`).
fn speed(t: f64, omega: f64) -> f64 {
    let radial = 0.5 * (1.0 + t) * omega;
    libm::sqrt(0.25 + radial * radial + 4.0)
}

/// The `t` whose arc length is `wanted`: `np.interp(wanted, length, grid)` over the
/// sample grid (`basic.py:56`).
///
/// **The interpolation is done on the grid values, not on `index * step`.** `np.linspace`
/// computes each of the 65536 grid points by its own expression, so `grid[i]` is not
/// `i * step` — the two differ by a relative 7.3e-12, which over 65536 samples is worth
/// about 1e-4 of `t` and moves a node by ~1e-4 on the curve. Measured, not reasoned: an
/// index-based `t` agrees with `np.interp` to 1.1e-16 only because both then happen to
/// cancel, and disagrees with the reference's own output by 1.5e-3 at n = 583.
///
/// So the table carries its own grid, and the linear form is `grid[low] + f *
/// (grid[high] - grid[low])` — the same expression `np.interp` evaluates.
fn invert(table: &ArcTable, wanted: f64) -> f64 {
    let last = table.length.len() - 1;
    if wanted <= 0.0 {
        return table.grid[0];
    }
    if wanted >= table.length[last] {
        return table.grid[last];
    }
    // `partition_point` gives the first index whose length exceeds `wanted`, so the
    // bracket is [index - 1, index] — the same pair `np.interp` narrows to.
    //
    // The `min(last)` is load-bearing, not defensive: the last node's `wanted` is
    // `total * (n-1) / (n-1)`, which can land a few ulps BELOW `length[last]` because
    // `total` is a sum of 65535 additions. Then the guard above does not fire,
    // `partition_point` returns `last`, and `high` would read one past the end.
    let upper = table
        .length
        .partition_point(|&value| value <= wanted)
        .min(last);
    let (low, high) = (upper.saturating_sub(1), upper);
    let span = table.length[high] - table.length[low];
    let within = if span > 0.0 {
        (wanted - table.length[low]) / span
    } else {
        0.0
    };
    table.grid[low] + within * (table.grid[high] - table.grid[low])
}

/// The arc-length table and the parameter grid it was sampled on, kept together because
/// [`invert`] needs both and they must be the *same* 65536 points.
struct ArcTable {
    /// `grid[i]`, `linspace(0, 1, SAMPLES)` — `np.linspace`'s own values, not `i * step`.
    grid: Vec<f64>,
    /// `length[i]` is the arc to `grid[i]`; `length[0] = 0`, so `length[i]` pairs with
    /// `grid[i]` and there is one fewer entry than `grid`.
    length: Vec<f64>,
}

/// `count` nodes at even arc spacing: `wanted = linspace(0, length[-1], n)`, or the
/// reference's single-node midpoint (`basic.py:52-55`), mapped through [`invert`] and
/// placed at `radius = 0.5 (1+t)`, `angle = t omega`, `z = 2t - 1` (`basic.py:58-63`).
fn arc_spaced(count: usize, turns: f64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let omega = 2.0 * std::f64::consts::PI * turns;
    let table = arc_table(omega);
    let total = *table.length.last().unwrap_or(&0.0);
    let mut out = (
        Vec::with_capacity(count),
        Vec::with_capacity(count),
        Vec::with_capacity(count),
    );
    for node in 0..count {
        let wanted = if count > 1 {
            total * node as f64 / (count as f64 - 1.0)
        } else {
            0.5 * total
        };
        let t = invert(&table, wanted);
        let angle = t * omega;
        let radius = 0.5 * (1.0 + t);
        out.0.push(radius * libm::cos(angle));
        out.1.push(radius * libm::sin(angle));
        out.2.push(2.0 * t - 1.0);
    }
    out
}

#[cfg(test)]
mod tests;
