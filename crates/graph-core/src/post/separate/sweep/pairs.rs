//! The pair arithmetic, and the three numbers every call into it needs.
//!
//! Split out of `super` for the house 300-line limit, and because this is the part of the pass
//! worth reading on its own: given two discs and a [`Run`], how far apart they are and which way.
//!
//! **Nothing here allocates and nothing here branches on a clock or a random number.** Every
//! function is a pure function of its arguments, which is what lets the sweep be a gather (D10)
//! and the result bit-identical on every target.

use crate::post::separate::buckets::Grid;

/// The three numbers a whole run shares: the gap added to every pair, how much further than
/// the bare penetration a node moves, and the cap on sweeps.
///
/// **A struct rather than three more parameters.** These travel together through every call in
/// the pass — the cell side, the separation, the residual count — and carrying them as arguments
/// put `separation` at eight parameters, which is past both the house limit of four and clippy's
/// own. A caller changes a run by building a different `Run`, and every arithmetic expression
/// below keeps the operand order it had when these were separate arguments, so the output is
/// unchanged.
#[derive(Debug, Clone, Copy)]
pub struct Run {
    /// Extra gap added to every pair's required separation, in layout units.
    pub margin: f32,
    /// The over-relaxation factor; see [`super::OVER_RELAXATION`].
    pub omega: f32,
    /// The iteration cap; see [`crate::post::separate::sweep`]'s `max_iterations`.
    pub max_iterations: u32,
}

/// One disc: where it is and how big it is.
///
/// The three columns a pair needs, named once. `Copy` so passing a disc moves no memory and
/// cannot be mistaken for a borrow of the caller's node arrays.
#[derive(Debug, Clone, Copy)]
pub struct Disc {
    /// Centre x, in layout units.
    pub x: f32,
    /// Centre y, in layout units.
    pub y: f32,
    /// Collision radius, in layout units; see [`super::super::radii`].
    pub r: f32,
}

/// The pass's three per-node columns, borrowed: positions and radii in node order.
///
/// A borrowed view rather than a copy, so building it per node costs nothing — which is the
/// point, since the sweep builds one per displacement it gathers.
pub struct Discs<'a> {
    /// Every node's x, in node order.
    pub x: &'a [f32],
    /// Every node's y, in node order.
    pub y: &'a [f32],
    /// Every node's radius, in node order.
    pub r: &'a [f32],
}

impl Discs<'_> {
    /// Node `i`'s disc.
    pub fn at(&self, i: usize) -> Disc {
        Disc {
            x: self.x[i],
            y: self.y[i],
            r: self.r[i],
        }
    }
}

/// The separation `a` needs from `b`: half the penetration along the unit vector away from `b`,
/// or `(0, 0)` when the pair is already clear.
///
/// **Coincident discs** (`d == 0`) have no direction, so the pair separates along x — see this
/// module's parent for why that heuristic carries a marker. Note this branch is reached only
/// *between* nodes, not *within* one: `Workspace::break_symmetry` has already given every disc
/// at one point its own offset, so `d == 0` never survives the first sweep.
pub fn separation(a: Disc, b: Disc, run: Run) -> (f32, f32) {
    let (dx, dy) = (a.x - b.x, a.y - b.y);
    let need = a.r + b.r + 2.0 * run.margin;
    let d = libm::sqrtf(dx * dx + dy * dy);
    if d >= need {
        return (0.0, 0.0);
    }
    let pen = need - d;
    if d == 0.0 {
        return (pen * 0.5 * run.omega, 0.0);
    }
    let scale = run.omega * pen * 0.5 / d;
    (dx * scale, dy * scale)
}

/// Whether a pair is closer than it should be by more than [`super::TOLERANCE`].
pub fn overlapping(a: Disc, b: Disc, run: Run) -> bool {
    let (dx, dy) = (a.x - b.x, a.y - b.y);
    let need = a.r + b.r + 2.0 * run.margin - super::TOLERANCE;
    libm::sqrtf(dx * dx + dy * dy) < need
}

/// The cell side, `2 · (largest + margin)`, and never zero: an all-zero layout has no discs
/// and no pairs, so the sweep is a no-op and the side only has to keep the division finite.
///
/// The factor of 2 is exact rather than generous: the largest separation any pair can need
/// is `largest + largest + 2 · margin`, which is this. See `buckets` for why that
/// makes a 3 × 3 walk complete.
pub fn cell_side(radii: &[f32], run: Run) -> f32 {
    let largest = crate::post::separate::largest(radii);
    let side = 2.0 * (largest + run.margin);
    if side > 0.0 { side } else { 1.0 }
}

/// Node `node`'s displacement: the sum of every separation vector it is inside of.
///
/// The sum runs over an ascending node index — the buckets are a counting sort over dense node
/// order — so the reduction is fixed-order and identical on every target (D2).
pub fn gather(grid: &Grid, discs: &Discs, run: Run, node: usize) -> (f32, f32) {
    let mut total = (0.0f32, 0.0f32);
    each_neighbour(grid, node, |other| {
        let other = other as usize;
        if other == node {
            return;
        }
        let (ux, uy) = separation(discs.at(node), discs.at(other), run);
        total.0 += ux;
        total.1 += uy;
    });
    total
}

/// Node `node`'s neighbours: every node sharing its cell or one of the eight around it.
///
/// Returned as a **borrowed slice of one cell's nodes at a time**, walked here rather than
/// returned as one iterator — the nine-cell walk needs to borrow the grid's buckets for the
/// whole walk, and a returned iterator would have to own the neighbourhood to outlive it. The
/// order is ascending flat cell index and ascending node index within each cell, so the
/// caller's reduction is fixed-order (D2).
pub fn each_neighbour(grid: &Grid, node: usize, mut visit: impl FnMut(u32)) {
    let hood = grid.around(node);
    for cell in hood.cells() {
        for other in grid.buckets.of(*cell) {
            visit(*other);
        }
    }
}
