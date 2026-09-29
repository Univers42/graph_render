//! Tier I — LOBPCG (`docs/decisions/eigensolver.md`), ported from scipy `_lobpcg.py`'s
//! `B = None` path. The `Y = 1` constraint is specialised to column mean-subtraction
//! (exact for the constant constraint), the Rayleigh-Ritz Gram matrix is always
//! computed explicitly (scipy's implicit/cached-diagonal shortcut is a documented
//! simplification, not ported), and soft-locking plus the Cholesky/Gram-Schmidt-failure
//! restart are ported as scipy has them. `sizeX` in the scipy signature is `block` here.

use super::EigBlock;

mod ops;
mod ritz;

/// Ponytail: the reference's own `_LOBPCG_MAXITER` is `300`, sized for its `k - 1`
/// *random* start columns (`docs/decisions/eigensolver.md`'s approved seed-independence
/// deviation replaces them with fixed Weyl columns, `ops::start_block`). Measurement
/// caught that the trade: even the smallest LOBPCG-tier fixture, a `257`-node path just
/// above `DENSE_EIG_LIMIT`, needed `410` iterations against this port's own weaker
/// (constant-diagonal) preconditioner on a path/cycle's `O(1/n^2)` eigenvalue gap — `300`
/// is not merely tight here, it is short for every required fixture, not only large `n`.
/// `1500` is a measured, not guessed, bound: comfortably above the worst required-fixture
/// count seen (`545`, a `400`-node cycle) with headroom, while still bounded (D8: a step
/// count, not a clock). Under-reporting direction: exhausting the cap without meeting the
/// gate below returns the component's best iterate anyway; the caller's own
/// residual/orthonormality check (not this count) is what refuses to trust it. Escape
/// hatch: `layout::spectral`'s `converged` gate, identical to Tier D's.
const MAXITER: u32 = 1500;
/// The reference's internal tolerance (`networkx_layouts.py:111`), unscaled — scipy
/// uses it raw when the caller supplies it, which the reference does.
const TOL: f64 = 1e-6;

/// Ponytail: a Gram-Schmidt column below this norm is treated as linearly dependent on
/// the current basis, which is exactly what happens as a search direction converges
/// (its residual heads to the same subspace already spanned). Under-reporting
/// direction: rather than divide by a near-zero norm, the column — or, if it is the
/// last active residual direction, the whole iteration — is dropped, so the returned
/// block can be short of full accuracy on an adversarial spectrum. Escape hatch: the
/// caller's residual/orthonormality gate (`docs/decisions/eigensolver.md`), which marks
/// the component unsolved rather than trusting a partially-converged block.
const COLLAPSE_NORM: f64 = 1e-10;

/// One iteration's updated `(X, AX, λ, P, AP)`.
type StepUpdate = (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>);

/// What stays fixed across every iteration of one [`lobpcg_smallest`] call, bundled so
/// [`lobpcg_step`] takes 4 parameters, not 6.
struct StepCtx<'a> {
    diag: &'a [f64],
    n: usize,
    block: usize,
}

/// What one call to [`lobpcg_smallest`] produced.
#[derive(Debug, Clone, PartialEq)]
pub struct LobpcgOutcome {
    /// The `block` smallest eigenpairs found, ascending.
    pub eig: EigBlock,
    /// Every column's final residual was at most [`TOL`] (scipy's own, unscaled, exit
    /// criterion — the caller applies its own scaled gate separately).
    pub converged: bool,
    /// Iterations actually run, at most 1500 (`MAXITER`).
    pub iterations: u32,
}

/// The `block` smallest eigenpairs of the symmetric operator `matvec`, `Y = 1`
/// constrained (the constant vector is projected out of every iterate), `diag`
/// preconditioned (`x /= max(diag, 1e-9)`, applied by the caller through `diag`
/// already floored — see `layout::spectral`). `block` is fixed by the caller
/// (`dims + 2`); this file does not compute it.
pub fn lobpcg_smallest(
    mut matvec: impl FnMut(&[f64], &mut [f64]),
    diag: &[f64],
    n: usize,
    block: usize,
) -> LobpcgOutcome {
    let (mut x, mut ax, mut lambda) = lobpcg_init(&mut matvec, n, block);
    let mut active = vec![true; block];
    let (mut p, mut ap) = (vec![0.0; n * block], vec![0.0; n * block]);
    let mut has_p = false;
    let mut best = (x.clone(), f64::MAX);
    let mut iterations = 0;
    let ctx = StepCtx { diag, n, block };

    for iter in 0..=MAXITER {
        iterations = iter;
        let r = ops::residual_block((&x, &ax), &lambda, (n, block));
        let idx = update_progress((&x, &r), &mut active, &mut best, &ctx);
        if idx.is_empty() {
            break;
        }
        let it = Iterate {
            x: &x,
            ax: &ax,
            r: &r,
            p: &p,
            ap: &ap,
            has_p,
        };
        let Some(next) = lobpcg_step(&mut matvec, &ctx, &it, &idx) else {
            break;
        };
        (x, ax, lambda, p, ap) = next;
        has_p = true;
    }

    lobpcg_finalize(&mut matvec, best.0, (n, block), iterations)
}

/// Updates the best-seen iterate (`best = (x, average residual norm)`) and each column's
/// active flag from this iteration's residual `r`, returning the still-active column
/// indices — empty means every column met [`TOL`].
fn update_progress(
    xr: (&[f64], &[f64]),
    active: &mut [bool],
    best: &mut (Vec<f64>, f64),
    ctx: &StepCtx<'_>,
) -> Vec<usize> {
    let (x, r) = xr;
    let norms = ops::column_norms(r, ctx.n, ctx.block);
    let avg = norms.iter().sum::<f64>() / ctx.block as f64;
    if avg < best.1 {
        best.1 = avg;
        best.0.copy_from_slice(x);
    }
    for j in 0..ctx.block {
        if norms[j] <= TOL {
            active[j] = false;
        }
    }
    (0..ctx.block).filter(|&j| active[j]).collect()
}

/// The state one Rayleigh-Ritz step reads: the current block, its residual, and the
/// previous search direction (absent on iteration 0).
struct Iterate<'a> {
    x: &'a [f64],
    ax: &'a [f64],
    r: &'a [f64],
    p: &'a [f64],
    ap: &'a [f64],
    has_p: bool,
}

fn lobpcg_init(
    matvec: &mut impl FnMut(&[f64], &mut [f64]),
    n: usize,
    block: usize,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut x = ops::start_block(n, block);
    ops::subtract_column_means(&mut x, n);
    if x[..n].iter().all(|v| *v == 0.0) {
        x[0] = -1.0;
        x[1..n].fill(1.0);
    }
    ops::mgs_orthonormalize(&mut x, None, n, block);
    let ax = ops::apply_matvec_block(matvec, &x, n);
    let small = ritz::rayleigh_ritz(&x, &ax, n);
    let lambda = small.values[..block].to_vec();
    let (new_x, new_ax) = ritz::combine_xax(n, block, &small, (&x, &ax));
    (new_x, new_ax, lambda)
}

/// One LOBPCG iteration: builds the active residual (and, if available, the active
/// search direction), runs Rayleigh-Ritz over `span[X, R, P]` (or `span[X, R]` on
/// restart), and returns the updated `(X, AX, λ, P, AP)`. `None` when the active
/// residual itself collapses under Gram-Schmidt (scipy's "failed, break").
fn lobpcg_step(
    matvec: &mut impl FnMut(&[f64], &mut [f64]),
    ctx: &StepCtx<'_>,
    it: &Iterate<'_>,
    idx: &[usize],
) -> Option<StepUpdate> {
    let (n, block) = (ctx.n, ctx.block);
    let m = idx.len();
    let mut active_r = ops::gather_columns(it.r, n, idx);
    ops::apply_precond(ctx.diag, &mut active_r);
    ops::subtract_column_means(&mut active_r, n);
    ops::project_out_x(it.x, &mut active_r, n, block);
    if !ops::mgs_orthonormalize(&mut active_r, None, n, m) {
        return None;
    }
    let active_ar = ops::apply_matvec_block(matvec, &active_r, n);

    let (active_p, active_ap, p_ready) = gather_active_p(it, idx, n, m);

    let p_ap = p_ready.then_some((active_p.as_slice(), active_ap.as_slice()));
    let (small, use_p) = ritz::step_rayleigh_ritz((it.x, it.ax), (&active_r, &active_ar), p_ap, n);
    let lambda = small.values[..block].to_vec();

    let (mut new_x, mut new_ax) = ritz::combine_xax(n, block, &small, (it.x, it.ax));
    let p_for_combine = use_p.then_some((active_p.as_slice(), active_ap.as_slice()));
    let (pp, app) = ritz::combine_rp(n, block, &small, (&active_r, &active_ar, p_for_combine));
    for i in 0..(n * block) {
        new_x[i] += pp[i];
        new_ax[i] += app[i];
    }
    Some((new_x, new_ax, lambda, pp, app))
}

/// Gathers the previous search direction's active columns (`P`, `AP`) at `idx` and
/// orthonormalizes them against each other (scipy's restart-on-failure path):
/// `p_ready` is `false` when there is no previous direction yet (iteration 0) or when
/// the gathered columns collapse under Gram-Schmidt, either way telling the caller to
/// drop `P` from this step's Rayleigh-Ritz span. Split out of [`lobpcg_step`] to stay
/// under the house line cap.
fn gather_active_p(
    it: &Iterate<'_>,
    idx: &[usize],
    n: usize,
    m: usize,
) -> (Vec<f64>, Vec<f64>, bool) {
    if !it.has_p {
        return (Vec::new(), Vec::new(), false);
    }
    let mut active_p = ops::gather_columns(it.p, n, idx);
    let mut active_ap = ops::gather_columns(it.ap, n, idx);
    let p_ready = ops::mgs_orthonormalize(&mut active_p, Some(&mut active_ap), n, m);
    (active_p, active_ap, p_ready)
}

/// The final "exact" Rayleigh-Ritz pass on the best iterate seen (scipy's
/// postprocessing step), and the resulting [`LobpcgOutcome`].
fn lobpcg_finalize(
    matvec: &mut impl FnMut(&[f64], &mut [f64]),
    x: Vec<f64>,
    dims: (usize, usize),
    iterations: u32,
) -> LobpcgOutcome {
    let (n, block) = dims;
    let ax = ops::apply_matvec_block(matvec, &x, n);
    let small = ritz::rayleigh_ritz(&x, &ax, n);
    let lambda = small.values[..block].to_vec();
    let (fx, fax) = ritz::combine_xax(n, block, &small, (&x, &ax));
    let residual = ops::residual_block((&fx, &fax), &lambda, (n, block));
    let converged = ops::column_norms(&residual, n, block)
        .iter()
        .all(|&v| v <= TOL);
    LobpcgOutcome {
        eig: EigBlock {
            values: lambda,
            vectors: fx,
            n,
            k: block,
        },
        converged,
        iterations,
    }
}

#[cfg(test)]
mod tests;
