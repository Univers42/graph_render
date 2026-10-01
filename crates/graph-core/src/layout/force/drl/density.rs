//! DrL's repulsive density field: a coarse 1000 x 1000 grid of separable tent kernels,
//! replaced in the last stage by per-cell node lists and a `1/d^2` sum.

use std::collections::BTreeMap;

const SIDE: i64 = 1000;
const RADIUS: i64 = 10;
const HALF_VIEW: f64 = 2000.0;
const VIEW_TO_GRID: f64 = 0.25;
/// Energy of a position on or beyond the border margin: a wall, not a measurement.
const WALL: f64 = 10_000.0;

/// Grid cell of `at`, `None` inside the border margin or outside the plane.
fn cell(at: [f64; 2]) -> Option<(i64, i64)> {
    let fx = (at[0] + HALF_VIEW) * VIEW_TO_GRID;
    let fy = (at[1] + HALF_VIEW) * VIEW_TO_GRID;
    if !(fx >= 0.0 && fy >= 0.0 && fx < SIDE as f64 && fy < SIDE as f64) {
        return None;
    }
    let (x, y) = (fx as i64, fy as i64);
    let inside = |c: i64| (RADIUS..SIDE - RADIUS).contains(&c);
    (inside(x) && inside(y)).then_some((x, y))
}

fn tent(offset: i64) -> f64 {
    1.0 - offset.abs() as f64 / RADIUS as f64
}

pub(super) struct Density {
    grid: Vec<f64>,
    bins: Option<BTreeMap<i64, Vec<u32>>>,
}

impl Density {
    pub(super) fn new() -> Self {
        Self {
            grid: vec![0.0; (SIDE * SIDE) as usize],
            bins: None,
        }
    }

    pub(super) fn is_fine(&self) -> bool {
        self.bins.is_some()
    }

    /// Drops the grid and bins every node by its current cell.
    pub(super) fn switch_to_fine(&mut self, pos: &[[f64; 2]]) {
        self.grid = Vec::new();
        self.bins = Some(BTreeMap::new());
        for (v, &at) in pos.iter().enumerate() {
            self.add(v as u32, at);
        }
    }

    pub(super) fn add(&mut self, v: u32, at: [f64; 2]) {
        self.apply(v, at, 1.0);
    }

    pub(super) fn remove(&mut self, v: u32, at: [f64; 2]) {
        self.apply(v, at, -1.0);
    }

    fn apply(&mut self, v: u32, at: [f64; 2], sign: f64) {
        let Some((cx, cy)) = cell(at) else { return };
        if let Some(bins) = &mut self.bins {
            let bin = bins.entry(cx * SIDE + cy).or_default();
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
    pub(super) fn energy(&self, v: u32, at: [f64; 2], pos: &[[f64; 2]]) -> f64 {
        let Some((cx, cy)) = cell(at) else {
            return WALL;
        };
        let Some(bins) = &self.bins else {
            let d = self.grid[(cx * SIDE + cy) as usize];
            return d * d;
        };
        let mut sum = 0.0;
        for dx in -1..=1 {
            for dy in -1..=1 {
                let Some(bin) = bins.get(&((cx + dx) * SIDE + cy + dy)) else {
                    continue;
                };
                for &u in bin.iter().filter(|&&u| u != v) {
                    let d = [at[0] - pos[u as usize][0], at[1] - pos[u as usize][1]];
                    sum += 1e-4 / (d[0] * d[0] + d[1] * d[1] + 1e-50);
                }
            }
        }
        sum
    }
}
