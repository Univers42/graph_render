//! The stress iteration's own control: the stopping rule, the initial placement, the packed
//! stride, and the matrices the passes build.
//!
//! These are the cases where the reference does something a reader would not choose — the
//! convergence test is evaluated *before* the solve and the solve runs anyway, the budget is
//! the node count, `DBL_MAX` seeds the first pass — so each is pinned with the worked numbers
//! that make it the reference's behaviour rather than a plausible one.

use super::{Packed, State, change_is_small, initial_placement, majorize, write_diagonal};
use crate::csr::Csr;
use crate::layout::graphviz::neato::MAX_ITERATIONS as BUDGET;
use crate::layout::graphviz::neato::matrix::{right_mult, row_sums};
use crate::layout::graphviz::neato::rng::Drand48;

/// `count` nodes in a ring, the fixture the convergence cases share: one connected cycle, no
/// leaf, so nothing here depends on a tie-break.
fn ring(count: u32) -> Csr {
    let pairs: Vec<(u32, u32)> = (0..count)
        .flat_map(|i| [(i, (i + 1) % count), ((i + 1) % count, i)])
        .collect();
    Csr::from_pairs(count, pairs.into_iter()).expect("rows in range")
}

/// The reference's stopping rule, in the two shapes that matter: a small *relative* change
/// stops the iteration, and so does a stress that has collapsed below the tolerance outright.
/// A stress that *rises* by less than the tolerance also stops, because the reference takes
/// `fabs` of the change rather than trusting that the stress descends monotonically.
///
/// The two clauses are not the same test, and the second is not a special case of the first:
/// a stress of `0.5` from `100` is a change of `0.5`, far above `1e-4` relatively, so only a
/// stress below `epsilon` *itself* stops it.
#[test]
fn a_small_relative_change_or_a_collapsed_stress_stops_the_iteration() {
    assert!(change_is_small(100.0, 99.999, 1e-4));
    assert!(change_is_small(100.0, 100.001, 1e-4));
    assert!(change_is_small(100.0, 1e-5, 1e-4));
    assert!(!change_is_small(100.0, 0.5, 1e-4));
    assert!(!change_is_small(100.0, 90.0, 1e-4));
    // The first pass reads `old_stress = DBL_MAX`, so its relative change is effectively zero
    // and it cannot be mistaken for converged — which is why the reference seeds it with the
    // maximum rather than with zero.
    assert!(!change_is_small(f64::MAX, 1.0, 1e-4));
}

/// The initial placement is two draws per node in x-then-y order, then each column centred —
/// so node 0's x is the *first* draw and node 0's y the *second*, not the second node's x.
/// Getting this order wrong changes the drawing while leaving every marginal distribution
/// looking right: two `Vec::map`s over the same generator draw all of x before any of y,
/// which is a different picture with the same spread.
#[test]
fn the_initial_placement_reads_two_draws_per_node_in_x_then_y_order() {
    let (x, y) = initial_placement(3, 1);
    let mut rng = Drand48::seeded(1);
    let mut want_x: Vec<f64> = Vec::new();
    let mut want_y: Vec<f64> = Vec::new();
    for _ in 0..3 {
        want_x.push(rng.next());
        want_y.push(rng.next());
    }
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    let shift = |v: &mut Vec<f64>| {
        let m = mean(v);
        for value in v.iter_mut() {
            *value -= m;
        }
    };
    shift(&mut want_x);
    shift(&mut want_y);
    assert_eq!(x, want_x);
    assert_eq!(y, want_y);
}

/// The packed diagonal is written in the reference's stride, which is not `i * count`: row 0's
/// diagonal is at 0, row 1's at `count`, row 2's at `count + (count - 1)`, and the last row's
/// at the final entry alone.
#[test]
fn the_packed_diagonal_is_written_at_the_reference_stride() {
    let count = 4;
    let mut packed = vec![0.0f32; count * (count + 1) / 2];
    write_diagonal(&mut packed, &[-1.0, -2.0, -3.0, -4.0], count);
    assert_eq!(packed[0], -1.0);
    assert_eq!(packed[4], -2.0);
    assert_eq!(packed[7], -3.0);
    assert_eq!(packed[9], -4.0);
}

/// A cycle converges to a drawing whose columns are centred, which is the visible consequence
/// of every conjugate-gradient pass centring `x` first. Asserted as a property of the result
/// rather than against a run's numbers.
#[test]
fn a_converged_cycle_comes_back_centred_and_finite() {
    let got = majorize(&ring(9), 9, 1, BUDGET, 1e-4);
    assert_eq!(got.x.len(), 9);
    assert_eq!(got.y.len(), 9);
    for column in [&got.x, &got.y] {
        let total: f64 = column.iter().sum();
        assert!(total.abs() < 1e-2, "column sums to {total}");
        assert!(column.iter().all(|v| v.is_finite()));
    }
}

/// Once the stopping rule has fired the extra passes are not taken, so a budget *past* the
/// convergence point is not a different answer. An 8-cycle needs 27 passes, so 32 and 200
/// agree exactly and 4 does not — the short budget is the negative control, and without it a
/// `pass` that stopped on its first iteration would also "agree".
#[test]
fn a_budget_past_convergence_is_not_a_different_answer() {
    let past = majorize(&ring(8), 8, 1, 32, 1e-4);
    let full = majorize(&ring(8), 8, 1, BUDGET, 1e-4);
    let agreed = past.x.iter().zip(&full.x).all(|(a, b)| a == b);
    assert!(agreed, "{:?} vs {:?}", past.x, full.x);
    let short = majorize(&ring(8), 8, 1, 4, 1e-4);
    let differs = short
        .x
        .iter()
        .zip(&full.x)
        .any(|(a, b)| (a - b).abs() > 1e-3);
    assert!(
        differs,
        "the budget is not observable: {short:?} vs {full:?}"
    );
}

/// A zero budget draws the initial placement and nothing else — the reference's
/// `if (n == 1 || maxi == 0)` early return (`stress.c:909-912`), and the strongest statement
/// that the iteration is what this port reproduces and not the placement.
#[test]
fn a_zero_budget_is_the_initial_placement_untouched() {
    let (want_x, want_y) = initial_placement(7, 1);
    let got = majorize(&ring(7), 7, 1, 0, 1e-4);
    assert_eq!(got.x, want_x);
    assert_eq!(got.y, want_y);
}

/// **The real `lap1` of the real last pass annihilates the ones vector.** This reads the matrix
/// the iteration actually built rather than re-deriving one from the answer, which is the only
/// version of the claim that tests the code: a re-derivation would pass even if
/// `build_weighted` wrote the diagonal in the wrong place, since it would be testing the test's
/// own arithmetic.
#[test]
fn the_built_laplacian_annihilates_the_ones_vector() {
    let count = 6;
    let state = converged_state(&ring(count as u32), count);
    let mut product = vec![0.0f32; count];
    right_mult(state.weighted(), count, &vec![1.0; count], &mut product);
    for (node, sum) in product.iter().enumerate() {
        assert!(sum.abs() < 1e-4, "row {node} sums to {sum} over ones");
    }
}

/// **The negative control for the test above**: zero the diagonal and the same matrix stops
/// annihilating the ones vector, by exactly the row sums. Without this, a `build_weighted` that
/// never wrote the diagonal could pass the first test whenever the degrees happened to be
/// small, and the property would be untested.
#[test]
fn the_diagonal_is_what_makes_it_a_laplacian() {
    let count = 6;
    let state = converged_state(&ring(count as u32), count);
    let mut stripped = state.weighted().to_vec();
    let (mut step, mut slot) = (count, 0);
    for _ in 0..count {
        stripped[slot] = 0.0;
        slot += step;
        step -= 1;
    }
    let mut product = vec![0.0f32; count];
    right_mult(&stripped, count, &vec![1.0; count], &mut product);
    assert!(
        product.iter().any(|sum| sum.abs() > 1e-3),
        "zeroing the diagonal changed nothing: {product:?}"
    );
}

/// `lap2` is built once from the distances and never changes, and its diagonal carries the
/// negated row sums of `1 / d_ij^2` rather than of the raw distances — so it annihilates the
/// ones vector too, and reading it off the *distances* would not.
#[test]
fn the_fixed_weight_matrix_is_also_a_laplacian() {
    let count = 5;
    let state = converged_state(&ring(count as u32), count);
    let mut product = vec![0.0f32; count];
    right_mult(state.weights(), count, &vec![1.0; count], &mut product);
    for (node, sum) in product.iter().enumerate() {
        assert!(sum.abs() < 1e-3, "row {node} sums to {sum} over ones");
    }
}

/// A [`State`] run to convergence, for the tests that need its matrices rather than its
/// coordinates. It runs the same passes [`majorize`] does and stops, so it cannot diverge
/// from what the layout does — it only stops exposing the buffers.
fn converged_state(neighbours: &Csr, count: usize) -> State {
    let distances = Packed::of_hops(neighbours, count);
    let mut state = State::of(&distances, count);
    let (mut x, mut y) = initial_placement(count, 1);
    for _ in 0..BUDGET {
        if state.pass(count, 1e-4, &mut x, &mut y) {
            break;
        }
    }
    state
}

/// The degrees of a matrix that is not a Laplacian, used to show `row_sums` reads the
/// off-diagonals and the diagonal slot is skipped: a matrix whose diagonal is already
/// non-zero must have those entries ignored.
#[test]
fn row_sums_skip_the_diagonal_slot() {
    let count = 3;
    let mut packed = vec![0.0f32; count * (count + 1) / 2];
    // Row 0: diagonal 100.0, then d01 = 1.0, d02 = 1.0. Row 1: diagonal 200.0, d12 = 1.0.
    packed[0] = 100.0;
    packed[1] = 1.0;
    packed[2] = 1.0;
    packed[3] = 200.0;
    packed[4] = 1.0;
    let degrees = row_sums(&packed, count);
    assert!((degrees[0] + 2.0).abs() < 1e-6, "{degrees:?}");
    assert!((degrees[1] + 2.0).abs() < 1e-6, "{degrees:?}");
    assert!((degrees[2] + 2.0).abs() < 1e-6, "{degrees:?}");
}
