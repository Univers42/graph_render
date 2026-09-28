//! LOBPCG exercised directly against closed-form Laplacians large enough (`n > 256`) to
//! force the iterative tier — the per-component dispatch that actually *chooses* this
//! tier is `layout::spectral`'s concern, not this file's.

use super::*;
use crate::linalg::{orthonormal, residual_converged};

fn path_diag(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| if i == 0 || i == n - 1 { 1.0 } else { 2.0 })
        .collect()
}

fn path_matvec(x: &[f64], y: &mut [f64]) {
    let n = x.len();
    for i in 0..n {
        let deg = if i == 0 || i == n - 1 { 1.0 } else { 2.0 };
        let left = if i > 0 { x[i - 1] } else { 0.0 };
        let right = if i + 1 < n { x[i + 1] } else { 0.0 };
        y[i] = deg * x[i] - left - right;
    }
}

fn cycle_diag(n: usize) -> Vec<f64> {
    vec![2.0; n]
}

fn cycle_matvec(x: &[f64], y: &mut [f64]) {
    let n = x.len();
    for i in 0..n {
        y[i] = 2.0 * x[i] - x[(i + n - 1) % n] - x[(i + 1) % n];
    }
}

fn path_full_spectrum(n: usize) -> Vec<f64> {
    (0..n)
        .map(|k| 2.0 - 2.0 * libm::cos(core::f64::consts::PI * k as f64 / n as f64))
        .collect()
}

fn cycle_full_spectrum(n: usize) -> Vec<f64> {
    let mut v: Vec<f64> = (0..n)
        .map(|k| 2.0 - 2.0 * libm::cos(2.0 * core::f64::consts::PI * k as f64 / n as f64))
        .collect();
    v.sort_by(f64::total_cmp);
    v
}

#[test]
fn finds_the_smallest_nontrivial_path_eigenvalues_above_the_dense_limit() {
    let n = 300;
    let out = lobpcg_smallest(path_matvec, &path_diag(n), n, 4);
    let want = path_full_spectrum(n);
    for (got, want) in out.eig.values[..2].iter().zip(&want[1..3]) {
        assert!((got - want).abs() < 1e-3, "{got} vs {want}");
    }
    assert!(orthonormal(&out.eig, 1e-4), "{:?}", out.eig.values);
    assert!(residual_converged(path_matvec, &out.eig, 1e-2));
}

#[test]
fn finds_the_smallest_nontrivial_cycle_eigenvalues_above_the_dense_limit() {
    // The cycle's nontrivial eigenvalues repeat (lambda_1 == lambda_{n-1}), the exact
    // degenerate-spectrum case a block method exists for (R1).
    let n = 300;
    let out = lobpcg_smallest(cycle_matvec, &cycle_diag(n), n, 4);
    let want = cycle_full_spectrum(n);
    for (got, want) in out.eig.values[..2].iter().zip(&want[1..3]) {
        assert!((got - want).abs() < 1e-2, "{got} vs {want}");
    }
    assert!(orthonormal(&out.eig, 1e-4), "{:?}", out.eig.values);
    assert!(residual_converged(cycle_matvec, &out.eig, 1e-2));
}

#[test]
fn is_deterministic_run_twice() {
    let n = 400;
    let once = lobpcg_smallest(path_matvec, &path_diag(n), n, 4);
    let twice = lobpcg_smallest(path_matvec, &path_diag(n), n, 4);
    assert_eq!(once, twice, "same input bits, same output bits");
}

/// Prints the iteration counts and timings `docs/measurements/phase06-eigen.md`
/// reports, captured with `--nocapture` at report time (test-only
/// `std::time::Instant`; the motor itself never reads the clock, D8). Sizes stop at
/// `1000`: a path's eigenvalue gap shrinks as `O(1/n^2)`, and measurement showed a
/// `4096`-node path genuinely exceeds [`MAXITER`] against this port's constant-diagonal
/// preconditioner — an honest scaling limit of the simplified port (`docs/decisions/
/// eigensolver.md`), not a threshold to assert past.
#[test]
fn measurement_lobpcg_iterations_and_timing() {
    for n in [300usize, 601, 1000] {
        let start = std::time::Instant::now();
        let out = lobpcg_smallest(path_matvec, &path_diag(n), n, 4);
        let elapsed = start.elapsed();
        assert!(
            residual_converged(path_matvec, &out.eig, 1e-2),
            "n={n} residual gate failed"
        );
        assert!(orthonormal(&out.eig, 1e-4), "n={n}: {:?}", out.eig.values);
        eprintln!(
            "lobpcg path n={n} iterations={} converged={} elapsed_ms={:.3}",
            out.iterations,
            out.converged,
            elapsed.as_secs_f64() * 1000.0
        );
    }
}
