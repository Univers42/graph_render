//! DrL's repulsive density field: a coarse 1000 x 1000 grid of separable tent kernels,
//! replaced in the last stage by per-cell node lists and a `1/d^2` sum.
//!
//! **The coarse grid has no third axis, so the 3D arm uses the per-cell bins from the
//! start.** That is the honest shape rather than a 2D field extruded along `z`: the bins
//! hold node positions and sum `1/d^2` over the real 3D distance, so the 3D field is a 3D
//! field; only the cheap separable approximation is 2D. Ponytail: igraph's own 3D DrL
//! (which SciGraphs calls at `igraph_layouts.py:342`) uses its internal density rather than
//! either of these, so the 3D pictures differ. Failing input: a graph whose final spread
//! is far wider than `HALF_VIEW`, where every candidate sits outside the view and every
//! energy reads `WALL`, so no move ever wins on energy. Direction: cosmetic — the layout
//! still settles, on the analytic pull alone. Escape hatch: `HALF_VIEW` and `VIEW_TO_GRID`
//! here, and `dim` on `DrlParams`.

use std::collections::BTreeMap;

const SIDE: i64 = 1000;
const RADIUS: i64 = 10;
const HALF_VIEW: f64 = 2000.0;
const VIEW_TO_GRID: f64 = 0.25;
/// The view's own middle, the `dim` 2 stand-in for a third coordinate.
const MID_VIEW: f64 = 2000.0;
/// Energy of a position on or beyond the border margin: a wall, not a measurement.
const WALL: f64 = 10_000.0;

/// The widest point this port keeps; see
/// [`super::fruchterman_reingold::MAX_DIM`].
pub(super) const MAX_DIM: usize = 3;

/// Grid cell of `at`, `None` inside the border margin or outside the view.
///
/// The third cell is `0` at `dim` 2 and real at `dim` 3, so the key is one number in both
/// cases and the 2D binning is the 3D binning with `cz` held at zero — which is why the
/// 2D arm's bytes do not move.
fn cell(at: &[f64; MAX_DIM], dim: usize) -> Option<(i64, i64, i64)> {
    let fx = (at[0] + HALF_VIEW) * VIEW_TO_GRID;
    let fy = (at[1] + HALF_VIEW) * VIEW_TO_GRID;
    // At `dim` 2 the third coordinate is the view's own middle, so it is always inside the
    // border margin. Reading `at[2]` would be zero on a 2D run and sit in the margin's
    // dead band, where every candidate reads as outside the view and every energy is the
    // wall — which is exactly the bug this arm would have shipped.
    let fz = if dim == MAX_DIM {
        (at[2] + HALF_VIEW) * VIEW_TO_GRID
    } else {
        MID_VIEW * VIEW_TO_GRID
    };
    if !(fx >= 0.0
        && fy >= 0.0
        && fz >= 0.0
        && fx < SIDE as f64
        && fy < SIDE as f64
        && fz < SIDE as f64)
    {
        return None;
    }
    let (x, y, z) = (fx as i64, fy as i64, fz as i64);
    let inside = |c: i64| (RADIUS..SIDE - RADIUS).contains(&c);
    (inside(x) && inside(y) && inside(z)).then_some((x, y, z))
}

/// The flat bin key for a cell, `x` outermost — the 2D key is `x * SIDE + y` unchanged.
fn key(cx: i64, cy: i64, cz: i64, dim: usize) -> i64 {
    if dim == MAX_DIM {
        (cx * SIDE + cy) * SIDE + cz
    } else {
        cx * SIDE + cy
    }
}

fn tent(offset: i64) -> f64 {
    1.0 - offset.abs() as f64 / RADIUS as f64
}

pub(super) struct Density {
    /// The coarse separable grid; empty once `bins` is taken, and empty from the start at
    /// `dim` 3 (see the module comment).
    grid: Vec<f64>,
    bins: Option<BTreeMap<i64, Vec<u32>>>,
    dim: usize,
}

impl Density {
    pub(super) fn new(dim: usize) -> Self {
        if dim == MAX_DIM {
            // Bins from the start, and no grid: the coarse grid is 2D, so at 3 the bin
            // map is both the only field and the "already fine" state. Initialising `bins`
            // here rather than only zeroing `grid` is what keeps `apply` off the grid path
            // — an empty grid indexed by cell is the out-of-bounds this shape avoids.
            return Self {
                grid: Vec::new(),
                bins: Some(BTreeMap::new()),
                dim,
            };
        }
        Self {
            grid: vec![0.0; (SIDE * SIDE) as usize],
            bins: None,
            dim,
        }
    }

    pub(super) fn is_fine(&self) -> bool {
        self.bins.is_some()
    }

    /// Drops the grid and bins every node by its current cell. A no-op at `dim` 3, whose
    /// field is binned from the start — and it must stay one, because the binned field has
    /// to *hold* the nodes, so dropping it here would empty the field mid-run.
    pub(super) fn switch_to_fine(&mut self, pos: &[[f64; MAX_DIM]]) {
        if self.dim == MAX_DIM {
            return;
        }
        self.grid = Vec::new();
        self.bins = Some(BTreeMap::new());
        for (v, &at) in pos.iter().enumerate() {
            self.add(v as u32, at);
        }
    }

    pub(super) fn add(&mut self, v: u32, at: [f64; MAX_DIM]) {
        self.apply(v, at, 1.0);
    }

    pub(super) fn remove(&mut self, v: u32, at: [f64; MAX_DIM]) {
        self.apply(v, at, -1.0);
    }

    fn apply(&mut self, v: u32, at: [f64; MAX_DIM], sign: f64) {
        let dim = self.dim;
        let Some((cx, cy, cz)) = cell(&at, dim) else {
            return;
        };
        if let Some(bins) = &mut self.bins {
            let bin = bins.entry(key(cx, cy, cz, dim)).or_default();
            if sign > 0.0 {
                bin.push(v);
            } else {
                bin.retain(|&u| u != v);
            }
            return;
        }
        for dx in -RADIUS..=RADIUS {
            let row = (cx + dx) * SIDE + cy;
            let wx = tent(dx) * sign;
            for dy in -RADIUS..=RADIUS {
                self.grid[(row + dy) as usize] += wx * tent(dy);
            }
        }
    }

    /// Density energy of node `v` sitting at `at`; `v` itself is not in the field.
    pub(super) fn energy(&self, v: u32, at: [f64; MAX_DIM], pos: &[[f64; MAX_DIM]]) -> f64 {
        let dim = self.dim;
        let Some((cx, cy, cz)) = cell(&at, dim) else {
            return WALL;
        };
        let Some(bins) = &self.bins else {
            // Only reachable at `dim` 2: at 3 `new` installs the bins. So this indexes a
            // grid that exists, and `cx * SIDE + cy` is in range because `cell` already
            // refused everything outside the view.
            let d = self.grid[(cx * SIDE + cy) as usize];
            return d * d;
        };
        let mut sum = 0.0;
        // The third offset is `0` at `dim` 2, not `0` of three: the bin key ignores `z`
        // there, so a `-1..=1` sweep would visit every 2D bin three times and triple the
        // field — a wrong number that still looks like a layout.
        let dz_range: &[i64] = if dim == MAX_DIM { &[-1, 0, 1] } else { &[0] };
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &dz in dz_range {
                    let Some(bin) = bins.get(&key(cx + dx, cy + dy, cz + dz, dim)) else {
                        continue;
                    };
                    for &u in bin.iter().filter(|&&u| u != v) {
                        let mut d = [0.0; MAX_DIM];
                        for a in 0..dim {
                            d[a] = at[a] - pos[u as usize][a];
                        }
                        sum += 1e-4 / (norm2(&d, dim) + 1e-50);
                    }
                }
            }
        }
        sum
    }
}

/// `sum of squares` over the live axes, ascending — the 2D arm's `d[0]*d[0] + d[1]*d[1]`
/// in the same order and the same bits.
fn norm2(v: &[f64; MAX_DIM], dim: usize) -> f64 {
    let mut sum = 0.0;
    for x in v.iter().take(dim) {
        sum += x * x;
    }
    sum
}
