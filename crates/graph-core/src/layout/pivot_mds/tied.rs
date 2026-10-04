//! LF-11: the canonical orientation of a **tied** group of Pivot MDS eigenvectors.
//!
//! `pivot_mds::top_eigenpairs` selects the `dims_eff` largest eigenpairs of the `k x k`
//! Gram matrix and copies `eigh`'s own columns for them. When two of those eigenvalues are
//! equal, the *eigenspace* is determined by the matrix and the *basis* inside it is not: any
//! rotation of the tied columns is an equally valid answer, and which one `tred2`/`tql2`
//! returns is decided by its internal arithmetic. Projecting that basis straight through
//! (`matrix::project`) lets the solver's rotation reach the drawing, where neither sign
//! pinning nor peak normalisation can take it back — `pin_signs` picks `argmax|vᵢ|`, whose
//! index moves under a rotation, and the peak is scale-invariant but not rotation-invariant.
//!
//! Measured (`pivot_mds::tests`, `c4_and_c8_land_on_their_hand_computed_orientation`): a 4-cycle
//! and an 8-cycle both have their **two largest** Gram eigenvalues equal (`4, 4` and
//! `186.5097, 186.5097`), so the 2-D arm's whole selection is one tied group — the ordinary
//! case for a symmetric input, not a corner.
//!
//! ## Where the rule runs, and why there
//!
//! On `top`, the `k x dims_eff` block of Gram eigenvectors, **before** [`matrix::project`](super::matrix::project).
//! `project` is `C @ top`, linear in `top`'s columns, so canonicalising before it and
//! projecting after is the same arithmetic as projecting first and canonicalising the
//! `n_c`-dimensional result — and `k <= 100` against `n_c` unbounded is the difference between
//! a `k x k` projector and an `n_c x n_c` one.
//!
//! ## The rule, in full
//!
//! Over `top`'s columns, in ascending eigenvalue order:
//!
//! 1. **Group.** Columns `start..end` form one group while every further column's eigenvalue
//!    is within [`TIE_TOL`] of the group's first. A lone column is its own group.
//! 2. **Skip.** A group of one is left exactly as the solver returned it. Nothing is
//!    arbitrary about a simple eigenvector's direction once [`crate::linalg::pin_signs`] has
//!    pinned its sign, so re-deriving it would move bytes for no reason.
//! 3. **Project.** The group's **`k x k` orthogonal projector** `P = Σ_d v_d v_dᵀ`, summed
//!    over the group's columns in ascending column order (D3). `P` is a function of the
//!    subspace alone: a rotation `R` inside the group gives `R Rᵀ = I`, so `P` is unchanged.
//! 4. **Canonicalise.** Modified Gram-Schmidt over **the rows of `P`**, ascending index,
//!    skipping a row whose norm falls below [`COLLAPSE_NORM`]. This is the load-bearing step,
//!    and it is stated in terms of *rows of the projector* because that is the one object a
//!    rotation cannot reach: Gram-Schmidt over an ordered list of row vectors is *not*
//!    rotation-invariant (rotating the rows changes which vector the sweep starts from), while
//!    `P` itself is.
//! 5. **Rescale.** Each canonical column is multiplied by `sqrt(lambda)` of the group, so it
//!    keeps the norm `project` would have given it: `‖C v‖² = vᵀ Cᵀ C v = vᵀ G v = lambda` for
//!    a Gram eigenvector, so every column of the group already had norm `sqrt(lambda)` and a
//!    tied group stays commensurate with the untied columns beside it.
//!
//! Ponytail: `TIE_TOL` is a relative tolerance, and its failing input is a Gram matrix whose
//! top two eigenvalues are closer together than the tolerance but whose eigenspaces are
//! genuinely different. Direction: **over**-grouping — two near-equal but distinct
//! eigenvectors are replaced by a canonical basis of their (2-dimensional) span, which is a
//! valid orthonormal pair of that span and so a valid answer to the same eigenproblem, just
//! not the one `eigh` happened to return. No residual check can see the difference (both
//! pairs satisfy `Gv = lambda v` to rounding), and neither can any drawing-invariant check;
//! under-grouping instead would leave a real tie uncanonicalised, which is the defect. The
//! number is measured, not guessed: the tied pairs in the fixtures below agree to `1e-16`
//! relative, and the nearest *untied* pair in the same fixtures is `1.4` relative — six
//! orders of magnitude apart. Escape hatch: raise it only if a fixture ever needs a
//! stricter split, and `c4_and_c8_land_on_their_hand_computed_orientation` is what would
//! then say so.

use super::{COLLAPSE_NORM, TIE_TOL};
use crate::linalg::EigBlock;

/// Replaces every tied group of `top`'s columns with the group's canonical basis. The
/// eigenvalues are left as the solver reported them — the rule picks directions, not values.
pub(super) fn canonicalise(top: &mut EigBlock) {
    for (start, len) in groups(top) {
        if len < 2 {
            continue;
        }
        for (offset, column) in canonical_columns(top, start, len).into_iter().enumerate() {
            let target = (start + offset) * top.n..(start + offset + 1) * top.n;
            top.vectors[target].copy_from_slice(&column);
        }
    }
}

/// The `(start, length)` of every tied group, ascending, over the whole block.
fn groups(top: &EigBlock) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut start = 0;
    while start < top.k {
        let mut end = start + 1;
        let largest = top.values[start].abs().max(f64::MIN_POSITIVE);
        while end < top.k && (top.values[end] - top.values[start]).abs() <= TIE_TOL * largest {
            end += 1;
        }
        found.push((start, end - start));
        start = end;
    }
    found
}

/// Step 3 to 5: the group's projector, Gram-Schmidt over its rows, rescaled by `sqrt(lambda)`.
fn canonical_columns(top: &EigBlock, start: usize, len: usize) -> Vec<Vec<f64>> {
    let p = projector(top, start, len);
    let scale = top.values[start].sqrt();
    row_space_basis(&p, top.n, len)
        .into_iter()
        .map(|mut column| {
            for value in &mut column {
                *value *= scale;
            }
            column
        })
        .collect()
}

/// `P = Σ_d v_d v_dᵀ` over the group's columns, in ascending column order (D3). Rotation-
/// invariant by construction, which is the whole reason the rule reads `P` and not the
/// columns.
fn projector(top: &EigBlock, start: usize, len: usize) -> Vec<f64> {
    let k = top.n;
    let mut p = vec![0.0; k * k];
    for d in start..(start + len) {
        let v = top.column(d);
        for i in 0..k {
            for j in 0..=i {
                p[i * k + j] += v[i] * v[j];
            }
        }
    }
    for i in 0..k {
        for j in 0..i {
            p[j * k + i] = p[i * k + j];
        }
    }
    p
}

/// Modified Gram-Schmidt over the rows of the `n x n` `p` in ascending index, taking `len` of
/// them. `p` is symmetric, so row `i` and column `i` coincide and either reads the same row.
fn row_space_basis(p: &[f64], n: usize, len: usize) -> Vec<Vec<f64>> {
    let mut basis: Vec<Vec<f64>> = Vec::with_capacity(len);
    for r in 0..n {
        let row: Vec<f64> = (0..n).map(|c| p[r * n + c]).collect();
        let mut next = project_out(&row, &basis);
        let norm = norm_of(&next);
        if norm < COLLAPSE_NORM {
            continue;
        }
        for value in &mut next {
            *value /= norm;
        }
        basis.push(next);
        if basis.len() == len {
            break;
        }
    }
    basis
}

/// `v` with every component of every already-accepted basis vector removed, in ascending
/// basis order.
fn project_out(v: &[f64], basis: &[Vec<f64>]) -> Vec<f64> {
    let mut out = v.to_vec();
    for b in basis {
        let dot: f64 = out.iter().zip(b).map(|(a, c)| a * c).sum();
        for (o, c) in out.iter_mut().zip(b) {
            *o -= dot * c;
        }
    }
    out
}

fn norm_of(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}
