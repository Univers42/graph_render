//! `generalized_rayleigh_ritz` checked against a hand-solved `2 x 2` generalized
//! eigenproblem, independent of the LOBPCG loop that motivated it: `gramA = [[3,0],[0,1]]`,
//! `gramB = [[1,0.5],[0.5,1]]` (not the identity, the exact situation `P`'s overlap with
//! `X`/`R` creates). `det(A - λB) = 0` gives `0.75λ² - 4λ + 3 = 0`, so
//! `λ = (8 ± 2√7) / 3`.

use super::*;

const SQRT3_OVER_2: f64 = 0.8660254037844386;
const SQRT3: f64 = 1.7320508075688772;
const TWO_OVER_SQRT3: f64 = 1.1547005383792515;

/// `n = 3` physical rows, `m = 2` basis columns chosen so `basisᵀbasis = gramB` and
/// `basisᵀabasis = gramA` exactly (worked by hand in this module's doc comment).
fn hand_solved_case() -> (Vec<f64>, Vec<f64>) {
    let basis = vec![1.0, 0.0, 0.0, 0.5, SQRT3_OVER_2, 0.0];
    let abasis = vec![3.0, -SQRT3, 0.0, 0.0, TWO_OVER_SQRT3, 0.0];
    (basis, abasis)
}

#[test]
fn matches_the_hand_solved_two_by_two_generalized_eigenvalues() {
    let (basis, abasis) = hand_solved_case();
    let out = generalized_rayleigh_ritz(&basis, &abasis, 3).expect("gramB is positive definite");
    let sqrt7 = 7.0_f64.sqrt();
    let want = [(8.0 - 2.0 * sqrt7) / 3.0, (8.0 + 2.0 * sqrt7) / 3.0];
    for (got, want) in out.values.iter().zip(want) {
        assert!((got - want).abs() < 1e-9, "{got} vs {want}");
    }
}

/// `Av = λBv`, checked directly on `v` in the caller's own basis-index coordinates (not
/// just that `λ` matches): with `gramA`, `gramB` reconstructed straight from the matrices
/// this case was designed around, `(gramA - λ gramB) v` must be (numerically) zero.
#[test]
fn the_returned_vectors_actually_solve_the_generalized_problem() {
    let (basis, abasis) = hand_solved_case();
    let out = generalized_rayleigh_ritz(&basis, &abasis, 3).expect("gramB is positive definite");
    let gram_a = [3.0, 0.0, 0.0, 1.0];
    let gram_b = [1.0, 0.5, 0.5, 1.0];
    for col in 0..2 {
        let v = out.column(col);
        let lambda = out.values[col];
        for row in 0..2 {
            let av: f64 = (0..2).map(|k| gram_a[row * 2 + k] * v[k]).sum();
            let bv: f64 = (0..2).map(|k| gram_b[row * 2 + k] * v[k]).sum();
            assert!(
                (av - lambda * bv).abs() < 1e-9,
                "row {row} col {col}: {av} vs {}",
                lambda * bv
            );
        }
    }
}

#[test]
fn a_non_positive_definite_gram_b_is_refused_not_miscomputed() {
    // Two identical basis columns: gramB is singular ([[1,1],[1,1]]), not positive
    // definite.
    let basis = vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let abasis = vec![2.0, 0.0, 0.0, 2.0, 0.0, 0.0];
    assert!(generalized_rayleigh_ritz(&basis, &abasis, 3).is_none());
}
