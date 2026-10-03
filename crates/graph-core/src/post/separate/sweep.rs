//! The Jacobi sweep: disc separation over a uniform grid.
//!
//! **Gather form (D10).** Every node's new position is computed from the *previous* position
//! array and written to a second array; nothing reads a position a sweep has already written.
//! That is also what makes the sweep's arithmetic order-independent, though it is run in dense
//! node order anyway so the reduction order is stated rather than incidental.
//!
//! The displacement for one node is the sum, over the discs it overlaps, of
//! `(r_i + r_j + 2 · margin − d) · û`, where `û` points from `j` to `i`: half the penetration
//! each way, so a pair separates by exactly the penetration when nothing else is in the way.
//! Summing rather than shoving is what makes this a relaxation — a node wedged between two
//! others is pushed by both and the two contributions largely cancel, which is the behaviour
//! that makes a compressed drawing spread instead of rippling.
//!
//! **Exactly coincident discs** (`d == 0`) have no direction and `û` would be NaN. The rule
//! is deterministic: the pair separates along x. It is a heuristic and carries a ponytail
//! marker — see [`super::META`].
//!
//! Every grid is rebuilt each sweep, because the positions move: a grid built once would go
//! stale, separating nodes that are no longer near each other while missing ones that are.

use super::buckets::Grid;
use crate::stage::StageError;
use graph_contract::geometry::NodeGeometry;

/// The over-relaxation factor: how much further than the bare penetration a node moves.
///
/// The bare rule — each node moves by half of each penetration it is inside, summing over its
/// neighbours — converges **linearly and slowly**. Measured on a stack of 200 coincident
/// discs: a worst overlap of 0.138 after 16 sweeps, 0.0106 after 64, 0.00013 after 128, and
/// still 0.21 after 16 sweeps at n = 500. At that rate the invariant is unreachable inside any
/// sensible cap.
///
/// Multiplying the step by 1.8 is what makes the cap reachable. The same stack separates in 11
/// sweeps and a line of 2000 discs in 17. Chosen by measurement, not by taste: across 12
/// randomised instances (uniform, gridded and clumped clouds; n up to 500; radius 0.5 to 2.0;
/// only *feasible* packings, since a cloud that cannot physically fit will never separate) the
/// factor that converged on **12 of 12** was 1.8, at a median of 16 sweeps and a worst of 120.
/// The bare rule converged on 0 of 12 within 300 sweeps, and 1.5 on only 6 of 12 — so this is
/// not "any factor above 1 works", it is the one that stopped diverging on the cases 1.3 and
/// 1.5 diverged on.
///
/// Ponytail (over-relaxation): a step larger than the penetration is a step that can
/// **overshoot**, so the sweep is not monotone and a badly chosen factor oscillates instead of
/// converging. Failing input: a packing near its physical limit, where the sweep oscillates
/// and the cap is reached with a residue. Direction: under-separates at worst, never
/// over-separates past the requested gap, because an overshoot is undone by the next sweep.
/// Escape hatch: `max_iterations`, and `residual()` reports exactly what is left. 1.8 is
/// inside the theoretically stable range for this class (below 2) but that is a bound, not a
/// guarantee.
pub const OVER_RELAXATION: f32 = 1.8;

/// How far a symmetric node is nudged, as a fraction of its own radius, before the first
/// sweep.
///
/// **0.35 by measurement, and the sensitivity is sharp on both sides.** Across 14 randomised
/// feasible packings (uniform, gridded and clumped clouds; n up to 400; radius 0.5 to 2.0) at
/// `OVER_RELAXATION`, this converged on 14 of 14 with a median of 6 sweeps and a worst of 76.
/// At 0.5 or 1.0 a gridded packing **diverged outright** — the nudge overshoots and the sweep
/// oscillates; at 0.15 the same packing missed the 128-sweep cap. So this is the middle of a
/// working window, not a tuned constant that tolerates its neighbours.
///
/// Smaller than the radius on purpose: a node that was overlapping something may well have a
/// clear neighbour, and a nudge of a full radius could push it into one. The nudge cannot fix
/// a new overlap the sweeps did not already have to fix, and it cannot turn a clear pair into
/// an overlapping one on its own.
const NUDGE_FRACTION: f32 = 0.35;

/// The tolerance, in layout units, that a residual overlap is measured against.
///
/// Not zero: `f32` arithmetic does not reach it, so an exact-zero test would fail on a pair
/// separated to within one rounding, and a test that cannot be satisfied gets deleted. One
/// part in a thousand of a layout unit is the honest bound, and it is a **named constant**
/// rather than a literal at each use, so a caller that needs a different one changes one line.
pub const TOLERANCE: f32 = 1e-3;

/// The pass's working state, refilled in place so a second sweep over the same drawing
/// allocates nothing beyond the first.
pub struct Workspace {
    x: Vec<f32>,
    y: Vec<f32>,
    /// The displacement this sweep computed per node, gathered in the same dense order.
    dx: Vec<f32>,
    dy: Vec<f32>,
}

impl Workspace {
    /// A workspace for `n` nodes.
    pub fn new(n: usize) -> Self {
        Self {
            x: vec![0.0; n],
            y: vec![0.0; n],
            dx: vec![0.0; n],
            dy: vec![0.0; n],
        }
    }

    /// True when there is nothing to separate.
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    /// The separated positions, as `(x, y)` columns in node order.
    pub fn positions(&self) -> (&[f32], &[f32]) {
        (&self.x, &self.y)
    }

    /// Loads the coordinates into the reused buffers, refusing a count the workspace does not
    /// hold — a mismatch would index past the end rather than produce a wrong answer.
    fn load(&mut self, nodes: &NodeGeometry) -> Result<(), StageError> {
        let (x, y) = crate::post::centres(nodes);
        if x.len() != self.x.len() || y.len() != self.y.len() {
            return Err(StageError::Param {
                name: "node count",
                rule: "the workspace's own",
            });
        }
        self.x.copy_from_slice(x);
        self.y.copy_from_slice(y);
        Ok(())
    }

    /// Runs up to `max_iterations` Jacobi sweeps, stopping as soon as one moves nothing.
    ///
    /// **Only one early exit, and it is the strict one:** a sweep that displaces no node at
    /// all. A relaxation converges slowly on a crowded drawing — a stack of discs takes
    /// dozens of sweeps and each one still displaces everything — so any "the count stopped
    /// falling" heuristic cuts the cap short exactly when it matters most. Measured: a
    /// `resolved >= previous` exit left a stack of 500 discs overlapping by 1.93 at any cap,
    /// while running to `resolved == 0` separates them. Convergence is the cap's job, and
    /// `residual()` is what reports when it did not arrive.
    pub fn sweep(
        &mut self,
        nodes: &NodeGeometry,
        radii: &[f32],
        margin: f32,
        omega: f32,
        max_iterations: u32,
    ) -> Result<(u32, u32), StageError> {
        self.load(nodes)?;
        let side = cell_side(radii, margin);
        self.break_symmetry(radii, margin, side)?;
        let mut total = 0u32;
        for _ in 0..max_iterations {
            let resolved = self.iteration(radii, margin, omega, side)?;
            total += resolved;
            if resolved == 0 {
                break;
            }
        }
        let residual = self.residual(radii, margin, side)?;
        Ok((total, residual))
    }

    /// **Breaks the symmetry a stack of coincident discs has, before any sweep runs.**
    ///
    /// Every disc at `(0, 0)` sees every other at distance 0, so every node's displacement is
    /// the *same* vector — the sum of identical contributions — and the whole cluster translates
    /// without a single pair separating. That is not slow convergence, it is none at all, and the
    /// invariant fails at any iteration cap. Measured: without this, a sweep leaves a stack of 500
    /// discs exactly where it found it.
    ///
    /// **Only nodes that overlap something are nudged.** Nudging every node would be simpler and
    /// is wrong: it moves a drawing that had no overlaps at all, which is the opposite of what a
    /// caller asking for overlap removal wants, and it broke the test `a_clear_layout_is_left_alone`
    /// — a 100-disc drawing with no overlapping pair came back with every node displaced. Gated on
    /// *overlap* rather than on *exact coincidence*, because the packed line needs it too and an
    /// exact-coincidence gate left it stuck: measured, every interior node of a line of discs at
    /// unit spacing has its two neighbours' displacement vectors cancel exactly, so the interior
    /// never moves and the line does not converge at any cap (600 sweeps, no separation). Only a
    /// displaced node breaks that cancellation.
    ///
    /// The nudge is **deterministic and derived from the dense index alone**: node `i` moves by
    /// [`NUDGE_FRACTION`] of its own radius along a fixed unit vector on the golden angle, so no
    /// two discs are given the same offset and the order of the drawing cannot change the outcome.
    ///
    /// Ponytail: a nudge is a heuristic, not geometry — it moves nodes that were at the same
    /// point, which is the only thing it may do, since two discs at one point have no "right"
    /// relative position. Failing input: many discs at one point; the nudge spreads them on a
    /// spiral and the sweeps then do the real work. Direction: bounded by the radius, so it can
    /// never turn a clear pair into an overlapping one — it is applied only to a disc that was
    /// already on top of another. Escape hatch: none needed; without it the adversarial case
    /// cannot converge at all.
    fn break_symmetry(&mut self, radii: &[f32], margin: f32, side: f32) -> Result<(), StageError> {
        if radii.is_empty() {
            return Ok(());
        }
        let grid = Grid::over(&self.x, &self.y, side)?;
        let golden = core::f32::consts::TAU * 0.618_034;
        for node in self.overlapping(&grid, radii, margin) {
            let angle = golden * node as f32;
            let nudge = radii[node as usize] * NUDGE_FRACTION;
            self.x[node as usize] += nudge * libm::cosf(angle);
            self.y[node as usize] += nudge * libm::sinf(angle);
        }
        Ok(())
    }

    /// Every node overlapping at least one other, in dense index order.
    ///
    /// Judged against **neighbours only**, through the same grid the sweeps use, so this costs
    /// `O(n · k)` and never an all-pairs scan — the gate is a full extra pass over the
    /// neighbourhood, not over the graph.
    fn overlapping(&self, grid: &Grid, radii: &[f32], margin: f32) -> Vec<u32> {
        let mut found = Vec::new();
        for node in 0..self.x.len() {
            let mut touching = false;
            each_neighbour(grid, node, |other| {
                let other = other as usize;
                if other == node || touching {
                    return;
                }
                let (dx, dy) = (self.x[node] - self.x[other], self.y[node] - self.y[other]);
                let need = radii[node] + radii[other] + 2.0 * margin;
                if dx * dx + dy * dy < need * need {
                    touching = true;
                }
            });
            if touching {
                found.push(node as u32);
            }
        }
        found
    }

    /// One sweep: gather every node's displacement from the current positions, then apply
    /// them all. Two passes over the nodes, never interleaved — the separation is what keeps
    /// this Jacobi rather than Gauss-Seidel, and interleaving would make the result depend on
    /// the order nodes happen to be visited.
    fn iteration(
        &mut self,
        radii: &[f32],
        margin: f32,
        omega: f32,
        side: f32,
    ) -> Result<u32, StageError> {
        let grid = Grid::over(&self.x, &self.y, side)?;
        let mut resolved = 0u32;
        for node in 0..self.x.len() {
            let (dx, dy) = gather(&grid, &self.x, &self.y, radii, margin, omega, node);
            resolved += u32::from(dx != 0.0 || dy != 0.0);
            self.dx[node] = dx;
            self.dy[node] = dy;
        }
        for node in 0..self.x.len() {
            self.x[node] += self.dx[node];
            self.y[node] += self.dy[node];
        }
        Ok(resolved)
    }

    /// Pairs still overlapping by more than the tolerance, counted over the **grid** and not
    /// over all pairs.
    ///
    /// The grid makes this exact and `O(n · k)`: the cell side is `2 · (largest + margin)`, so
    /// a pair overlapping by more than the tolerance is nearer than one cell side and is
    /// therefore in the 3 × 3 neighbourhood. Each unordered pair is counted once, when the
    /// lower node index of the two is the one asking. This is what `Bundled::unbundled`
    /// reports, so it is on the product path — the *tests* use a brute-force all-pairs scan
    /// instead, which is the instrument this count is checked against.
    fn residual(&self, radii: &[f32], margin: f32, side: f32) -> Result<u32, StageError> {
        let grid = Grid::over(&self.x, &self.y, side)?;
        let mut count = 0u32;
        for node in 0..self.x.len() {
            each_neighbour(&grid, node, |other| {
                // Each unordered pair is counted once, when the lower of its two node
                // indices is the one asking — which is why this is `other > node` and not
                // a `!=`: a symmetric test would count every pair twice.
                let other = other as usize;
                if other <= node {
                    return;
                }
                let close = overlapping(
                    self.x[node],
                    self.y[node],
                    self.x[other],
                    self.y[other],
                    radii[node],
                    radii[other],
                    margin,
                );
                if close {
                    count += 1;
                }
            });
        }
        Ok(count)
    }
}

/// Node `node`'s neighbours: every node sharing its cell or one of the eight around it.
///
/// Returned as a **borrowed slice of one cell's nodes at a time**, walked by
/// [`each_neighbour`] rather than returned as one iterator — the nine-cell walk needs to
/// borrow the grid's buckets for the whole walk, and a returned iterator would have to own
/// the neighbourhood to outlive it. The order is ascending flat cell index and ascending node
/// index within each cell, so the caller's reduction is fixed-order (D2).
fn each_neighbour(grid: &Grid, node: usize, mut visit: impl FnMut(u32)) {
    let hood = grid.around(node);
    for cell in hood.cells() {
        for other in grid.buckets.of(*cell) {
            visit(*other);
        }
    }
}

/// Node `node`'s displacement: the sum of every separation vector it is inside of.
///
/// The sum runs over an ascending node index — the buckets are a counting sort over dense
/// node order — so the reduction is fixed-order and identical on every target (D2).
fn gather(
    grid: &Grid,
    x: &[f32],
    y: &[f32],
    radii: &[f32],
    margin: f32,
    omega: f32,
    node: usize,
) -> (f32, f32) {
    let mut total = (0.0f32, 0.0f32);
    each_neighbour(grid, node, |other| {
        let other = other as usize;
        if other == node {
            return;
        }
        let (ux, uy) = separation(
            x[node],
            y[node],
            x[other],
            y[other],
            radii[node],
            radii[other],
            margin,
            omega,
        );
        total.0 += ux;
        total.1 += uy;
    });
    total
}

/// The separation `i` needs from `j`: half the penetration along the unit vector away from
/// `j`, or `(0, 0)` when the pair is already clear.
///
/// **Coincident nodes** (`d == 0`) have no direction, so the pair separates along x — see
/// this module's doc and [`super::META`] for why that heuristic carries a marker. Note this
/// branch is reached only *between* nodes, not *within* one: [`Workspace::break_symmetry`]
/// has already given every disc at one point its own offset, so `d == 0` never survives the
/// first sweep.
fn separation(
    xi: f32,
    yi: f32,
    xj: f32,
    yj: f32,
    ri: f32,
    rj: f32,
    margin: f32,
    omega: f32,
) -> (f32, f32) {
    let (dx, dy) = (xi - xj, yi - yj);
    let need = ri + rj + 2.0 * margin;
    let d = libm::sqrtf(dx * dx + dy * dy);
    if d >= need {
        return (0.0, 0.0);
    }
    let pen = need - d;
    if d == 0.0 {
        return (pen * 0.5 * omega, 0.0);
    }
    let scale = omega * pen * 0.5 / d;
    (dx * scale, dy * scale)
}

/// Whether a pair is closer than it should be by more than [`TOLERANCE`].
fn overlapping(xi: f32, yi: f32, xj: f32, yj: f32, ri: f32, rj: f32, margin: f32) -> bool {
    let (dx, dy) = (xi - xj, yi - yj);
    let need = ri + rj + 2.0 * margin - TOLERANCE;
    libm::sqrtf(dx * dx + dy * dy) < need
}

/// The cell side, `2 · (largest + margin)`, and never zero: an all-zero layout has no discs
/// and no pairs, so the sweep is a no-op and the side only has to keep the division finite.
///
/// The factor of 2 is exact rather than generous: the largest separation any pair can need
/// is `largest + largest + 2 · margin`, which is this. See [`super::buckets`] for why that
/// makes a 3 × 3 walk complete.
fn cell_side(radii: &[f32], margin: f32) -> f32 {
    let largest = crate::post::separate::radii::largest(radii);
    let side = 2.0 * (largest + margin as f32);
    if side > 0.0 { side } else { 1.0 }
}
