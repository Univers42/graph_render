//! Closed-form fixtures for the dense solver itself (`dense_sym::eigh`), independent of
//! any graph plumbing: hand-checked small matrices, then Laplacian spectra with known
//! closed forms (path, cycle, star, `K_n`, grid), all `n <= 256` — the dense tier's own
//! domain. The full per-component dispatch (which tier runs, packing, sign pinning at
//! the layout level) is `layout::spectral`'s test suite, not this one.

use super::*;
use crate::linalg::{orthonormal, residual_converged};

fn laplacian_from_edges(n: usize, edges: &[(usize, usize)]) -> Vec<f64> {
    let mut a = vec![0.0; n * n];
    for &(i, j) in edges {
        a[i * n + j] -= 1.0;
        a[j * n + i] -= 1.0;
        a[i * n + i] += 1.0;
        a[j * n + j] += 1.0;
    }
    a
}

pub(crate) fn path_edges(n: usize) -> Vec<(usize, usize)> {
    (0..n.saturating_sub(1)).map(|i| (i, i + 1)).collect()
}

pub(crate) fn cycle_edges(n: usize) -> Vec<(usize, usize)> {
    let mut e = path_edges(n);
    if n >= 2 {
        e.push((n - 1, 0));
    }
    e
}

pub(crate) fn star_edges(n: usize) -> Vec<(usize, usize)> {
    (1..n).map(|i| (0, i)).collect()
}

pub(crate) fn complete_edges(n: usize) -> Vec<(usize, usize)> {
    let mut e = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            e.push((i, j));
        }
    }
    e
}

pub(crate) fn grid_edges(rows: usize, cols: usize) -> Vec<(usize, usize)> {
    let idx = |r: usize, c: usize| r * cols + c;
    let mut e = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            if c + 1 < cols {
                e.push((idx(r, c), idx(r, c + 1)));
            }
            if r + 1 < rows {
                e.push((idx(r, c), idx(r + 1, c)));
            }
        }
    }
    e
}

/// `2 - 2cos(kπ/n)`, `k = 0..n-1`: the path Laplacian's closed-form spectrum.
fn path_spectrum(n: usize) -> Vec<f64> {
    (0..n)
        .map(|k| 2.0 - 2.0 * libm::cos(core::f64::consts::PI * k as f64 / n as f64))
        .collect()
}

/// `2 - 2cos(2πk/n)`, `k = 0..n-1`, sorted ascending (the closed form's own `k`-order
/// rises then falls, unlike `eigh`'s output): the cycle Laplacian's closed-form spectrum.
fn cycle_spectrum(n: usize) -> Vec<f64> {
    let mut v: Vec<f64> = (0..n)
        .map(|k| 2.0 - 2.0 * libm::cos(2.0 * core::f64::consts::PI * k as f64 / n as f64))
        .collect();
    v.sort_by(f64::total_cmp);
    v
}

/// `0, 1 (×(n-2)), n`: the star Laplacian's closed-form spectrum.
fn star_spectrum(n: usize) -> Vec<f64> {
    let mut v = vec![1.0; n];
    v[0] = 0.0;
    v[n - 1] = n as f64;
    v
}

/// `0, n (×(n-1))`: the complete graph's closed-form spectrum.
fn complete_spectrum(n: usize) -> Vec<f64> {
    let mut v = vec![n as f64; n];
    v[0] = 0.0;
    v
}

/// The Cartesian-product (grid) Laplacian spectrum: every sum of a row-path and a
/// column-path eigenvalue, sorted ascending.
fn grid_spectrum(rows: usize, cols: usize) -> Vec<f64> {
    let (r, c) = (path_spectrum(rows), path_spectrum(cols));
    let mut v: Vec<f64> = r
        .iter()
        .flat_map(|&a| c.iter().map(move |&b| a + b))
        .collect();
    v.sort_by(f64::total_cmp);
    v
}

fn assert_spectrum_matches(a: &[f64], n: usize, want: &[f64], tol: f64) {
    let eig = eigh(a, n);
    assert_eq!(eig.values.len(), n);
    for (got, want) in eig.values.iter().zip(want) {
        assert!((got - want).abs() <= tol, "got {got} want {want}");
    }
    assert!(orthonormal(&eig, 1e-8), "{:?}", eig.values);
    let matvec = |x: &[f64], y: &mut [f64]| {
        for i in 0..n {
            y[i] = (0..n).map(|j| a[i * n + j] * x[j]).sum();
        }
    };
    assert!(residual_converged(matvec, &eig, 1e-2));
}

#[test]
fn identity_has_the_repeated_eigenvalue_one() {
    let n = 4;
    let mut a = vec![0.0; n * n];
    for i in 0..n {
        a[i * n + i] = 1.0;
    }
    assert_spectrum_matches(&a, n, &[1.0; 4], 1e-10);
}

#[test]
fn a_hand_worked_two_by_two_matches_exactly() {
    // [[2,1],[1,2]] -> eigenvalues 1 and 3, eigenvectors (1,-1)/sqrt2, (1,1)/sqrt2.
    let a = [2.0, 1.0, 1.0, 2.0];
    let eig = eigh(&a, 2);
    assert!((eig.values[0] - 1.0).abs() < 1e-12);
    assert!((eig.values[1] - 3.0).abs() < 1e-12);
    let inv_sqrt2 = core::f64::consts::FRAC_1_SQRT_2;
    let mut expected0 = [inv_sqrt2, -inv_sqrt2];
    if eig.column(0)[0] < 0.0 {
        expected0 = [-inv_sqrt2, inv_sqrt2];
    }
    assert!((eig.column(0)[0] - expected0[0]).abs() < 1e-10);
    assert!((eig.column(0)[1] - expected0[1]).abs() < 1e-10);
}

#[test]
fn path_cycle_star_complete_and_grid_spectra_match_closed_form() {
    for n in [2usize, 3, 8, 40, 100, 256] {
        let a = laplacian_from_edges(n, &path_edges(n));
        assert_spectrum_matches(&a, n, &path_spectrum(n), 1e-6);
        let a = laplacian_from_edges(n, &cycle_edges(n));
        assert_spectrum_matches(&a, n, &cycle_spectrum(n), 1e-6);
        let a = laplacian_from_edges(n, &star_edges(n));
        assert_spectrum_matches(&a, n, &star_spectrum(n), 1e-6);
        if n <= 40 {
            let a = laplacian_from_edges(n, &complete_edges(n));
            assert_spectrum_matches(&a, n, &complete_spectrum(n), 1e-6);
        }
    }
    for (rows, cols) in [(4usize, 4usize), (16, 16)] {
        let a = laplacian_from_edges(rows * cols, &grid_edges(rows, cols));
        assert_spectrum_matches(&a, rows * cols, &grid_spectrum(rows, cols), 1e-6);
    }
}

#[test]
fn the_solver_is_deterministic_run_twice() {
    let n = 64;
    let a = laplacian_from_edges(n, &cycle_edges(n));
    let once = eigh(&a, n);
    let twice = eigh(&a, n);
    assert_eq!(once, twice, "same input bits, same output bits");
}

#[test]
fn a_30_iteration_cap_hit_is_reported_not_hidden() {
    // The cap is generous for every fixture above; this only checks the flag plumbs
    // through without panicking on a well-behaved matrix (never hit here).
    let n = 32;
    let a = laplacian_from_edges(n, &path_edges(n));
    let (_, hit_cap) = eigh_with_diagnostics(&a, n);
    assert!(!hit_cap, "a well-conditioned Laplacian never needs the cap");
}

mod jacobi;
