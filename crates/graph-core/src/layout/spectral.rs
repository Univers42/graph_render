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

use super::Geometry;

mod graph;
use graph::ComponentGraph;
pub(crate) use space::{pack_components, scatter, to_geometry};
mod space;

/// Output dimensionality of the 2D arm. The reference solves 3D (`dims=3`); the 2D
/// `Point` geometry is what forced this to 2 while the motor had no z column.
pub const DIMS: usize = 2;

/// Output dimensionality of the 3D arm, the reference's own `dims=3`
/// (`_spectral_layout_3d`, `networkx_layouts.py:260`).
pub const DIMS_3D: usize = 3;
/// `_DENSE_EIG_LIMIT` (reference constant).
pub const DENSE_EIG_LIMIT: usize = 256;
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

/// Simple, undirected, collapsed adjacency (C6): every node's distinct neighbours,
/// self-loops dropped, parallel edges collapsed, ascending. Built once per call by a
/// sort + dedup over `(row, col)` pairs — no hashing, D4.
pub(crate) fn simple_neighbors(topology: &Topology) -> Vec<Vec<u32>> {
    let n = topology.node_count() as usize;
    let edges = topology.edges();
    let mut pairs: Vec<(u32, u32)> = Vec::with_capacity(edges.source.len() * 2);
    for (&s, &t) in edges.source.iter().zip(&edges.target) {
        if s != t {
            pairs.push((s, t));
            pairs.push((t, s));
        }
    }
    pairs.sort_unstable();
    pairs.dedup();
    let mut neighbors = vec![Vec::new(); n];
    for (row, col) in pairs {
        neighbors[row as usize].push(col);
    }
    neighbors
}

/// Maps each component member's dense index to its local index (position in
/// `members`). Shared by `layout::spectral`'s Laplacian [`graph::ComponentGraph`] and
/// `layout::pivot_mds`'s BFS graph — both address neighbours by local index, neither
/// needs the other's per-node payload (degree here, none there), so the types stay
/// separate but this ~5-line construction does not.
pub(crate) fn local_index_map(members: &[u32], n: usize) -> Vec<u32> {
    let mut local_of = vec![u32::MAX; n];
    for (li, &g) in members.iter().enumerate() {
        local_of[g as usize] = li as u32;
    }
    local_of
}

/// Connected components by BFS from the lowest unvisited index, members sorted
/// ascending (`_connected_component_indices`), components in discovery order — which is
/// already ascending by minimum index.
pub(crate) fn find_components(neighbors: &[Vec<u32>]) -> Vec<Vec<u32>> {
    let n = neighbors.len();
    let mut visited = vec![false; n];
    let mut components = Vec::new();
    for start in 0..n as u32 {
        if visited[start as usize] {
            continue;
        }
        components.push(bfs_component(neighbors, &mut visited, start));
    }
    components
}

fn bfs_component(neighbors: &[Vec<u32>], visited: &mut [bool], start: u32) -> Vec<u32> {
    let mut members = Vec::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(start);
    visited[start as usize] = true;
    while let Some(v) = queue.pop_front() {
        members.push(v);
        for &w in &neighbors[v as usize] {
            if !visited[w as usize] {
                visited[w as usize] = true;
                queue.push_back(w);
            }
        }
    }
    members.sort_unstable();
    members
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
    let block = (dims_eff + 2).min(graph.size() - 1);
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

/// C12's only failure: at least one component was attempted (size `>= 2`) and none of
/// the attempts solved. An all-singleton graph attempts nothing, so that is not this
/// error — every node is already placed (at the origin, then its packing cell) with no
/// solve ever having been tried. `pub(crate)`: `layout::pivot_mds` has the identical rule
/// for `MdsError::NothingSolved`.
pub(crate) fn nothing_solved(components: &[Vec<u32>], any_solved: bool) -> bool {
    components.iter().any(|c| c.len() >= 2) && !any_solved
}

/// Runs the spectral layout at [`DIMS`]. `Ok` even when some components were skipped —
/// see [`SpectralError::NothingSolved`] for the only failure this returns.
pub fn run(topology: &Topology) -> Result<(Geometry, Vec<ComponentReport>), SpectralError> {
    run_at(topology, DIMS)
}

/// The same layout at `dims` output dimensions, which is the whole of what separates
/// `layout.spectral` from `layout.spectral.3d`: the number of eigenpairs kept, the
/// packing lattice, and whether the geometry carries a z column.
///
/// **The reference's `n < 4` random fallback is not ported here, in either arm.** The
/// reference returns `_random_layout(n, scale)` for `n < 4` (`networkx_layouts.py:257-258`)
/// and again for a graph no component solved (`:262-263`); this motor refuses instead
/// ([`SpectralError::NothingSolved`], C12) because a silently random picture is worse than
/// a stated failure, and `eigensolver.md:238` records the same decision for 2D. A 3-node
/// graph therefore *solves* here — `dims_eff = min(3, 2) = 2` non-trivial eigenpairs, the
/// third dimension filled by [`scatter`]'s "copy the last column" rule — where the
/// reference would have scattered it at random. Recorded in `docs/measurements/p12-t4a.md`.
pub fn run_at(
    topology: &Topology,
    dims: usize,
) -> Result<(Geometry, Vec<ComponentReport>), SpectralError> {
    let n = topology.node_count() as usize;
    let neighbors = simple_neighbors(topology);
    let components = find_components(&neighbors);
    let mut coords = vec![0.0_f64; n * dims];
    let mut reports = Vec::new();
    let mut any_solved = false;

    for members in &components {
        if members.len() < 2 {
            continue;
        }
        let graph = ComponentGraph::build(members, &neighbors, n);
        let dims_eff = dims.min(graph.size() - 1);
        let (solved, tier, iterations) = solve_component(&graph, dims_eff);
        let ok = solved.is_some();
        if let Some(eig) = solved {
            let mut eig = eig;
            pin_signs(&mut eig);
            scatter(&mut coords, members, &eig, dims);
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
    pack_components(&mut coords, &components, dims);
    Ok((to_geometry(&coords, n, dims), reports))
}

#[cfg(test)]
mod tests;
