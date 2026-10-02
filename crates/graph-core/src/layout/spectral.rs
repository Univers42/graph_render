//! `layout.spectral` (`docs/decisions/eigensolver.md`): `L = D - A` per connected
//! component, the smallest `dims = 2` non-trivial eigenpairs, packed onto a 2D lattice.
//! Ports `SciGraphs/.../mesh/layouts/networkx_layouts.py`'s `_spectral_component_
//! coordinates` (`:133-164`) and `_pack_component_blocks` (`:218-236`, adapted to 2D).
//!
//! **Scope**: this layout does not implement [`crate::stage::Stage`] (no `Params`); it is
//! registered through [`super::spectral_stage`], which drops the reports. There is
//! therefore no external `scale`; the reference's final `_rescale_positions` multiplier
//! is fixed at its identity (only per-component peak-normalisation and packing run).
//!
//! **C12 (no silent random fallback)**: a component whose solve fails the residual/
//! orthonormality gate is skipped (its nodes stay at the origin, then sit at their
//! packing cell); if *no* component solves at all, [`run`] returns
//! [`SpectralError::NothingSolved`] rather than a random layout. Every attempted
//! component's outcome is in the returned [`ComponentReport`]s — graph-core's half of
//! C12; `graph-cli` printing them is the integration step's.

use crate::index::Topology;
use crate::linalg::dense_sym::eigh;
use crate::linalg::lobpcg::lobpcg_smallest;
use crate::linalg::{EigBlock, orthonormal, pin_signs, residual_converged};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

use super::Geometry;

mod graph;
mod neighbors;
use graph::ComponentGraph;
pub(crate) use neighbors::{Neighbors, find_components, local_positions, simple_neighbors};

/// Output dimensionality. The reference solves 3D (`dims=3`); our `Point` geometry is
/// 2D, so every place the reference passes `dims=3` this ports as `DIMS=2`.
pub const DIMS: usize = 2;
/// `_DENSE_EIG_LIMIT` (reference constant).
pub const DENSE_EIG_LIMIT: usize = 256;
/// `_COMPONENT_SPACING` (reference constant).
const COMPONENT_SPACING: f64 = 2.5;

/// Which tier solved a component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// `n_c <= 256`: `tred2`/`tql2`.
    Dense,
    /// `n_c > 256`: LOBPCG.
    Lobpcg,
}

/// One component's outcome — graph-core's half of C12's "show per-component residuals
/// and skipped components through a diagnostic".
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentReport {
    /// The component's lowest dense index (its identity across a run).
    pub min_index: u32,
    /// Members.
    pub size: u32,
    /// Which tier attempted it.
    pub tier: Tier,
    /// Passed both the residual and orthonormality gate.
    pub solved: bool,
    /// LOBPCG iterations run, or `None` on the dense tier.
    pub iterations: Option<u32>,
}

/// Why [`run`] produced nothing at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpectralError {
    /// No component's solve passed the residual/orthonormality gate (C12: never a
    /// silent random fallback here — an explicit refusal instead).
    NothingSolved,
}

/// `_eig_converged` plus the orthonormality check, both applied on the caller's side
/// even for the dense tier (`docs/decisions/eigensolver.md`: "the reference trusts
/// `eigh` for dense; we verify anyway because it is cheap").
fn converged(graph: &ComponentGraph, eig: &EigBlock) -> bool {
    let matvec = |x: &[f64], y: &mut [f64]| graph.matvec(x, y);
    residual_converged(matvec, eig, 1e-2) && orthonormal(eig, 1e-6)
}

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

/// One component's solve: dense when `n_c <= 256`, else LOBPCG. Returns the accepted
/// eigenvectors (already sign-pinned) or `None` when the gate refuses them.
fn solve_component(
    graph: &ComponentGraph,
    dims_eff: usize,
) -> (Option<EigBlock>, Tier, Option<u32>) {
    if graph.size() <= DENSE_EIG_LIMIT {
        let full = eigh(&graph.dense_matrix(), graph.size());
        let candidate = sub_block(&full, 1, dims_eff);
        let ok = converged(graph, &candidate);
        return (ok.then_some(candidate), Tier::Dense, None);
    }
    let block = (DIMS + 2).min(graph.size() - 1);
    let matvec = |x: &[f64], y: &mut [f64]| graph.matvec(x, y);
    let outcome = lobpcg_smallest(matvec, &graph.degree, graph.size(), block);
    let candidate = sub_block(&outcome.eig, 0, dims_eff);
    let ok = converged(graph, &candidate);
    (
        ok.then_some(candidate),
        Tier::Lobpcg,
        Some(outcome.iterations),
    )
}

/// Writes one solved component's (already sign-pinned, peak-normalised) coordinates
/// into the shared `coords` buffer, filling any dimension past `dims_eff` with the last
/// solved column (`n_c = 2`'s "copy the last column into y", generalised). `pub(crate)`:
/// `layout::pivot_mds` scatters its own projected coordinates the same way.
pub(crate) fn scatter(coords: &mut [f64], members: &[u32], eig: &EigBlock) {
    let peak = eig.vectors.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    for d in 0..DIMS {
        let source = d.min(eig.k - 1);
        for (li, &g) in members.iter().enumerate() {
            let value = eig.column(source)[li];
            coords[g as usize * DIMS + d] = if peak > 0.0 { value / peak } else { value };
        }
    }
}

/// The 2D-adapted lattice packing (`docs/decisions/eigensolver.md`): side
/// `ceil(sqrt(components))`, per-component scale `sqrt(n_c / biggest)`, stable sort on
/// `(-size, min_index)`. `pub(crate)`: shared with `layout::pivot_mds`, which packs the
/// same shape of per-component blocks.
pub(crate) fn pack_components(coords: &mut [f64], components: &[Vec<u32>]) {
    if components.len() < 2 {
        return;
    }
    let mut order: Vec<usize> = (0..components.len()).collect();
    order.sort_by(|&a, &b| {
        components[b]
            .len()
            .cmp(&components[a].len())
            .then(components[a][0].cmp(&components[b][0]))
    });
    let biggest = components.iter().map(Vec::len).max().unwrap_or(1) as f64;
    let side = (components.len() as f64).sqrt().ceil().max(1.0) as usize;
    let offset = (side as f64 - 1.0) * COMPONENT_SPACING * 0.5;
    let original = coords.to_vec();
    for (slot, &c) in order.iter().enumerate() {
        let scale = (components[c].len() as f64 / biggest).sqrt();
        let cell = ((slot % side) as f64, (slot / side) as f64);
        for &g in &components[c] {
            let (gi, base) = (g as usize, g as usize * DIMS);
            coords[base] = original[base] * scale + cell.0 * COMPONENT_SPACING - offset;
            coords[base + 1] = original[base + 1] * scale + cell.1 * COMPONENT_SPACING - offset;
            let _ = gi;
        }
    }
}

/// C12's only failure: at least one component was attempted (size `>= 2`) and none of
/// the attempts solved. An all-singleton graph attempts nothing, so that is not this
/// error — every node is already placed (at the origin, then its packing cell) with no
/// solve ever having been tried. `pub(crate)`: `layout::pivot_mds` has the identical rule
/// for `MdsError::NothingSolved`.
pub(crate) fn nothing_solved(components: &[Vec<u32>], any_solved: bool) -> bool {
    components.iter().any(|c| c.len() >= 2) && !any_solved
}

/// Runs the spectral layout. `Ok` even when some components were skipped — see
/// [`SpectralError::NothingSolved`] for the only failure this returns.
pub fn run(topology: &Topology) -> Result<(Geometry, Vec<ComponentReport>), SpectralError> {
    let n = topology.node_count() as usize;
    let neighbors = simple_neighbors(topology);
    let components = find_components(&neighbors);
    let local_of = local_positions(&components, n);
    let mut coords = vec![0.0_f64; n * DIMS];
    let mut reports = Vec::new();
    let mut any_solved = false;

    for members in &components {
        if members.len() < 2 {
            continue;
        }
        let graph = ComponentGraph::build(members, &neighbors, &local_of);
        let dims_eff = DIMS.min(graph.size() - 1);
        let (solved, tier, iterations) = solve_component(&graph, dims_eff);
        let ok = solved.is_some();
        if let Some(eig) = solved {
            let mut eig = eig;
            pin_signs(&mut eig);
            scatter(&mut coords, members, &eig);
            any_solved = true;
        }
        reports.push(ComponentReport {
            min_index: members[0],
            size: members.len() as u32,
            tier,
            solved: ok,
            iterations,
        });
    }

    if nothing_solved(&components, any_solved) {
        return Err(SpectralError::NothingSolved);
    }
    pack_components(&mut coords, &components);
    Ok((to_geometry(&coords, n), reports))
}

/// `pub(crate)`: `layout::pivot_mds` packs its own `f64` coordinate buffer into the same
/// `f32` `Point` geometry.
pub(crate) fn to_geometry(coords: &[f64], n: usize) -> Geometry {
    let x = (0..n).map(|i| coords[i * DIMS] as f32).collect();
    let y = (0..n).map(|i| coords[i * DIMS + 1] as f32).collect();
    Geometry::planar(NodeGeometry::Point { x, y }, EdgeGeometry::Line, Vec::new())
}

#[cfg(test)]
mod tests;
