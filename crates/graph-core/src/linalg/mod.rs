//! Dense and iterative symmetric eigensolvers shared by the spectral and Pivot MDS
//! layouts (`docs/decisions/eigensolver.md`). Domain-agnostic: everything here works on
//! a plain matrix or an operator closure, never on a [`crate::index::Topology`] — the
//! two layout modules build the graph Laplacian and hand it in.
//!
//! `f64` throughout (D-EIG's "single rounding to f32 at the wire" applies at the layout
//! boundary, not here).

pub mod dense_sym;
pub mod lobpcg;

/// A block of `k` eigenpairs of an `n × n` symmetric matrix. Column-major:
/// `vectors[j * n + i]` is component `i` of eigenvector `j`.
#[derive(Debug, Clone, PartialEq)]
pub struct EigBlock {
    /// The `k` eigenvalues, in the order their columns appear in `vectors`.
    pub values: Vec<f64>,
    /// `n * k` components, column-major.
    pub vectors: Vec<f64>,
    /// Matrix dimension.
    pub n: usize,
    /// Number of eigenpairs (columns).
    pub k: usize,
}

impl EigBlock {
    /// Eigenvector `j`'s `n` components.
    pub fn column(&self, j: usize) -> &[f64] {
        &self.vectors[j * self.n..(j + 1) * self.n]
    }

    /// Eigenvector `j`'s `n` components, mutably.
    pub fn column_mut(&mut self, j: usize) -> &mut [f64] {
        let n = self.n;
        &mut self.vectors[j * n..(j + 1) * n]
    }
}

/// `_eig_converged` (`networkx_layouts.py:74-80`), ported unchanged: every value finite,
/// and the largest column residual `‖matvec(vⱼ) − λⱼvⱼ‖` at most
/// `tol * max(max|λ|, 1e-12)`. `matvec` applies the operator to one column at a time, in
/// ascending column order (a fixed order, D3).
pub fn residual_converged(
    mut matvec: impl FnMut(&[f64], &mut [f64]),
    eig: &EigBlock,
    tol: f64,
) -> bool {
    if eig.values.iter().any(|v| !v.is_finite()) || eig.vectors.iter().any(|v| !v.is_finite()) {
        return false;
    }
    let mut av = vec![0.0; eig.n];
    let mut worst = 0.0_f64;
    for j in 0..eig.k {
        matvec(eig.column(j), &mut av);
        let residual: f64 = av
            .iter()
            .zip(eig.column(j))
            .map(|(a, v)| {
                let diff = a - eig.values[j] * v;
                diff * diff
            })
            .sum::<f64>()
            .sqrt();
        worst = worst.max(residual);
    }
    let scale = eig
        .values
        .iter()
        .fold(0.0_f64, |m, v| m.max(v.abs()))
        .max(1e-12);
    worst <= tol * scale
}

/// `‖VᵀV − I‖_max ≤ tol` (D-EIG / C5), sequential over `(j, l)` pairs in ascending order.
pub fn orthonormal(eig: &EigBlock, tol: f64) -> bool {
    for j in 0..eig.k {
        for l in j..eig.k {
            let dot: f64 = (0..eig.n)
                .map(|i| eig.column(j)[i] * eig.column(l)[i])
                .sum();
            let want = if j == l { 1.0 } else { 0.0 };
            if (dot - want).abs() > tol {
                return false;
            }
        }
    }
    true
}

/// `_fix_eigenvector_signs` (`networkx_layouts.py:66-72`): for each column, scan `i`
/// ascending with a strict `>` on `|vᵢ|` so the lowest index wins a tie; flip the column
/// if that entry is negative.
///
/// Ponytail: on a symmetric input (a path, a cycle, a regular grid) two entries can tie
/// or near-tie on magnitude; the ascending-index rule is reproducible under
/// bit-identity, but which entry wins — and so which sign is chosen — is an artifact of
/// the solver's internal path, not of the eigenvector itself (both signs solve the same
/// eigenproblem). Direction: cosmetic, never wrong. Escape hatch: none needed for
/// correctness; a probe-vector canonicalisation (rejected by default, D-EIG) would
/// remove the residual non-uniqueness if ever required.
pub fn pin_signs(eig: &mut EigBlock) {
    for j in 0..eig.k {
        let column = eig.column(j);
        let mut best = 0usize;
        for i in 1..column.len() {
            if column[i].abs() > column[best].abs() {
                best = i;
            }
        }
        if column[best] < 0.0 {
            for value in eig.column_mut(j) {
                *value = -*value;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_block(n: usize) -> EigBlock {
        let mut vectors = vec![0.0; n * n];
        for i in 0..n {
            vectors[i * n + i] = 1.0;
        }
        EigBlock {
            values: vec![1.0; n],
            vectors,
            n,
            k: n,
        }
    }

    #[test]
    fn identity_is_orthonormal_and_residual_converged() {
        let eig = identity_block(3);
        assert!(orthonormal(&eig, 1e-9));
        let matvec = |x: &[f64], y: &mut [f64]| y.copy_from_slice(x);
        assert!(residual_converged(matvec, &eig, 1e-9));
    }

    #[test]
    fn a_non_finite_value_never_converges() {
        let mut eig = identity_block(2);
        eig.values[0] = f64::NAN;
        let matvec = |x: &[f64], y: &mut [f64]| y.copy_from_slice(x);
        assert!(!residual_converged(matvec, &eig, 1e9));
    }

    #[test]
    fn sign_pinning_flips_so_the_largest_magnitude_entry_is_positive() {
        let mut eig = EigBlock {
            values: vec![1.0],
            vectors: vec![0.1, -0.9, 0.2],
            n: 3,
            k: 1,
        };
        pin_signs(&mut eig);
        assert_eq!(eig.column(0), [-0.1, 0.9, -0.2]);
    }

    #[test]
    fn a_tie_on_magnitude_is_won_by_the_lowest_index() {
        let mut eig = EigBlock {
            values: vec![1.0],
            vectors: vec![-0.5, 0.5],
            n: 2,
            k: 1,
        };
        pin_signs(&mut eig);
        // index 0 wins the tie; it is negative, so the column flips.
        assert_eq!(eig.column(0), [0.5, -0.5]);
    }
}
