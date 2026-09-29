//! Tier D — the dense symmetric eigensolver (`docs/decisions/eigensolver.md`).
//!
//! Householder tridiagonalisation (`tred2`) then implicit-shift QL with eigenvector
//! accumulation (`tql2`), ported line-for-line from `Jama.EigenvalueDecomposition`'s
//! symmetric path (`/home/user/refs/jama-1.0.3/Jama/EigenvalueDecomposition.java`, sha256
//! `c9d8efe5cfd22b7dddfc3d7fb185bb01d1a6ed2130610c0fcfe7195ccee36a81`). Only the symmetric
//! branch is ported; JAMA's non-symmetric `orthes`/`hqr2` path has no caller here.
//!
//! Two changes from the reference, both decided in the ADR: `Maths.hypot` is
//! `libm::hypot`, and eigenvalues are ordered with a stable sort on `(value, index)`
//! rather than JAMA's undocumented-stability selection sort (D5).

use super::EigBlock;

mod tql2;
mod tred2;

/// Working storage for one symmetric eigendecomposition. `v` is row-major (`v[i*n+j]`
/// is JAMA's `V[i][j]`); [`eigh_with_diagnostics`] transposes it into the public,
/// column-major [`EigBlock`] at the end. Fields are private to `dense_sym` but visible
/// to its `tred2`/`tql2` submodules (a descendant module, not a different crate).
struct Sym {
    n: usize,
    d: Vec<f64>,
    e: Vec<f64>,
    v: Vec<f64>,
}

impl Sym {
    fn get(&self, i: usize, j: usize) -> f64 {
        self.v[i * self.n + j]
    }

    fn set(&mut self, i: usize, j: usize, value: f64) {
        self.v[i * self.n + j] = value;
    }
}

/// Full symmetric eigendecomposition of the `n × n` row-major matrix `a`, ascending
/// eigenvalues. Callers pass a matrix they already know is symmetric (a graph Laplacian
/// or a Gram matrix); this does not check.
pub fn eigh(a: &[f64], n: usize) -> EigBlock {
    eigh_with_diagnostics(a, n).0
}

/// [`eigh`] plus whether any eigenvalue hit the 30-iteration `tql2` cap (measurement and
/// the ADR's Ponytail marker; the residual check, not this flag, decides usability).
pub fn eigh_with_diagnostics(a: &[f64], n: usize) -> (EigBlock, bool) {
    if n == 0 {
        return (
            EigBlock {
                values: Vec::new(),
                vectors: Vec::new(),
                n: 0,
                k: 0,
            },
            false,
        );
    }
    let mut sym = Sym {
        n,
        d: vec![0.0; n],
        e: vec![0.0; n],
        v: a.to_vec(),
    };
    tred2::tred2(&mut sym);
    let hit_cap = tql2::tql2(&mut sym);
    flush_near_zero(&mut sym.d, &mut sym.v, frobenius_flush_threshold(a));
    (transpose_to_block(&sym), hit_cap)
}

/// `eps * ‖A‖_F`, the threshold below which a value is flushed to exactly `0.0` so the
/// result does not depend on denormal handling (the ADR's "flush entries" rule).
fn frobenius_flush_threshold(a: &[f64]) -> f64 {
    let sum_sq: f64 = a.iter().map(|x| x * x).sum();
    f64::EPSILON * sum_sq.sqrt()
}

fn flush_near_zero(values: &mut [f64], vectors: &mut [f64], threshold: f64) {
    for value in values.iter_mut() {
        if value.abs() < threshold {
            *value = 0.0;
        }
    }
    for value in vectors.iter_mut() {
        if value.abs() < threshold {
            *value = 0.0;
        }
    }
}

/// Row-major `Sym::v` into the public column-major [`EigBlock`] (`vectors[j*n+i]` is
/// component `i` of eigenvector `j`), full spectrum (`k = n`).
fn transpose_to_block(sym: &Sym) -> EigBlock {
    let n = sym.n;
    let mut vectors = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            vectors[j * n + i] = sym.get(i, j);
        }
    }
    EigBlock {
        values: sym.d.clone(),
        vectors,
        n,
        k: n,
    }
}

#[cfg(test)]
mod tests;
