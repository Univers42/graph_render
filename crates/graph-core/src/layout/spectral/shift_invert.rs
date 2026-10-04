//! The reference's shift-invert tier (`networkx_layouts.py:120-129`), the second half of
//! `_laplacian_nontrivial_eigenvectors`: when LOBPCG's block does not pass
//! `_eig_converged` (`:114`), the reference prints and re-solves with
//! `eigsh(L, k=dims + 1, sigma=-1e-3, which='LM', v0=start, maxiter=300)` — a Lanczos on
//! `(L + 1e-3 I)^-1`, whose negative shift lifts the *smallest* Laplacian eigenvalues to the
//! top of the inverse spectrum and so separates exactly the cluster plain LOBPCG chokes on.
//!
//! **No new dependency, and no sparse factorisation.** `docs/decisions/eigensolver.md`
//! ("Rejected alternatives") turned this tier down for want of "a sparse direct
//! factorisation; none is on the `libm`/`indexmap` allow-list and none is on disk (R2)",
//! leaving the cascade "dense -> LOBPCG -> skip-and-report, never shift-invert". What has
//! gone is that *reason*, not the decision's caution: ARPACK wants a sparse factorisation of
//! `L + 1e-3 I`, and the **dense** Cholesky below is the same factorisation of the same
//! matrix — the one the `DENSE_EIG_LIMIT` tier already builds. The decision record is outside
//! this job's paths, so it is reported rather than edited
//! (`docs/measurements/fix-spectral.md`, LF-09 row). `crate::linalg::lobpcg::ritz:`
//! `generalized` factors the same way for the same reason and is not reachable from here, so
//! the factorisation is written out again rather than hoisted into `crate::linalg`.
//!
//! `maxiter`: the reference's `300` is ARPACK's *restart* cap, not an iteration count, and
//! `lobpcg::MAXITER`'s measured `1500` (`docs/decisions/eigensolver.md`, "Caps and
//! tolerances") is the port's own answer to the same question — a 257-node path needs 410.
//! The retry therefore inherits tier I's cap rather than claiming to match a number that
//! counts something else. That is under-reporting in the safe direction and cannot reach the
//! output: `crate::linalg::residual_converged` decides, not the count. `tol` is tier I's
//! `1e-6`, already the reference's own (`:111`).

use super::graph::ComponentGraph;
use crate::linalg::dense_sym::eigh;
use crate::linalg::lobpcg::lobpcg_smallest;
use crate::linalg::EigBlock;

/// `eigsh(L, sigma=-1e-3)` (`networkx_layouts.py:121`). Negative, so `M = L - sigma I` is
/// `L + 1e-3 I` — positive definite even on the Laplacian's one-dimensional null space,
/// which is what makes a Cholesky of it exist at all.
pub(super) const SIGMA: f64 = -1e-3;

/// The largest component this tier will factorise densely.
///
/// Ponytail: this is a *budget*, not a bound — past it the retry is skipped and the
/// component is reported unsolved, so the tier under-reports (a component that shift-invert
/// would have solved is reported as a failure) rather than over-reports. Failing input: a
/// component of more than this many nodes whose spectrum LOBPCG cannot separate, which is
/// a path- or cycle-like component above 1024 — the 800-node path the ceiling already names
/// stays unsolved either way. Direction: under-reporting only; a wrong answer is impossible
/// because the residual gate runs on the result regardless of which tier produced it.
/// Escape hatch: raise this bound and pay `O(n^3/6)` for the factorisation plus `O(n^2)`
/// twice over; at 1024 that is 8.4 MB of `f64` per buffer and 1.8e8 flops, at 2048 four
/// times the memory and eight times the flops, and the wasm32 memory ceiling is 4 GiB.
pub(super) const DENSE_INVERT_LIMIT: usize = 1024;

/// A pivot at or below this is not positive definite, so `M` is refused rather than
/// factorised on a rounding artefact. The same floor `lobpcg::ritz::generalized` uses.
const CHOL_MIN_PIVOT: f64 = 1e-10;

/// The `dims` smallest non-trivial eigenpairs of `graph`, or `None` when `M` is not
/// positive definite.
///
/// `block` is the reference's `k = dims + 1` (`:121`): one column more than it keeps, so
/// the Rayleigh-Ritz below has a direction to choose from and `dims` is what it returns.
pub(super) fn eigenpairs(graph: &ComponentGraph, block: usize, dims: usize) -> Option<EigBlock> {
    let n = graph.size();
    let mut factor = shifted(graph);
    cholesky(&mut factor, n)?;
    // `-M^-1`, whose *smallest* eigenvalues are the largest of `M^-1`, i.e. the smallest
    // `lambda + 1e-3`. `lobpcg_smallest` only ever finds smallest, hence the negation.
    let inverse = |x: &[f64], y: &mut [f64]| {
        y.copy_from_slice(x);
        solve_in_place(&factor, n, y);
        for value in y.iter_mut() {
            *value = -*value;
        }
    };
    let identity = vec![1.0; n];
    let outcome = lobpcg_smallest(inverse, &identity, n, block);
    Some(ritz(graph, &outcome.eig, dims))
}

/// `M = L - SIGMA I` in place, row-major `n x n`, only the lower triangle written.
fn shifted(graph: &ComponentGraph) -> Vec<f64> {
    let mut a = graph.dense_matrix();
    let n = graph.size();
    for i in 0..n {
        a[i * n + i] -= SIGMA;
    }
    a
}

/// In-place lower Cholesky, `M = L Lᵀ`: column `j` of `L` over rows `j..n`. Ascending
/// index order throughout (D3). `None` on a pivot at or below [`CHOL_MIN_PIVOT`].
fn cholesky(a: &mut [f64], n: usize) -> Option<()> {
    for j in 0..n {
        let mut diag = a[j * n + j];
        for k in 0..j {
            diag -= a[j * n + k] * a[j * n + k];
        }
        if !(diag > CHOL_MIN_PIVOT) {
            return None;
        }
        a[j * n + j] = libm::sqrt(diag);
        for i in (j + 1)..n {
            let mut acc = a[i * n + j];
            for k in 0..j {
                acc -= a[i * n + k] * a[j * n + k];
            }
            a[i * n + j] = acc / a[j * n + j];
        }
    }
    Some(())
}

/// Solves `M y = x` in place through the lower factor: forward substitution `L z = x`, then
/// back substitution `Lᵀ y = z`. Ascending then descending index order (D3).
fn solve_in_place(a: &[f64], n: usize, y: &mut [f64]) {
    for i in 0..n {
        let mut acc = y[i];
        for k in 0..i {
            acc -= a[i * n + k] * y[k];
        }
        y[i] = acc / a[i * n + i];
    }
    for i in (0..n).rev() {
        let mut acc = y[i];
        for k in (i + 1)..n {
            acc -= a[k * n + i] * y[k];
        }
        y[i] = acc / a[i * n + i];
    }
}

/// Rayleigh-Ritz on `span(X)` against `L` itself, keeping its `dims` smallest values.
///
/// The retry's block is an approximate eigenspace of `-M^-1`, not of `L`: its own
/// Rayleigh quotients are of the wrong operator, and its `mu` would have to be read back
/// through `lambda = sigma - 1/mu`, which is ill-conditioned exactly where this tier is
/// needed. Projecting onto `L` inside the subspace the retry found costs one `dims x dims`
/// eigensolve and makes the eigenvalues and the residual the gate reads both properties of
/// `L`.
fn ritz(graph: &ComponentGraph, x: &EigBlock, dims: usize) -> EigBlock {
    let n = x.n;
    let b = x.k;
    let mut lx = vec![0.0; n * b];
    for j in 0..b {
        graph.matvec(x.column(j), &mut lx[j * n..(j + 1) * n]);
    }
    let full = eigh(&gram(x, &lx, b), b);
    let mut values = Vec::with_capacity(dims);
    let mut vectors = vec![0.0; n * dims];
    for d in 0..dims {
        values.push(full.values[d]);
        let z = full.column(d);
        for i in 0..n {
            vectors[d * n + i] = (0..b).map(|j| x.column(j)[i] * z[j]).sum();
        }
    }
    EigBlock {
        values,
        vectors,
        n,
        k: dims,
    }
}

/// `Xᵀ (L X)`, symmetric by mirroring `a[j][i]` onto `a[i][j]` — `a * b == b * a` exactly
/// in IEEE 754, the same argument `pivot_mds::matrix::gram` makes.
fn gram(x: &EigBlock, lx: &[f64], b: usize) -> Vec<f64> {
    let n = x.n;
    let mut a = vec![0.0; b * b];
    for j in 0..b {
        for i in j..b {
            let dot: f64 = (0..n).map(|r| x.column(i)[r] * lx[j * n + r]).sum();
            a[j * b + i] = dot;
            a[i * b + j] = dot;
        }
    }
    a
}
