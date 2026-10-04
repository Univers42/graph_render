//! `layout.spectral` and `layout.spectral3d` (`docs/decisions/eigensolver.md`): `L = D - A`
//! per connected component, the smallest non-trivial eigenpairs — two coordinates for the
//! 2D id, the reference's three for the 3D one — packed onto a lattice. Ports
//! `SciGraphs/.../mesh/layouts/networkx_layouts.py`'s `_spectral_component_coordinates`
//! (`:133-164`), `_pack_component_blocks` (`:218-236`), `_rescale_positions` (`:238-247`)
//! and `_spectral_layout_3d` (`:249-269`).
//!
//! **Two ids, because the reference has two entries and one of them is 3D.** Until this
//! module grew the second id, every place the reference passed `dims=3` was ported as
//! `DIMS=2`, so `SPECTRAL_3D` compared a plane against a volume (grey a line, green a
//! cluster). `layout.spectral` keeps `DIMS = 2` and every byte it had; `layout.spectral3d`
//! is the reference's own pipeline, and the difference between the two arms is exactly the
//! [`Width`] this module passes down.
//!
//! **Scope**: this layout does not implement [`crate::stage::Stage`] (no `Params`); it is
//! registered through [`super::spectral_stage`], which drops the reports. There is
//! therefore no external `scale`; the 2D arm's reference multiplier is fixed at its
//! identity (only per-component peak-normalisation and packing run) and the 3D arm's is
//! [`SCALE`], the dispatcher's `scale = 5.0`.
//!
//! **C12 (no silent random fallback)**: a component whose solve fails the residual/
//! orthonormality gate is skipped (its nodes stay at the origin, then sit at their
//! packing cell); if *no* component solves at all, [`run`] returns
//! [`SpectralError::NothingSolved`] rather than a random layout. Every attempted
//! component's outcome is in the returned [`ComponentReport`]s — graph-core's half of
//! C12; `graph-cli` printing them is the integration step's. The one place a random
//! picture *is* the reference's answer is `_spectral_layout_3d:257-258`'s `n < 4` guard,
//! and that is [`run_3d`]'s own explicit branch.

use crate::index::Topology;
use crate::layout::random;
use crate::linalg::{EigBlock, pin_signs};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

use super::Geometry;

mod graph;
mod neighbors;
mod pack;
mod shift_invert;
pub(super) mod solve;
mod width;
use graph::ComponentGraph;
pub(crate) use neighbors::{Neighbors, find_components, local_positions, simple_neighbors};
pub(crate) use pack::{pack_component_blocks_3d, pack_components, rescale_to_scale};
use solve::solve_component;
pub(crate) use width::Width;

/// Output dimensionality of the two-dimensional ids. The reference solves 3D
/// (`dims=3`); [`DIMS_3D`] is that, and the two-dimensional arm ports every one of them.
pub const DIMS: usize = 2;
/// `_spectral_component_coordinates(G, 3)` / `_mds_component_coordinates(G, 3, 100)`.
pub const DIMS_3D: usize = 3;
/// `apply_graph_layout`'s `scale` default (`dispatcher.py:14`), the multiplier
/// `_rescale_positions` ends with. A constant and not a parameter for the reason
/// `basic_3d::SCALE` states: the reference takes one `scale` with no default of its own and
/// no caller in SciGraphs passes anything else.
pub const SCALE: f64 = 5.0;
/// `_spectral_layout_3d:257` — "Fewer than four nodes leaves too few eigenvectors to place
/// them", below which the reference draws `_random_layout` instead.
pub const MIN_NODES_3D: u32 = 4;
/// `_DENSE_EIG_LIMIT` (reference constant).
pub const DENSE_EIG_LIMIT: usize = 256;
/// `_COMPONENT_SPACING` (reference constant).
pub const COMPONENT_SPACING: f64 = 2.5;
/// The seed the registered `layout.spectral3d` hands its `n < 4` branch, where
/// `layout::random`'s registered default pins its own. The conformance arm passes the
/// layout seed through [`run_3d`] instead, the way it does for `sfdp::run_seeded`.
pub const DEFAULT_SEED: u32 = 0x00_5EED;

/// Which tier solved a component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// `n_c <= 256`: `tred2`/`tql2`.
    Dense,
    /// `n_c > 256`: LOBPCG.
    Lobpcg,
    /// LOBPCG's block missed the gate and the shift-invert retry solved it instead
    /// (`networkx_layouts.py:120-126`, `spectral/shift_invert.rs`).
    ShiftInvert,
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
    /// LOBPCG iterations run, or `None` on the dense tier and on the shift-invert retry.
    pub iterations: Option<u32>,
    /// `max_j ‖Lv_j − λ_j v_j‖`, the number the residual gate decides on and the one a
    /// caller needs to say *why* a component was skipped (C12). `None` only when no
    /// candidate was produced at all — the retry's Cholesky refused — never when one was
    /// produced and the gate said no.
    pub peak_residual: Option<f64>,
}

/// Why [`run`] produced nothing at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpectralError {
    /// No component's solve passed the residual/orthonormality gate (C12: never a
    /// silent random fallback here — an explicit refusal instead).
    NothingSolved,
    /// `_random_layout` refused, which it does not do: named rather than folded into
    /// [`SpectralError::NothingSolved`] so the `n < 4` branch cannot be mistaken for a
    /// failed eigensolve.
    RandomRefused,
}

/// Writes one solved component's (already sign-pinned, peak-normalised) coordinates into the
/// shared `coords` buffer, under `width`'s rule for a solve that reached fewer dimensions
/// than were asked for. `pub(crate)`: `layout::pivot_mds` scatters its own projected
/// coordinates the same way.
pub(crate) fn scatter(coords: &mut [f64], members: &[u32], eig: &EigBlock, width: Width) {
    let dims = width.dims();
    let peak = eig.vectors.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    for d in 0..width.columns(eig.k) {
        let source = d.min(eig.k - 1);
        for (li, &g) in members.iter().enumerate() {
            let value = eig.column(source)[li];
            coords[g as usize * dims + d] = if peak > 0.0 { value / peak } else { value };
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

/// Runs the 2D spectral layout, byte for byte what it was before the 3D arm existed.
/// `Ok` even when some components were skipped — see [`SpectralError::NothingSolved`] for
/// the only failure this returns.
pub fn run(topology: &Topology) -> Result<(Geometry, Vec<ComponentReport>), SpectralError> {
    run_width(topology, Width::Spectral2d)
}

/// `_spectral_layout_3d` (`networkx_layouts.py:249-269`) at `SCALE`: the reference's three
/// coordinates, its cubic component lattice and its `_rescale_positions`.
///
/// **Below [`MIN_NODES_3D`] this is `_random_layout` and says so.** That is the reference's
/// own guard, and it is the only random picture in this family — so it is an explicit branch
/// here, seeded by the caller's `seed` (`get_layout_seed()`), rather than a fallback any
/// other failure could reach.
pub fn run_3d(
    topology: &Topology,
    seed: u32,
) -> Result<(Geometry, Vec<ComponentReport>), SpectralError> {
    if topology.node_count() < MIN_NODES_3D {
        let geometry =
            random::run_seeded(topology, seed).map_err(|_| SpectralError::RandomRefused)?;
        return Ok((geometry, Vec::new()));
    }
    run_width(topology, Width::Spectral3d)
}

/// Both arms' pipeline: components, one solve each, then the lattice and — in 3D only —
/// the rescale. `dims_eff` and the peak normalisation are the reference's own per-component
/// rules, so they are shared; only the tail is an arm's own.
fn run_width(
    topology: &Topology,
    width: Width,
) -> Result<(Geometry, Vec<ComponentReport>), SpectralError> {
    let n = topology.node_count() as usize;
    let neighbors = simple_neighbors(topology);
    let components = find_components(&neighbors);
    let local_of = local_positions(&components, n);
    let mut coords = vec![0.0_f64; n * width.dims()];
    let mut reports = Vec::new();
    let mut any_solved = false;

    for members in &components {
        if members.len() < 2 {
            continue;
        }
        let graph = ComponentGraph::build(members, &neighbors, &local_of);
        let solve = solve_component(&graph, width);
        let ok = solve.eig.is_some();
        if let Some(mut eig) = solve.eig {
            pin_signs(&mut eig);
            scatter(&mut coords, members, &eig, width);
            any_solved = true;
        }
        reports.push(ComponentReport {
            min_index: members[0],
            size: members.len() as u32,
            tier: solve.tier,
            solved: ok,
            iterations: solve.iterations,
            peak_residual: solve.peak_residual,
        });
    }

    if nothing_solved(&components, any_solved) {
        return Err(SpectralError::NothingSolved);
    }
    if width.in_space() {
        pack_component_blocks_3d(&mut coords, &components);
        rescale_to_scale(&mut coords, width.dims(), SCALE);
    } else {
        pack_components(&mut coords, &components, width);
    }
    Ok((to_geometry(&coords, n, width), reports))
}

/// `pub(crate)`: `layout::pivot_mds` packs its own `f64` coordinate buffer into the same
/// geometry, in space when its arm says so.
pub(crate) fn to_geometry(coords: &[f64], n: usize, width: Width) -> Geometry {
    let dims = width.dims();
    let x = (0..n).map(|i| coords[i * dims] as f32).collect();
    let y = (0..n).map(|i| coords[i * dims + 1] as f32).collect();
    let points = NodeGeometry::Point { x, y };
    if !width.in_space() {
        return Geometry::planar(points, EdgeGeometry::Line, Vec::new());
    }
    let z = (0..n).map(|i| coords[i * dims + 2] as f32).collect();
    Geometry::in_space(points, EdgeGeometry::Line, Vec::new(), z)
}

#[cfg(test)]
mod tests;
