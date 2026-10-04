//! One component's solve, the three tiers it can be attempted on, and the gate that decides
//! whether an attempt is usable. Split out of `spectral.rs` to stay under the house line cap
//! and so the cascade reads in the reference's own order:
//! `dense` (`:96-98`), then LOBPCG (`:100-118`), then the shift-invert retry (`:120-129`),
//! then `None` (`:131`).

use super::graph::ComponentGraph;
use super::shift_invert;
use super::width::Width;
use crate::linalg::dense_sym::eigh;
use crate::linalg::lobpcg::lobpcg_smallest;
use crate::linalg::{EigBlock, orthonormal, residual_converged};

use super::{DENSE_EIG_LIMIT, Tier};

/// `_EIG_RESIDUAL_TOL` (`networkx_layouts.py:8`), the residual gate's own scale.
const RESIDUAL_TOL: f64 = 1e-2;
/// `‖VᵀV − I‖_max` (`docs/decisions/eigensolver.md`, decision 4 / C5).
const ORTHONORMAL_TOL: f64 = 1e-6;

/// What one component's cascade produced.
pub(crate) struct Solve {
    /// The accepted eigenvectors, or `None` when the gate refused every attempt.
    pub(super) eig: Option<EigBlock>,
    /// The tier whose candidate the gate decided on.
    pub(super) tier: Tier,
    /// LOBPCG iterations, or `None` on the dense tier and when no candidate was produced.
    pub(super) iterations: Option<u32>,
    /// `max_j ‖Lv_j − λ_j v_j‖` — the number `linalg::residual_converged` gates on and
    /// cannot return, and the one [`super::ComponentReport`] has to name.
    pub(super) peak_residual: Option<f64>,
}

/// `max_j ‖Lv_j − λ_j v_j‖`, the quantity `_eig_converged` (`:79`) compares against
/// `_EIG_RESIDUAL_TOL · max(max|λ|, 1e-12)`.
///
/// `linalg::residual_converged` is the same arithmetic and returns only the verdict, which
/// is no use to a report that has to print the number; `tests/shift_invert.rs` pins the two
/// against each other so this cannot drift into a second, laxer gate.
fn peak_residual(graph: &ComponentGraph, eig: &EigBlock) -> f64 {
    let mut av = vec![0.0; eig.n];
    let mut worst = 0.0_f64;
    for j in 0..eig.k {
        graph.matvec(eig.column(j), &mut av);
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
    worst
}

/// `_eig_converged` plus the orthonormality check, both applied on the caller's side even
/// for the dense tier (`docs/decisions/eigensolver.md`: "the reference trusts `eigh` for
/// dense; we verify anyway because it is cheap").
fn converged(graph: &ComponentGraph, eig: &EigBlock) -> (bool, f64) {
    // The verdict is `linalg`'s own, not a second copy of the same arithmetic: the report needs
    // the *number*, `residual_converged` returns only the verdict, so the peak is measured
    // alongside and the two are pinned against each other by
    // `tests::shift_invert::the_reported_peak_residual_is_the_number_the_gate_decided_on`.
    let residual = peak_residual(graph, eig);
    let matvec = |x: &[f64], y: &mut [f64]| graph.matvec(x, y);
    let passed = residual_converged(matvec, eig, RESIDUAL_TOL) && orthonormal(eig, ORTHONORMAL_TOL);
    (passed, residual)
}

/// Columns `start..start + k` of `eig`, `n` and the tail of `values` along with them.
fn sub_block(eig: &EigBlock, start: usize, k: usize) -> EigBlock {
    let mut vectors = Vec::with_capacity(eig.n * k);
    for j in start..(start + k) {
        vectors.extend_from_slice(eig.column(j));
    }
    EigBlock {
        values: eig.values[start..start + k].to_vec(),
        vectors,
        n: eig.n,
        k,
    }
}

/// One component's cascade. `width.dims()` is the reference's `dims` for the LOBPCG block
/// (`k = min(dims + 2, n - 1)`, `:102`) and for the retry's `k = dims + 1` (`:121`);
/// `dims_eff = min(dims, n_c - 1)` is its `:92`.
pub(super) fn solve_component(graph: &ComponentGraph, width: Width) -> Solve {
    let dims = width.dims();
    let dims_eff = dims.min(graph.size() - 1);
    if graph.size() <= DENSE_EIG_LIMIT {
        let full = eigh(&graph.dense_matrix(), graph.size());
        return accept(Tier::Dense, None, sub_block(&full, 1, dims_eff), graph);
    }
    let lobpcg = lobpcg_tier(graph, width, dims_eff);
    if lobpcg.eig.is_some() || graph.size() > shift_invert::DENSE_INVERT_LIMIT {
        return lobpcg;
    }
    match retry_tier(graph, dims, dims_eff) {
        Some(candidate) => accept(Tier::ShiftInvert, None, candidate, graph),
        None => lobpcg,
    }
}

/// Tier I: `maxiter`/`tol` the port's own measured values (`docs/decisions/eigensolver.md`),
/// `k = min(dims + 2, n - 1)` columns, `sub_block` from zero because the `Y = 1` constraint
/// never finds the trivial eigenvalue on purpose.
fn lobpcg_tier(graph: &ComponentGraph, width: Width, dims_eff: usize) -> Solve {
    let block = (width.dims() + 2).min(graph.size() - 1);
    let matvec = |x: &[f64], y: &mut [f64]| graph.matvec(x, y);
    let outcome = lobpcg_smallest(matvec, &graph.degree, graph.size(), block);
    accept(
        Tier::Lobpcg,
        Some(outcome.iterations),
        sub_block(&outcome.eig, 0, dims_eff),
        graph,
    )
}

/// The reference's retry (`:120-126`), reached only when tier I's block missed the gate.
fn retry_tier(graph: &ComponentGraph, dims: usize, dims_eff: usize) -> Option<EigBlock> {
    let block = (dims + 1).min(graph.size() - 1);
    shift_invert::eigenpairs(graph, block, dims_eff)
}

/// Runs the gate and packs the verdict. `peak_residual` is `None` only when no candidate was
/// produced at all (the retry's Cholesky refused), never when one was and the gate said no.
fn accept(
    tier: Tier,
    iterations: Option<u32>,
    candidate: EigBlock,
    graph: &ComponentGraph,
) -> Solve {
    let (passed, residual) = converged(graph, &candidate);
    Solve {
        eig: passed.then_some(candidate),
        tier,
        iterations,
        peak_residual: Some(residual),
    }
}

/// The raw tier-I block for `dims`, ungated and un-`sub_block`ed: the seam behind "which
/// gate refused, and on what" — the per-fixture residual measurements
/// `docs/decisions/eigensolver.md` asks to be *measured* rather than asserted a priori.
#[cfg(test)]
pub(super) fn lobpcg_block(graph: &ComponentGraph, dims: usize) -> EigBlock {
    let block = (dims + 2).min(graph.size() - 1);
    let matvec = |x: &[f64], y: &mut [f64]| graph.matvec(x, y);
    lobpcg_smallest(matvec, &graph.degree, graph.size(), block).eig
}
