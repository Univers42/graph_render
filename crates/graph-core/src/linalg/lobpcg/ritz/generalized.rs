//! The generalized Rayleigh-Ritz problem `gramA v = λ gramB v`, needed whenever the
//! basis includes the previous search direction `P`: `P` is only orthonormalized within
//! itself (`ops::mgs_orthonormalize`), never against `X` or the new residual `R`
//! (`lobpcg.py:791-810` does the same — it Cholesky-B-orthonormalizes `P` alone and lets
//! the generalized eigenproblem absorb the remaining `X`/`R` overlap), so `gramB` is not
//! the identity and an ordinary eigensolve on `gramA` alone silently returns the wrong
//! eigenpairs. `X` alone and `[X, R]` (no `P`) have `gramB ≈ I` by construction, so
//! [`super::rayleigh_ritz`] (ordinary) stays correct and cheaper for those two calls.
//!
//! Reduced via a Cholesky factorisation `gramB = L Lᵀ` (`lobpcg.py`'s own `eigh(gramA,
//! gramB)` does the LAPACK equivalent internally): `C = L⁻¹ gramA L⁻ᵀ`, an ordinary
//! symmetric eigenproblem in `y`, then `v = L⁻ᵀ y` back in the basis's own coordinates.
//! All matrices here are the small `m x m` Rayleigh-Ritz size (`m <= 3 * block`), row-major.

use crate::linalg::EigBlock;
use crate::linalg::dense_sym::eigh;

use super::gram_matrix;

/// Ponytail: a Cholesky pivot at or below this is treated as numerically non-positive-
/// definite (`gramB`'s diagonal is exactly `1` by construction, so this is a relative
/// floor, not an exact algorithm). Under-reporting direction: a false positive here drops
/// `P` from the step entirely (`lobpcg.rs`'s fallback to the ordinary `[X, R]` solve),
/// never accepts a wrong factorisation. Escape hatch: the caller's residual/orthonormality
/// gate, same as [`super::super::COLLAPSE_NORM`].
const CHOL_MIN_PIVOT: f64 = 1e-10;

/// Lower-triangular `L` with `L Lᵀ = b` (`b` symmetric `m x m`, row-major), or `None` when
/// a pivot is at or below [`CHOL_MIN_PIVOT`] (mirrors scipy's Cholesky-failure restart).
fn cholesky_lower(b: &[f64], m: usize) -> Option<Vec<f64>> {
    let mut l = vec![0.0; m * m];
    for i in 0..m {
        for j in 0..=i {
            let mut sum = b[i * m + j];
            for k in 0..j {
                sum -= l[i * m + k] * l[j * m + k];
            }
            if i == j {
                if sum <= CHOL_MIN_PIVOT {
                    return None;
                }
                l[i * m + i] = sum.sqrt();
            } else {
                l[i * m + j] = sum / l[j * m + j];
            }
        }
    }
    Some(l)
}

/// Solves `L Z = mat` for `Z` (`mat` `m x m`, row-major), column by column.
fn forward_solve_columns(l: &[f64], m: usize, mat: &[f64]) -> Vec<f64> {
    let mut z = vec![0.0; m * m];
    for col in 0..m {
        for i in 0..m {
            let mut sum = mat[i * m + col];
            for k in 0..i {
                sum -= l[i * m + k] * z[k * m + col];
            }
            z[i * m + col] = sum / l[i * m + i];
        }
    }
    z
}

/// Solves `Lᵀ v = y` for one column `y` (`Lᵀ` is upper triangular: `(Lᵀ)[i][j] = l[j*m+i]`).
fn back_solve_column(l: &[f64], m: usize, y: &[f64]) -> Vec<f64> {
    let mut v = vec![0.0; m];
    for step in 0..m {
        let i = m - 1 - step;
        let mut sum = y[i];
        for j in (i + 1)..m {
            sum -= l[j * m + i] * v[j];
        }
        v[i] = sum / l[i * m + i];
    }
    v
}

fn transpose_square(a: &[f64], m: usize) -> Vec<f64> {
    let mut t = vec![0.0; m * m];
    for i in 0..m {
        for j in 0..m {
            t[j * m + i] = a[i * m + j];
        }
    }
    t
}

fn symmetrize(a: &mut [f64], m: usize) {
    for i in 0..m {
        for j in (i + 1)..m {
            let avg = (a[i * m + j] + a[j * m + i]) / 2.0;
            a[i * m + j] = avg;
            a[j * m + i] = avg;
        }
    }
}

/// `gramA v = λ gramB v` for the given (possibly non-orthonormal) basis. `None` when
/// `gramB` fails its Cholesky factorisation — the basis's overlap is too degenerate to
/// trust, and the caller falls back to dropping the offending block (`P`).
pub(super) fn generalized_rayleigh_ritz(
    basis: &[f64],
    abasis: &[f64],
    n: usize,
) -> Option<EigBlock> {
    let m = basis.len() / n;
    let gram_a = gram_matrix(basis, abasis, n);
    let gram_b = gram_matrix(basis, basis, n);
    let l = cholesky_lower(&gram_b, m)?;

    let z = forward_solve_columns(&l, m, &gram_a);
    let zt = transpose_square(&z, m);
    let mut c = forward_solve_columns(&l, m, &zt);
    symmetrize(&mut c, m);

    let reduced = eigh(&c, m);
    let mut vectors = vec![0.0; m * m];
    for col in 0..m {
        let v = back_solve_column(&l, m, reduced.column(col));
        vectors[col * m..(col + 1) * m].copy_from_slice(&v);
    }
    Some(EigBlock {
        values: reduced.values,
        vectors,
        n: m,
        k: m,
    })
}

#[cfg(test)]
mod tests;
