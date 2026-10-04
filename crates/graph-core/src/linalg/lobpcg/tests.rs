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

/// **The bug the 3-D spectral arm found, asserted on its signature rather than through a
/// solve.** `start_block` drew its Weyl irrationals from a fixed `[f64; 3]`, so at
/// `block = 5` — the reference's own `k = min(dims + 2, n_c - 1)` at `dims = 3` — the last
/// column was left ALL ZERO. A start vector LOBPCG cannot move off returns the Laplacian's
/// trivial eigenvector, the caller's residual gate then refuses it, and `layout.spectral3d`
/// refused every gate seed above `DENSE_EIG_LIMIT` while `layout.spectral` passed them all.
///
/// The width is swept rather than pinned at 5 because the failure is a silent truncation,
/// and a truncation has no natural maximum: asserting one width would pass again the day
/// something else asked for a wider block.
#[test]
fn every_start_column_is_filled_at_every_block_width() {
    for block in 1..=8usize {
        let start = ops::start_block(64, block);
        assert_eq!(start.len(), 64 * block, "block {block}");
        for col in 0..block {
            let column = &start[col * 64..(col + 1) * 64];
            assert!(
                column.iter().any(|&v| v != 0.0),
                "block {block} column {col} is all zero"
            );
            assert!(
                column.iter().all(|v| v.is_finite()),
                "block {block} column {col} has a non-finite entry"
            );
        }
    }
}

/// **The control for the row above, and what makes the pair a pair.** The fix generates
/// alphas past column 3, and the 2-D block is 4 wide, so it reaches exactly one past the
/// old table's end — a change that is free to be wrong and would be invisible if nothing
/// held the old bytes. These three literals are the historical Weyl irrationals, so every
/// 2-D snapshot in the hash gate stays where it was.
#[test]
fn the_start_block_of_every_2d_run_is_unchanged() {
    assert_eq!(
        ops::alpha_for(1),
        0.6180339887498949,
        "column 1's historical alpha"
    );
    assert_eq!(
        ops::alpha_for(2),
        0.4142135623730951,
        "column 2's historical alpha"
    );
    assert_eq!(
        ops::alpha_for(3),
        0.7320508075688772,
        "column 3's historical alpha"
    );

    // Column 0 is the reference's own start vector, unchanged.
    let n = 32;
    let start = ops::start_block(n, 4);
    for (i, &got) in start[..n].iter().enumerate() {
        let want = libm::cos(i as f64 * 0.9124345) + libm::sin(i as f64 * 0.3141593);
        assert_eq!(got, want, "column 0, row {i}");
    }
    // And the three generated columns are the historical ones, in the historical order.
    for col in 1..4usize {
        let alpha = ops::alpha_for(col);
        for (i, &got) in start[col * n..(col + 1) * n].iter().enumerate() {
            let phase = (i as f64 + 1.0) * alpha;
            assert_eq!(
                got,
                2.0 * (phase - phase.floor()) - 1.0,
                "column {col}, row {i}"
            );
        }
    }
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
