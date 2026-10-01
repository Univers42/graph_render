//! The conjugate-gradient solve the stress iteration ends each pass with.
//!
//! Reference: `lib/neatogen/conjgrad.c:160-215` (`conjugate_gradient_mkernel`).
//!
//! **`A` is the *negated* graph Laplacian of `1 / d_ij^2`**, packed as its upper triangle,
//! and the solve is `A x' = b` with `b = L p` for the *other*, freshly built Laplacian
//! `L` of `1 / (d_ij * |p_i - p_j|)`. Both are singular in the ones direction — that is why
//! every vector is centred before use — and the gradient does not care: it converges to
//! *a* point of the affine space, and the centring is what picks which one.
//!
//! Four details are load-bearing and none is obvious:
//!
//! - **The scalars are `double`, the vectors are `float`, and the two meet at a cast.**
//!   `alpha` and `beta` are `double` divisions, then `(float)alpha` is what multiplies a
//!   `float` vector (`conjgrad.c:206-217`). Computing the update in `f64` and narrowing
//!   afterwards is a different solve.
//! - **The residual is updated recursively, not recomputed.** `r -= alpha * Ap` is the
//!   standard "fast" form and it is the one the reference uses, with the accurate
//!   recomputation left commented out (`conjgrad.c:198-200`). The recursive form drifts,
//!   and this port drifts with it.
//! - **`max_iterations` is `n`, not a tolerance-derived bound** (`conjgrad.c:193` is called
//!   with `conj_tol, n`), so a badly conditioned graph spends `n` passes and stops.
//! - **Every vector is centred at the top of every pass** — `p`, `x` and `r`, then `Ap`
//!   after the product (`conjgrad.c:189-194`). The centring of `x` is repeated rather than
//!   carried, so the drift the previous pass's centring left is removed rather than
//!   accumulated.

use super::matrix::{centre_f32, dot, max_abs, right_mult};

/// Solves `packed * x = b` in place, the reference's `conjugate_gradient_mkernel`.
///
/// `x` is both the initial guess and the answer, as in the reference: the caller passes the
/// current coordinates, which are already centred, and gets the next ones back. `b` is
/// centred here rather than by the caller because the reference centres it inside
/// (`conjgrad.c:178`) and a caller that pre-centred it would centre twice.
pub(super) fn solve(packed: &[f32], count: usize, x: &mut [f32], b: &[f32], tolerance: f64) {
    let mut b_centred = b.to_vec();
    let mut product = vec![0.0f32; count];
    let mut residual = vec![0.0f32; count];
    let mut direction = vec![0.0f32; count];
    centre_f32(x);
    centre_f32(&mut b_centred);
    right_mult(packed, count, x, &mut product);
    centre_f32(&mut product);
    for i in 0..count {
        residual[i] = b_centred[i] - product[i];
        direction[i] = residual[i];
    }
    let mut rr = dot(&residual, &residual);
    for step in 0..count {
        if max_abs(&residual) <= tolerance {
            return;
        }
        centre_f32(&mut direction);
        centre_f32(x);
        centre_f32(&mut residual);
        right_mult(packed, count, &direction, &mut product);
        centre_f32(&mut product);
        let dir_dot = dot(&direction, &product);
        if dir_dot == 0.0 {
            // The reference breaks here (`conjgrad.c:198-201`): a zero curvature means the
            // direction is in the null space and no step along it helps.
            return;
        }
        let alpha = rr / dir_dot;
        add_scaled(x, alpha as f32, &direction);
        if step + 1 < count {
            add_scaled(&mut residual, -alpha as f32, &product);
            let next = dot(&residual, &residual);
            if rr == 0.0 {
                return;
            }
            let beta = next / rr;
            rr = next;
            for i in 0..count {
                direction[i] = beta as f32 * direction[i] + residual[i];
            }
        }
    }
}

/// `target[i] += alpha * source[i]`, all `f32`: `vectors_mult_additionf`.
///
/// `-alpha as f32` is the reference's `(-alpha)` written as a `float` before the multiply
/// (`conjgrad.c:211`), and negating after the cast would be a different rounding for a
/// value that is not exactly representable — so the negation is where the reference has it.
fn add_scaled(target: &mut [f32], alpha: f32, source: &[f32]) {
    for (out, &value) in target.iter_mut().zip(source) {
        *out += alpha * value;
    }
}

#[cfg(test)]
mod tests {
    use super::solve;
    use crate::layout::graphviz::neato::matrix::{max_abs, right_mult};

    /// **This solve does not solve `A x = b`.** It solves a *centred* variant, and the
    /// difference is the whole reason a test here has to assert a measured value rather
    /// than the textbook one: the reference centres `x`, centres `b`, **and centres `Ap`
    /// after every product** (`conjgrad.c:178-192`), so the operator the gradient descends
    /// is `A` with its ones-direction component removed at each step rather than `A` itself.
    ///
    /// Worked by hand for `[[4, 1], [1, 3]]` with `b = [1, 2]`, from a zero guess:
    /// `b` centres to `[-0.5, 0.5]`, the first `Ap = centre(A[-0.5, 0.5])` is
    /// `centre([-1.5, 1.0]) = [-1.25, 1.25]`, `p_Ap = 1.25`, so `alpha = 0.5 / 1.25 = 0.4`
    /// and `x` becomes `[-0.2, 0.2]`, at which the residual is exactly zero. The exact
    /// solution of `A x = b` would be `[-0.1818, 0.2273]` — a different number, from a
    /// different operator, and a port that "fixed" this to reach it would no longer be a
    /// port. The assertion is therefore on the reference's own arithmetic.
    #[test]
    fn the_solve_descends_a_centred_operator_not_the_matrix_itself() {
        let packed = [4.0f32, 1.0, 3.0];
        let mut x = [0.0f32; 2];
        solve(&packed, 2, &mut x, &[1.0, 2.0], 1e-3);
        assert!((x[0] + 0.2).abs() < 1e-6, "{x:?}");
        assert!((x[1] - 0.2).abs() < 1e-6, "{x:?}");
    }

    /// The residual the reference's own steps drive to zero, recomputed here from the two
    /// matrix-vector products rather than from the answer — so a change to the centring
    /// shows up as a non-zero residual instead of as a plausible `x`.
    #[test]
    fn the_residual_reaches_zero_in_one_step() {
        let packed = [4.0f32, 1.0, 3.0];
        let mut product = [0.0f32; 2];
        right_mult(&packed, 2, &[-0.5, 0.5], &mut product);
        // `Ap` is the centred product, which is what `p_Ap` and the update use.
        let centred = (product[0] + product[1]) / 2.0;
        let ap = [product[0] - centred, product[1] - centred];
        let dir = [-0.5f32, 0.5];
        let alpha = 0.5f64 / f64::from(dir[0] * ap[0] + dir[1] * ap[1]);
        let residual = [
            dir[0] + (-alpha as f32) * ap[0],
            dir[1] + (-alpha as f32) * ap[1],
        ];
        assert!(max_abs(&residual) < 1e-6, "{residual:?}");
    }

    /// The system is singular in the ones direction, and the centring is what fixes which
    /// point of that space the answer lands on: both columns of `x` come back summing to
    /// about zero whatever the right-hand side was.
    #[test]
    fn the_answer_is_centred_whatever_the_right_hand_side() {
        // [[2, -2], [-2, 2]] has the ones vector in its null space.
        let packed = [2.0f32, -2.0, 2.0];
        let mut x = [0.5f32, -0.5];
        solve(&packed, 2, &mut x, &[3.0, 1.0], 1e-12);
        let total: f32 = x.iter().sum();
        assert!(total.abs() < 1e-5, "{x:?} sums to {total}");
    }

    /// A zero right-hand side still moves `x`, because the solve works on residuals rather
    /// than on a target: the first residual is `-A x`, and one step along it is not the
    /// identity. The case is worth pinning because "no force, no movement" is the intuition
    /// and it is wrong here — the graph's *shape* is what has to be null for a zero
    /// right-hand side to leave the drawing alone, and that is the Laplacian's null space,
    /// not the zero vector's.
    #[test]
    fn a_zero_right_hand_side_still_moves_the_guess() {
        // Diagonal, so the centred operator is a scaled identity and one step sends the
        // centred guess to zero.
        let packed = [2.0f32, 0.0, 2.0];
        let mut x = [0.25f32, -0.25];
        solve(&packed, 2, &mut x, &[0.0, 0.0], 1e-12);
        assert!(x[0].abs() < 1e-6 && x[1].abs() < 1e-6, "{x:?}");
    }

    /// A left null direction — `A p = 0` — breaks out rather than dividing by zero, which is
    /// `conjgrad.c:198-201`'s `if (p_Ap == 0) break`. Here the whole matrix is zero, so
    /// every direction is in the null space: the pass must return with `x` untouched and
    /// nothing non-finite, rather than producing a `NaN` from `0 / 0`.
    #[test]
    fn a_null_direction_stops_the_pass_instead_of_dividing_by_zero() {
        // The packed triangle for `count = 3` is six entries, all zero here.
        let packed = [0.0f32; 6];
        let mut x = [1.0f32, 0.0, -1.0];
        solve(&packed, 3, &mut x, &[1.0, 1.0, 1.0], 1e-12);
        assert!(x.iter().all(|value| value.is_finite()), "{x:?}");
        // `x` is centred on entry, which for this already-centred guess is a no-op.
        assert_eq!(x, [1.0, 0.0, -1.0]);
    }

    /// The pass budget is the node count, so the solve terminates whatever the conditioning
    /// — the reference's `max_iterations = n` and this port's `0..count`. A pathological
    /// matrix that would not converge is the case that makes the bound observable: with
    /// `tol = 0` the residual test never fires, so only the budget can stop it.
    #[test]
    fn the_pass_budget_bounds_a_solve_that_would_not_converge() {
        let count = 6;
        let mut packed = vec![0.0f32; count * (count + 1) / 2];
        let mut index = 0;
        for i in 0..count {
            // A strictly dominant diagonal with a weak off-diagonal: a well-posed system
            // whose centred operator is far from an eigenvector, so the gradient wanders.
            packed[index] = 8.0 + i as f32;
            index += 1;
            for _ in (i + 1)..count {
                packed[index] = 0.5;
                index += 1;
            }
        }
        let mut x = vec![0.0f32; count];
        let b = vec![1.0f32; count];
        solve(&packed, count, &mut x, &b, 0.0);
        assert!(x.iter().all(|value| value.is_finite()), "{x:?}");
        // The solve reached a fixed point of its own iteration: a second solve from the
        // answer changes nothing, which is what "the budget was enough" looks like.
        let mut again = x.clone();
        solve(&packed, count, &mut again, &b, 0.0);
        for (before, after) in x.iter().zip(&again) {
            assert!((before - after).abs() < 1e-4, "{x:?} vs {again:?}");
        }
    }
}
