//! The stress majorization loop: `stress_majorization_kD_mkernel` at `dim = 2`.
//!
//! Reference: `lib/neatogen/stress.c:782-1100`, with `initLayout` at `:131-160`.
//!
//! One pass, in the reference's order, is three named steps and a solve:
//!
//! 1. **Build `lap1`**, the Laplacian of `1 / (d_ij * |p_i - p_j|)`
//!    (`stress.c:983-1031`): the squared distances of one row, `1/d`, an overflow guard, a
//!    scale by `1/d_ij`, and the diagonal as the negated row sum.
//! 2. **Right-hand side**, `b = lap1 * p` per column (`stress.c:1034-1037`).
//! 3. **Stress** (`stress.c:1040-1057`), a `double` and the *only* stopping test.
//! 4. **Solve** `lap2 x' = b`, where `lap2 = 1 / d_ij^2` is built once before the loop and
//!    never changes (`stress.c:936-947`), by [`conjugate`].
//!
//! **The convergence test is evaluated before the solve, and the solve runs anyway.** The
//! reference computes `converged` from the stress it has just measured, records the stress,
//! and *then* takes the conjugate-gradient step (`stress.c:1059-1075`), so the pass that
//! discovers convergence is also the last pass that moves the drawing. Testing after the
//! solve would drop a step and land somewhere else.
//!
//! **The two precisions meet here and nowhere else.** The answer is kept in `f64` between
//! passes and re-narrowed at the top of each one (`stress.c:919-922`), which is what makes
//! this the place the reference's `double` coordinates and `float` iteration touch. Widening
//! the iteration to `f64` throughout would be a better layout and a different one.
//!
//! **Not gather form, on purpose.** Every entry of `lap1` needs every other entry of `p` in
//! its row, and the conjugate gradient's direction depends on the whole residual, so no two
//! nodes' updates are independent. D10's condition is not met; the parent says so rather
//! than implying it, and every reduction below is fixed-order so a parallel version, if one
//! were ever written, would have an order to agree with.

mod initial;

use super::conjugate;
use super::distance::Packed;
use super::matrix::{OVERFLOW, dot, invert, invert_sqrt_in_place, right_mult, row_sums, sqrt_into};
use super::{CG_TOLERANCE, STRESS_WEIGHT};
use crate::csr::Csr;
use initial::initial_placement;

/// The drawing, in the reference's own units: the `double` columns `initLayout` wrote and
/// the iteration left behind (`stress.c:1093-1098`).
#[derive(Debug)]
pub(super) struct Solution {
    /// The x column, in the reference's `ND_pos` units.
    pub(super) x: Vec<f64>,
    /// The y column, in the same units.
    pub(super) y: Vec<f64>,
}

/// The stress model at `count` nodes, seeded by `seed`, run for at most `max_iterations`
/// passes or until the stress's relative change falls below `epsilon`.
///
/// Five arguments because each is a distinct decision the reference makes in a distinct
/// place — the graph, its size, `start`, `maxiter`, `epsilon`. A `Config` would move five
/// choices into a struct literal and make five of the reference's knobs read as one.
pub(super) fn majorize(
    neighbours: &Csr,
    count: usize,
    seed: u32,
    max_iterations: u32,
    epsilon: f64,
) -> Solution {
    let distances = Packed::of_hops(neighbours, count);
    let mut state = State::of(&distances, count);
    let (mut x, mut y) = initial_placement(count, seed);
    for _ in 0..max_iterations {
        if state.pass(count, epsilon, &mut x, &mut y) {
            break;
        }
    }
    Solution { x, y }
}

/// Every buffer the passes share, allocated once as the reference allocates them once
/// (`stress.c:925-971`).
struct State {
    /// `1 / d_ij^2`, packed, with the Laplacian diagonal written in. Never changes.
    weights: Vec<f32>,
    /// `1 / (d_ij * |p_i - p_j|)`, packed, rebuilt every pass.
    weighted: Vec<f32>,
    /// The coordinates, narrowed: x in `0..count`, y in `count..2*count`. One array, so a
    /// column is a contiguous slice and the reference's `f_storage + k * n` becomes a range.
    coords: Vec<f32>,
    /// The right-hand sides, laid out as `coords` is.
    rhs: Vec<f32>,
    /// One column of scratch: the stress's second product, and the solve's internals.
    scratch: Vec<f32>,
    /// The squared distances of one row, the only per-pass temporary.
    distances: Vec<f32>,
    /// `(float)n * (n - 1) / 2` (`stress.c:931`): a `float` constant term, not a count.
    constant_term: f32,
    /// The previous pass's stress, `DBL_MAX` before the first so the very first pass cannot
    /// read as converged (`stress.c:973`).
    previous: f64,
}

impl State {
    /// `lap2` from the distances, and the buffers, in the reference's order.
    fn of(distances: &Packed, count: usize) -> Self {
        let mut weights = distances.as_slice().to_vec();
        // `stresswt = 2` squares every entry and then inverts it (`stress.c:936-940`), so
        // `lap2 = 1 / d_ij^2`. Written as a count of squarings rather than one `*= *=` so
        // the reference's `exp` is load-bearing here: `exp = 1` would leave the entry
        // unsquared and this port would then be a different layout, visibly, rather than
        // silently. The diagonal `d_ii = 0` squares to a zero and `invert` leaves a zero
        // alone, which is right: the diagonal is overwritten with a degree two lines later.
        for _ in 1..STRESS_WEIGHT {
            for value in weights.iter_mut() {
                *value *= *value;
            }
        }
        invert(&mut weights);
        let degrees = row_sums(&weights, count);
        write_diagonal(&mut weights, &degrees, count);
        Self {
            weights,
            weighted: vec![0.0; count * (count + 1) / 2],
            coords: vec![0.0; count * 2],
            rhs: vec![0.0; count * 2],
            scratch: vec![0.0; count],
            distances: vec![0.0; count],
            constant_term: count as f32 * (count as f32 - 1.0) / 2.0,
            previous: f64::MAX,
        }
    }

    /// One pass. Returns whether the iteration has converged, having already taken the
    /// step — the reference's ordering, and the reason the last pass still moves the drawing.
    fn pass(&mut self, count: usize, epsilon: f64, x: &mut [f64], y: &mut [f64]) -> bool {
        self.narrow(x, y);
        self.build_weighted(count);
        self.right_sides(count);
        let stress = self.stress(count);
        let converged = change_is_small(self.previous, stress, epsilon);
        self.previous = stress;
        self.solve(count);
        self.widen(count, x, y);
        converged
    }

    /// The `f64` columns into the iteration's `f32` array (`stress.c:919-922`).
    fn narrow(&mut self, x: &[f64], y: &[f64]) {
        for (slot, &value) in self.coords.iter_mut().zip(x.iter().chain(y)) {
            *slot = value as f32;
        }
    }

    /// The `f32` array back out, once per pass (`stress.c:1093-1098`).
    fn widen(&mut self, count: usize, x: &mut [f64], y: &mut [f64]) {
        for (out, &value) in x.iter_mut().zip(&self.coords[..count]) {
            *out = f64::from(value);
        }
        for (out, &value) in y.iter_mut().zip(&self.coords[count..]) {
            *out = f64::from(value);
        }
    }

    /// `lap1`: `sqrt(1/d_ij^2)` scaled by `1/|p_i - p_j|`, diagonal as the negated row sum
    /// (`stress.c:983-1031`).
    ///
    /// The whole packed array is square-rooted first, diagonal included, but the diagonal
    /// entries are negative degrees and `sqrt_into` leaves a negative alone — so they keep
    /// whatever the previous pass left and are then overwritten. The reference gets away
    /// with that because `lap1` is `gv_calloc`ed and never read before it is written; this
    /// reads no diagonal either, for the same reason and by the same construction.
    fn build_weighted(&mut self, count: usize) {
        sqrt_into(&self.weights, &mut self.weighted);
        let mut index = 0;
        for i in 0..count {
            let len = count - i - 1;
            self.distances[..len]
                .iter_mut()
                .for_each(|slot| *slot = 0.0);
            for column in 0..2 {
                let head = self.coords[column * count + i];
                let tail = column * count + i + 1;
                for (slot, &other) in self.distances[..len].iter_mut().zip(&self.coords[tail..]) {
                    // The reference writes `coords[k][i] + -1.0f * coords[k][i+1+x]`
                    // (`stress.c:996`). Negating an `f32` is exact, so this is the same
                    // subtraction rather than an equivalent-looking rewrite.
                    let delta = head - other;
                    *slot += delta * delta;
                }
            }
            invert_sqrt_in_place(&mut self.distances[..len]);
            for slot in self.distances[..len].iter_mut() {
                if *slot >= OVERFLOW || *slot < 0.0 {
                    *slot = 0.0;
                }
            }
            index += 1;
            for j in 0..len {
                self.weighted[index] *= self.distances[j];
                index += 1;
            }
        }
        let degrees = row_sums(&self.weighted, count);
        write_diagonal(&mut self.weighted, &degrees, count);
    }

    /// `b = lap1 * p`, per column (`stress.c:1034-1037`).
    fn right_sides(&mut self, count: usize) {
        for column in 0..2 {
            let range = column * count..(column + 1) * count;
            right_mult(
                &self.weighted,
                count,
                &self.coords[range.clone()],
                &mut self.rhs[range],
            );
        }
    }

    /// The stress of the current drawing (`stress.c:1040-1057`).
    ///
    /// Both Laplacians are negated — the diagonal carries the negated row sum — which is why
    /// the reference adds the first inner product and subtracts the second. Written as the
    /// reference writes it, `2 * <p, L1 p> + constant - <p, L2 p>`, rather than as the
    /// stress function itself would suggest.
    fn stress(&mut self, count: usize) -> f64 {
        let mut total = 0.0f64;
        for column in 0..2 {
            let range = column * count..(column + 1) * count;
            total += dot(&self.coords[range.clone()], &self.rhs[range]);
        }
        let mut stress = total * 2.0 + f64::from(self.constant_term);
        for column in 0..2 {
            let range = column * count..(column + 1) * count;
            right_mult(
                &self.weights,
                count,
                &self.coords[range.clone()],
                &mut self.scratch,
            );
            stress -= dot(&self.coords[range], &self.scratch);
        }
        stress
    }

    /// The `lap1` the last pass built, for a test that needs the matrix rather than the
    /// coordinates. Exposed under `cfg(test)` alone: nothing in the layout reads it, and a
    /// reader looking for a use would be misled.
    #[cfg(test)]
    fn weighted(&self) -> &[f32] {
        &self.weighted
    }

    /// The fixed `lap2 = 1 / d_ij^2`, for the same reason and with the same caveat.
    #[cfg(test)]
    fn weights(&self) -> &[f32] {
        &self.weights
    }

    /// `lap2 x' = b` per column, in place (`stress.c:1068-1075`).
    fn solve(&mut self, count: usize) {
        for column in 0..2 {
            let range = column * count..(column + 1) * count;
            conjugate::solve(
                &self.weights,
                count,
                &mut self.coords[range.clone()],
                &self.rhs[range],
                CG_TOLERANCE,
            );
        }
    }
}

/// The reference's convergence test (`stress.c:1059-1066`): the relative change in the
/// stress, or the stress below `epsilon` outright.
///
/// The `fabs` is the reference's own — "in theory `old >= new` but we use `fabs` in case of
/// numerical error" — and it matters, because a pass that very slightly *increases* the
/// stress stops the iteration rather than running it to the budget.
fn change_is_small(previous: f64, current: f64, epsilon: f64) -> bool {
    (previous - current).abs() / previous < epsilon || current < epsilon
}

/// Writes the negated row sums onto the packed diagonal, in the reference's stride:
/// `for (step = n, count = 0, i = 0; i < n; i++, count += step, step--)`.
fn write_diagonal(packed: &mut [f32], degrees: &[f64], count: usize) {
    let (mut step, mut slot) = (count, 0);
    for &degree in degrees.iter().take(count) {
        packed[slot] = degree as f32;
        slot += step;
        step -= 1;
    }
}

#[cfg(test)]
mod tests;
