//! Column-block vector operations LOBPCG's outer loop composes, split out of
//! `lobpcg.rs` to stay under the house line cap: the fixed start block, the residual,
//! the `Y = 1` constraint, the Jacobi preconditioner, and modified Gram-Schmidt.

/// Column 0 is the reference's own tie-breaking start vector (`_eig_start_vector`,
/// `networkx_layouts.py:53-64`), ported verbatim. Columns `1..block` are this port's
/// seed-independent replacement for the reference's random ones (`rng.uniform(-1, 1,
/// ...)`, approved deviation, `docs/decisions/eigensolver.md`) — a Weyl equidistribution
/// sequence (`2 * frac(i * alpha) - 1`, `alpha` irrational) per column, not a cos/sin
/// family.
///
/// Ponytail: the first cut used a fixed cos/sin frequency pair per column, like column
/// 0. That aliases directly onto a path/cycle's own cos/sin eigenbasis — a "low" fixed
/// radian frequency lands on a specific, *not necessarily smallest*, mode index
/// (`index ~ frequency * n / pi`, growing with `n`) — and measurement caught it:
/// `lobpcg_smallest` converged bit-stably to the *wrong* eigenvalue on `n = 300`.
/// `alpha` irrational makes `i * alpha mod 1` equidistribute (Weyl's theorem) with no
/// preferred frequency at any `n`, restoring the broadband coverage the reference's
/// random columns provide. Escape hatch: the caller's residual/orthonormality gate,
/// same as [`super::COLLAPSE_NORM`] — a pathological graph this still aliases against
/// is refused, not silently mislaid.
///
/// **`alpha_for` is generated rather than tabled, and the reason is [`alpha_for`]'s own
/// doc: a three-element table left a whole start column zero at the 3-D arm's `block = 5`.**

pub(super) fn start_block(n: usize, block: usize) -> Vec<f64> {
    let mut out = vec![0.0; n * block];
    for (i, slot) in out[..n].iter_mut().enumerate() {
        *slot = libm::cos(i as f64 * 0.9124345) + libm::sin(i as f64 * 0.3141593);
    }
    for col in 1..block {
        let alpha = alpha_for(col);
        for i in 0..n {
            let phase = (i as f64 + 1.0) * alpha;
            out[col * n + i] = 2.0 * (phase - phase.floor()) - 1.0;
        }
    }
    out
}

/// The Weyl irrational for start column `col`, **generated rather than tabled**.
///
/// This was a fixed `[f64; 3]` read with `.take(block - 1)`, and the 3-D spectral arm found
/// the bug. The reference's own block is `k = min(dims + 2, n_c - 1)`, so `dims = 3` asks
/// for `block = 5` where the 2-D arm asks for 4, and a three-element table over `block = 5`
/// leaves **column 4 entirely zero**. A start column of all zeros is one LOBPCG cannot move
/// off, so the solve returns the Laplacian's trivial eigenvector (eigenvalue 0.0), the
/// residual gate refuses it, and `layout.spectral3d` refused every gate seed above
/// `DENSE_EIG_LIMIT` (256) while `layout.spectral` passed every one. Measured: seed 255 of
/// the gate model, 257 nodes, is the first to reach the iterative path and the first to
/// refuse (`docs/measurements/p12-3d-oracles.md`).
///
/// **A silently-truncated table is the failure mode, and generating removes it:** there is
/// no fixed length left to forget to raise. The three historical values are returned
/// verbatim for `col` 1..=3, so every 2-D run's start block is bit-identical to what it was
/// and the 2-D hash gate does not move. Beyond them the sequence continues by
/// `frac(col * PHI)` — irrational, so it never repeats a value, and it never returns 0.0,
/// which would be exactly as degenerate as the all-zero column it replaced.
pub(super) fn alpha_for(col: usize) -> f64 {
    /// (sqrt(5) - 1) / 2, sqrt(2) - 1, sqrt(3) - 1: three mutually incommensurate
    /// irrationals, so no two columns' equidistribution can share a common near-rational
    /// resonance. Held as literals because the 2-D snapshots are pinned against them.
    const HISTORIC: [f64; 3] = [0.6180339887498949, 0.4142135623730951, 0.7320508075688772];
    match col {
        1 => HISTORIC[0],
        2 => HISTORIC[1],
        3 => HISTORIC[2],
        // The golden-ratio conjugate, the same irrational column 1 uses, stepped forward.
        _ => {
            let raw = col as f64 * HISTORIC[0];
            let frac = raw - raw.floor();
            // `frac` is irrational, so 0.0 is unreachable; this guards the rounding-up case
            // only, and an escape hatch must not itself be the degenerate value.
            if frac > 0.0 { frac } else { 0.5 }
        }
    }
}

/// `R = AX - X diag(λ)`, column by column (a gather: column `j` reads only `x`, `ax`
/// and `lambda[j]`, D10).
pub(super) fn residual_block(
    xax: (&[f64], &[f64]),
    lambda: &[f64],
    dims: (usize, usize),
) -> Vec<f64> {
    let (x, ax) = xax;
    let (n, block) = dims;
    let mut r = ax.to_vec();
    for j in 0..block {
        for row in 0..n {
            r[j * n + row] -= lambda[j] * x[j * n + row];
        }
    }
    r
}

pub(super) fn column_norms(data: &[f64], n: usize, cols: usize) -> Vec<f64> {
    (0..cols)
        .map(|j| {
            let sum_sq: f64 = (0..n)
                .map(|row| data[j * n + row] * data[j * n + row])
                .sum();
            sum_sq.sqrt()
        })
        .collect()
}

/// The `Y = 1` constraint (`_applyConstraints` with `B = None` and `Y` the constant
/// column): subtract each column's mean.
pub(super) fn subtract_column_means(data: &mut [f64], n: usize) {
    let cols = data.len() / n;
    for j in 0..cols {
        let mean: f64 = data[j * n..(j + 1) * n].iter().sum::<f64>() / n as f64;
        for v in &mut data[j * n..(j + 1) * n] {
            *v -= mean;
        }
    }
}

/// `x /= max(diag, 1e-9)`, per row, every column.
pub(super) fn apply_precond(diag: &[f64], data: &mut [f64]) {
    let n = diag.len();
    let cols = data.len() / n;
    for j in 0..cols {
        for row in 0..n {
            data[j * n + row] /= diag[row].max(1e-9);
        }
    }
}

pub(super) fn apply_matvec_block(
    matvec: &mut impl FnMut(&[f64], &mut [f64]),
    data: &[f64],
    n: usize,
) -> Vec<f64> {
    let cols = data.len() / n;
    let mut out = vec![0.0; data.len()];
    for j in 0..cols {
        matvec(&data[j * n..(j + 1) * n], &mut out[j * n..(j + 1) * n]);
    }
    out
}

pub(super) fn gather_columns(data: &[f64], n: usize, indices: &[usize]) -> Vec<f64> {
    let mut out = vec![0.0; n * indices.len()];
    for (new_j, &old_j) in indices.iter().enumerate() {
        out[new_j * n..(new_j + 1) * n].copy_from_slice(&data[old_j * n..(old_j + 1) * n]);
    }
    out
}

/// `target -= X (Xᵀ target)`: B-orthogonalises `target` against the full block `x`
/// (`b` columns), the reference's `activeBlockVectorR -= X @ (Xᵀ @ activeBlockVectorR)`.
pub(super) fn project_out_x(x: &[f64], target: &mut [f64], n: usize, b: usize) {
    let m = target.len() / n;
    for j in 0..m {
        for k in 0..b {
            let dot: f64 = (0..n).map(|row| x[k * n + row] * target[j * n + row]).sum();
            for row in 0..n {
                target[j * n + row] -= dot * x[k * n + row];
            }
        }
    }
}

/// Modified Gram-Schmidt, ascending column order, applying the same linear combination
/// to `companion` when given (so `AP` stays `A(P)`-consistent when `P` is
/// re-orthonormalised). `false` when a column collapses ([`super::COLLAPSE_NORM`]).
pub(super) fn mgs_orthonormalize(
    cols: &mut [f64],
    mut companion: Option<&mut [f64]>,
    n: usize,
    m: usize,
) -> bool {
    for j in 0..m {
        for i in 0..j {
            let dot: f64 = (0..n)
                .map(|row| cols[i * n + row] * cols[j * n + row])
                .sum();
            for row in 0..n {
                cols[j * n + row] -= dot * cols[i * n + row];
            }
            if let Some(c) = companion.as_deref_mut() {
                for row in 0..n {
                    c[j * n + row] -= dot * c[i * n + row];
                }
            }
        }
        let norm_sq: f64 = (0..n)
            .map(|row| cols[j * n + row] * cols[j * n + row])
            .sum();
        let norm = norm_sq.sqrt();
        if norm < super::COLLAPSE_NORM {
            return false;
        }
        for row in 0..n {
            cols[j * n + row] /= norm;
        }
        if let Some(c) = companion.as_deref_mut() {
            for row in 0..n {
                c[j * n + row] /= norm;
            }
        }
    }
    true
}
