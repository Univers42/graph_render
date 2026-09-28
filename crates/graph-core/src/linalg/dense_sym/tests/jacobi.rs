//! Cyclic Jacobi eigensolver, **test-only**: an independent cross-check for
//! `tred2`/`tql2`, never shipped (`docs/decisions/eigensolver.md`). Row-cyclic sweeps,
//! `p < q` ascending, the Rutishauser rotation
//! `t = sgn(θ) / (|θ| + sqrt(θ² + 1))`, using only `+ − * / sqrt`. Stops at
//! `off(A)² <= (eps·‖A‖_F)²` or 50 sweeps.

use super::super::EigBlock;

fn sign(x: f64) -> f64 {
    if x >= 0.0 { 1.0 } else { -1.0 }
}

/// One Jacobi rotation zeroing `a[p][q]`, applied to `a` and accumulated into `v`.
fn rotate(a: &mut [f64], v: &mut [f64], n: usize, p: usize, q: usize) {
    let apq = a[p * n + q];
    if apq == 0.0 {
        return;
    }
    let theta = (a[q * n + q] - a[p * n + p]) / (2.0 * apq);
    let t = sign(theta) / (theta.abs() + (theta * theta + 1.0).sqrt());
    let c = 1.0 / (t * t + 1.0).sqrt();
    let s = t * c;
    let (app, aqq) = (a[p * n + p], a[q * n + q]);
    a[p * n + p] = app - t * apq;
    a[q * n + q] = aqq + t * apq;
    a[p * n + q] = 0.0;
    a[q * n + p] = 0.0;
    for k in 0..n {
        if k != p && k != q {
            let (akp, akq) = (a[k * n + p], a[k * n + q]);
            a[k * n + p] = c * akp - s * akq;
            a[p * n + k] = a[k * n + p];
            a[k * n + q] = s * akp + c * akq;
            a[q * n + k] = a[k * n + q];
        }
    }
    for k in 0..n {
        let (vkp, vkq) = (v[k * n + p], v[k * n + q]);
        v[k * n + p] = c * vkp - s * vkq;
        v[k * n + q] = s * vkp + c * vkq;
    }
}

/// The off-diagonal sum of squares (each pair counted twice, symmetric matrix).
fn off_squared(a: &[f64], n: usize) -> f64 {
    let mut sum = 0.0;
    for p in 0..n {
        for q in (p + 1)..n {
            sum += 2.0 * a[p * n + q] * a[p * n + q];
        }
    }
    sum
}

/// Cyclic Jacobi, ascending eigenvalues, sorted the same way `tql2_sort` is.
pub(crate) fn cyclic_jacobi(input: &[f64], n: usize) -> EigBlock {
    let mut a = input.to_vec();
    let mut v = vec![0.0; n * n];
    for i in 0..n {
        v[i * n + i] = 1.0;
    }
    let frob_sq: f64 = input.iter().map(|x| x * x).sum();
    let threshold = f64::EPSILON * f64::EPSILON * frob_sq;
    for _sweep in 0..50 {
        if off_squared(&a, n) <= threshold {
            break;
        }
        for p in 0..n {
            for q in (p + 1)..n {
                rotate(&mut a, &mut v, n, p, q);
            }
        }
    }
    sorted_block(&a, &v, n)
}

fn sorted_block(a: &[f64], v: &[f64], n: usize) -> EigBlock {
    let diag: Vec<f64> = (0..n).map(|i| a[i * n + i]).collect();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&i, &j| diag[i].total_cmp(&diag[j]).then(i.cmp(&j)));
    let mut values = vec![0.0; n];
    let mut vectors = vec![0.0; n * n];
    for (new_col, &old_col) in order.iter().enumerate() {
        values[new_col] = diag[old_col];
        for row in 0..n {
            vectors[new_col * n + row] = v[row * n + old_col];
        }
    }
    EigBlock {
        values,
        vectors,
        n,
        k: n,
    }
}

#[test]
fn cross_checks_tred2_tql2_on_a_path_including_eigenvectors() {
    let n = 16;
    let a = super::laplacian_from_edges(n, &super::path_edges(n));
    let reference = super::eigh(&a, n);
    let jacobi = cyclic_jacobi(&a, n);
    for i in 0..n {
        let rel =
            (reference.values[i] - jacobi.values[i]).abs() / reference.values[i].abs().max(1.0);
        assert!(
            rel <= 1e-10,
            "eigenvalue {i}: {} vs {}",
            reference.values[i],
            jacobi.values[i]
        );
    }
    // The path's spectrum is simple (no repeated eigenvalues): every eigenvector is
    // unique up to sign, so a component-wise comparison after sign-aligning is valid.
    for j in 0..n {
        let (rc, jc) = (reference.column(j), jacobi.column(j));
        let flip = if rc[0] * jc[0] < 0.0 { -1.0 } else { 1.0 };
        for i in 0..n {
            assert!(
                (rc[i] - flip * jc[i]).abs() <= 1e-6,
                "column {j} component {i}"
            );
        }
    }
}

#[test]
fn cross_checks_tred2_tql2_eigenvalues_on_a_degenerate_cycle_and_grid() {
    // Cycle and grid Laplacians have repeated eigenvalues, so only the eigenvalue
    // multiset (not individual eigenvectors, which are non-unique) is compared.
    for a_n in [
        (super::laplacian_from_edges(12, &super::cycle_edges(12)), 12),
        (
            super::laplacian_from_edges(16, &super::grid_edges(4, 4)),
            16,
        ),
    ] {
        let (a, n) = a_n;
        let reference = super::eigh(&a, n);
        let jacobi = cyclic_jacobi(&a, n);
        for i in 0..n {
            let rel =
                (reference.values[i] - jacobi.values[i]).abs() / reference.values[i].abs().max(1.0);
            assert!(
                rel <= 1e-10,
                "eigenvalue {i}: {} vs {}",
                reference.values[i],
                jacobi.values[i]
            );
        }
    }
}
