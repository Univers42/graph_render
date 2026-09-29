//! The small Rayleigh-Ritz problem LOBPCG solves each iteration, split out of
//! `lobpcg.rs` to stay under the house line cap: the explicit Gram matrix (the ADR's
//! documented deviation from scipy's implicit/cached-diagonal shortcut), the dense
//! solve of it, and the combine step that lifts the small eigenvectors back to `n` rows.

use crate::linalg::EigBlock;
use crate::linalg::dense_sym::eigh;

mod generalized;

/// `(active_r, active_ar, p_ap)` for [`combine_rp`]: the active residual block, its
/// image, and the active search direction's block (absent on a restart).
type RpBlocks<'a> = (&'a [f64], &'a [f64], Option<(&'a [f64], &'a [f64])>);

/// The Rayleigh-Ritz Gram matrix `Cᵀ AC` (`C` = `basis`, `AC` = `abasis`), always
/// explicit (the ADR's deviation from scipy's implicit-Gram shortcut), symmetrised.
fn gram_matrix(basis: &[f64], abasis: &[f64], n: usize) -> Vec<f64> {
    let m = basis.len() / n;
    let mut g = vec![0.0; m * m];
    for i in 0..m {
        for j in 0..m {
            g[i * m + j] = (0..n)
                .map(|row| basis[i * n + row] * abasis[j * n + row])
                .sum();
        }
    }
    for i in 0..m {
        for j in (i + 1)..m {
            let avg = (g[i * m + j] + g[j * m + i]) / 2.0;
            g[i * m + j] = avg;
            g[j * m + i] = avg;
        }
    }
    g
}

pub(super) fn rayleigh_ritz(basis: &[f64], abasis: &[f64], n: usize) -> EigBlock {
    let gram = gram_matrix(basis, abasis, n);
    eigh(&gram, basis.len() / n)
}

/// The Rayleigh-Ritz step for `lobpcg_step`'s `[X, R]` or `[X, R, P]` basis. `P`
/// overlaps `X`/`R` (it is only ever orthonormalized within itself), so whenever it is
/// present this must solve the generalized problem, not the ordinary one; when that
/// generalized solve's Cholesky step fails, or `P` was not offered at all, this falls
/// back to the ordinary solve on `[X, R]` alone (`gramB ≈ I` there by construction).
/// Returns whether `P` actually contributed, so the caller knows whether to fold its
/// segment into the combined step.
pub(super) fn step_rayleigh_ritz(
    xax: (&[f64], &[f64]),
    rar: (&[f64], &[f64]),
    p_ap: Option<(&[f64], &[f64])>,
    n: usize,
) -> (EigBlock, bool) {
    if let Some(pap) = p_ap {
        let (basis, abasis) = build_basis(xax, rar, Some(pap));
        if let Some(small) = generalized::generalized_rayleigh_ritz(&basis, &abasis, n) {
            return (small, true);
        }
    }
    let (basis, abasis) = build_basis(xax, rar, None);
    (rayleigh_ritz(&basis, &abasis, n), false)
}

pub(super) fn build_basis(
    xax: (&[f64], &[f64]),
    rar: (&[f64], &[f64]),
    p_ap: Option<(&[f64], &[f64])>,
) -> (Vec<f64>, Vec<f64>) {
    let mut basis = xax.0.to_vec();
    basis.extend_from_slice(rar.0);
    let mut abasis = xax.1.to_vec();
    abasis.extend_from_slice(rar.1);
    if let Some((p, ap)) = p_ap {
        basis.extend_from_slice(p);
        abasis.extend_from_slice(ap);
    }
    (basis, abasis)
}

/// One contiguous run of rows `offset..offset+cols` in a small Rayleigh-Ritz
/// eigenvector, paired with the big-block basis columns those rows weight.
pub(super) struct Segment<'a> {
    pub(super) basis: &'a [f64],
    pub(super) offset: usize,
    pub(super) cols: usize,
}

impl<'a> Segment<'a> {
    /// A plain function call, unlike the `Segment { .. }` literal: rustfmt's narrow
    /// struct-literal width would otherwise force every call site onto 7 lines.
    fn new(basis: &'a [f64], offset: usize, cols: usize) -> Self {
        Self {
            basis,
            offset,
            cols,
        }
    }
}

/// Accumulates `out += basis_segment * small.vectors[.., offset..offset+cols]` for the
/// first `keep` output eigenvectors — the `pp += R @ eigR; pp += P @ eigP` pattern.
pub(super) struct Combine {
    pub(super) n: usize,
    pub(super) keep: usize,
}

impl Combine {
    pub(super) fn accumulate(&self, out: &mut [f64], small: &EigBlock, seg: Segment) {
        for outcol in 0..self.keep {
            for s in 0..seg.cols {
                let coeff = small.vectors[outcol * small.n + seg.offset + s];
                if coeff == 0.0 {
                    continue;
                }
                for row in 0..self.n {
                    out[outcol * self.n + row] += coeff * seg.basis[s * self.n + row];
                }
            }
        }
    }
}

/// Combines `small`'s first `block` output columns' `[0..block)` rows against the
/// `(X, AX)` basis segment — the `new_x`/`new_ax` step shared by `lobpcg_init`,
/// `lobpcg_step` and `lobpcg_finalize`, pulled out so none of the three needs `Combine`/
/// `Segment` boilerplate of its own.
pub(super) fn combine_xax(
    n: usize,
    block: usize,
    small: &EigBlock,
    xax: (&[f64], &[f64]),
) -> (Vec<f64>, Vec<f64>) {
    let combine = Combine { n, keep: block };
    let mut new_x = vec![0.0; n * block];
    let mut new_ax = vec![0.0; n * block];
    combine.accumulate(&mut new_x, small, Segment::new(xax.0, 0, block));
    combine.accumulate(&mut new_ax, small, Segment::new(xax.1, 0, block));
    (new_x, new_ax)
}

/// Combines `small`'s `[block..)` rows against the active `R` basis segment, and, when
/// `p_ap` is `Some`, the active `P` segment past it — `lobpcg_step`'s new search
/// direction `(P, AP)`. `blocks` is `(active_r, active_ar, p_ap)`; `p_ap`'s absence means
/// this iteration had no usable previous direction (restart), matching `use_p = false` at
/// the call site.
pub(super) fn combine_rp(
    n: usize,
    block: usize,
    small: &EigBlock,
    blocks: RpBlocks,
) -> (Vec<f64>, Vec<f64>) {
    let (r, ar, p_ap) = blocks;
    let m = r.len() / n;
    let combine = Combine { n, keep: block };
    let mut pp = vec![0.0; n * block];
    let mut app = vec![0.0; n * block];
    combine.accumulate(&mut pp, small, Segment::new(r, block, m));
    combine.accumulate(&mut app, small, Segment::new(ar, block, m));
    if let Some((p, ap)) = p_ap {
        combine.accumulate(&mut pp, small, Segment::new(p, block + m, m));
        combine.accumulate(&mut app, small, Segment::new(ap, block + m, m));
    }
    (pp, app)
}
