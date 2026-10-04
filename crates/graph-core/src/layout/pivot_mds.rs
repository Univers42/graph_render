//! `layout.mds.pivot` and `layout.mds.pivot3d` (`docs/decisions/eigensolver.md`): pivot
//! selection by a farthest-point BFS heuristic, double-centered squared hop-distances, then
//! the largest `dims` eigenpairs of the `k x k` `Cᵀ C` Gram matrix (`k = min(100, n_c)`),
//! projected back to `n_c` points and sign-pinned. Ports
//! `_pivot_mds_coordinates`/`_pivot_mds_component_coordinates` (`:166-216`) and
//! `_mds_layout_3d` (`:271-291`). Components, adjacency (C6), the lattices and the rescale
//! are `layout::spectral`'s, reused here through its `pub(crate)` items.
//!
//! **Two ids, two widths.** `layout.mds.pivot` solves two coordinates per node and is
//! byte-identical to what it was before the 3D arm existed; `layout.mds.pivot3d` is the
//! reference's own three, and is the row the conformance matrix calls `MDS_3D`.
//!
//! **Scope**: as `layout::spectral` — no [`crate::stage::Stage`], no external `scale`;
//! registered through [`super::spectral_stage`].
//!
//! **C12**: identical shape to `layout::spectral`'s — a component that fails the small
//! eigensolve's residual/orthonormality gate is skipped ((0, 0), then its packing
//! cell); [`MdsError::NothingSolved`] only when something was attempted and nothing
//! solved (`nothing_solved`, shared with `spectral`).

use crate::index::Topology;
use crate::layout::random;
use crate::linalg::dense_sym::eigh;
use crate::linalg::{EigBlock, orthonormal, pin_signs};

use super::Geometry;
use super::spectral::{
    MIN_NODES_3D, Neighbors, SCALE, Width, find_components, local_positions, nothing_solved,
    pack_component_blocks_3d, pack_components, rescale_to_scale, scatter, simple_neighbors,
    to_geometry,
};

mod matrix;
mod tied;
use matrix::{Centered, gram, project};
use tied::canonicalise;

/// `_MDS_PIVOTS` (reference constant).
pub const MAX_PIVOTS: usize = 100;

/// Relative width within which two Gram eigenvalues count as **tied** for LF-11's
/// canonicalisation (`pivot_mds/tied.rs` states the rule and where this number comes from;
/// it is the sole knob of that rule).
const TIE_TOL: f64 = 1e-9;

/// A row of a tied group's block whose norm is below this is linearly dependent on the rows
/// already taken. The same floor `linalg::lobpcg` drops a collapsing Gram-Schmidt column at.
const COLLAPSE_NORM: f64 = 1e-10;

/// One component's outcome — Pivot MDS's half of C12, the same shape as
/// [`super::spectral::ComponentReport`] minus the tier (Pivot MDS is always dense).
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentReport {
    /// The component's lowest dense index.
    pub min_index: u32,
    /// Members.
    pub size: u32,
    /// Pivots used, `min(100, size)`.
    pub pivots: u32,
    /// Passed both the residual and orthonormality gate.
    pub solved: bool,
    /// `max_j ‖Gv_j − λ_j v_j‖` on the `k x k` Gram solve, the number the residual gate
    /// decides on and the one a caller needs to say *why* a component was skipped (C12).
    pub peak_residual: f64,
}

/// Why [`run`] produced nothing at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdsError {
    /// No component's solve passed the gate (C12: an explicit refusal, never a silent
    /// random fallback).
    NothingSolved,
    /// `_random_layout` refused, which it does not do. Named rather than folded into
    /// [`MdsError::NothingSolved`] so the `n < 4` branch cannot be mistaken for a failed
    /// eigensolve — see [`run_3d`].
    RandomRefused,
}

/// Hop counts from `start` to every node of its component, into `hops`, which holds
/// `u32::MAX` on entry and doubles as the visited mark; `queue` has a slot per node. A
/// component is connected by construction, so the reference's `d[~np.isfinite(d)] = 0.0`
/// branch never triggers on this caller and is not ported.
fn walk(graph: &Neighbors, start: u32, hops: &mut [u32], queue: &mut [u32]) {
    hops[start as usize] = 0;
    queue[0] = start;
    let (mut head, mut tail) = (0, 1);
    while head < tail {
        let v = queue[head];
        head += 1;
        let next = hops[v as usize] + 1;
        for &w in graph.row(v) {
            if hops[w as usize] == u32::MAX {
                hops[w as usize] = next;
                queue[tail] = w;
                tail += 1;
            }
        }
    }
}

/// `np.argmax`'s own tie rule: the first (lowest-index) occurrence of the maximum.
fn argmax(v: &[u32]) -> usize {
    let mut best = 0;
    for i in 1..v.len() {
        if v[i] > v[best] {
            best = i;
        }
    }
    best
}

/// The hop counts to each pivot over one component's own adjacency (`graph`, local
/// indices), pivot `j`'s column being `[j * n..(j + 1) * n]`, each walk writing its column
/// in place. The pivots are chosen by the farthest-point heuristic
/// (`_pivot_mds_coordinates:176-185`): pivot 0 is local index 0; each next pivot is the
/// node currently farthest (by minimum hop count) from every pivot chosen so far, ties won
/// by the lowest index. A chosen pivot's own minimum is 0 and every other node's is at
/// least 1, so the reference's `-1` mark on chosen pivots is never needed here.
fn pivot_hops(graph: &Neighbors, k: usize) -> Vec<u32> {
    let n = graph.len();
    let mut hops = vec![u32::MAX; n * k];
    let mut queue = vec![0; n];
    let mut covered = vec![u32::MAX; n];
    let mut chosen = 0;
    for column in hops.chunks_exact_mut(n) {
        walk(graph, chosen as u32, column, &mut queue);
        for (c, &h) in covered.iter_mut().zip(column.iter()) {
            *c = (*c).min(h);
        }
        chosen = argmax(&covered);
    }
    hops
}

/// The `dims_eff` largest eigenpairs of `full` (ascending input, reversed selection —
/// `vectors[:, ::-1][:, :dims_eff]`).
fn top_eigenpairs(full: &EigBlock, dims_eff: usize) -> EigBlock {
    let k = full.n;
    let mut values = Vec::with_capacity(dims_eff);
    let mut vectors = Vec::with_capacity(k * dims_eff);
    for d in 0..dims_eff {
        let src = full.k - 1 - d;
        values.push(full.values[src]);
        vectors.extend_from_slice(full.column(src));
    }
    EigBlock {
        values,
        vectors,
        n: k,
        k: dims_eff,
    }
}

/// `_eig_converged` plus the orthonormality check, run on the small `k x k` eigensolve
/// (`docs/decisions/eigensolver.md`'s residual/orthonormality section covers this file
/// too: "the reference trusts `eigh`... we verify anyway because it is cheap" applies
/// just as well to Pivot MDS's own dense solve), plus the peak residual the C12 report has
/// to name. Same arithmetic as `crate::linalg::residual_converged`, which returns only the
/// verdict; `layout::spectral::tests` pins the two against each other.
fn converged(gram: &[f64], k: usize, eig: &EigBlock) -> (bool, f64) {
    let mut residual = 0.0_f64;
    let mut av = vec![0.0; k];
    for j in 0..eig.k {
        for row in 0..k {
            av[row] = (0..k)
                .map(|col| gram[row * k + col] * eig.column(j)[col])
                .sum();
        }
        let norm: f64 = av
            .iter()
            .zip(eig.column(j))
            .map(|(a, v)| {
                let diff = a - eig.values[j] * v;
                diff * diff
            })
            .sum::<f64>()
            .sqrt();
        residual = residual.max(norm);
    }
    let scale = eig
        .values
        .iter()
        .fold(0.0_f64, |m, v| m.max(v.abs()))
        .max(1e-12);
    let passed = residual <= 1e-2 * scale && orthonormal(eig, 1e-6);
    (passed, residual)
}

/// One component's solve: always dense (`k <= 100 < DENSE_EIG_LIMIT`). Returns the
/// projected, sign-pinnable coordinates (or `None` when the gate refuses them), the pivot
/// count used, and the peak residual. `width.dims()` is the reference's `dims`;
/// `dims_eff = min(dims, k)` is its `[:, ::-1][:, :min(dims, k)]` (`:197`).
fn solve_component(graph: &Neighbors, width: Width) -> (Option<EigBlock>, u32, f64) {
    let n = graph.len();
    let k = MAX_PIVOTS.min(n);
    let centered = Centered::new(pivot_hops(graph, k), n, k);
    let g = gram(&centered);
    let full = eigh(&g, k);
    let dims_eff = width.dims().min(k);
    let mut top = top_eigenpairs(&full, dims_eff);
    let (ok, peak_residual) = converged(&g, k, &top);
    canonicalise(&mut top);
    let projected = project(&centered, &top);
    (ok.then_some(projected), k as u32, peak_residual)
}

/// Runs the 2D Pivot MDS layout, byte for byte what it was before the 3D arm existed.
/// `Ok` even when some components were skipped — see [`MdsError::NothingSolved`] for the
/// only failure this returns.
pub fn run(topology: &Topology) -> Result<(Geometry, Vec<ComponentReport>), MdsError> {
    run_width(topology, Width::PivotMds2d)
}

/// `_mds_layout_3d` (`networkx_layouts.py:271-291`) at `SCALE`: the reference's three
/// coordinates, its cubic component lattice and its `_rescale_positions`. Below
/// [`MIN_NODES_3D`] it is `_random_layout` (`:283-284`), seeded by `seed`.
pub fn run_3d(
    topology: &Topology,
    seed: u32,
) -> Result<(Geometry, Vec<ComponentReport>), MdsError> {
    if topology.node_count() < MIN_NODES_3D {
        let geometry = random::run_seeded(topology, seed).map_err(|_| MdsError::RandomRefused)?;
        return Ok((geometry, Vec::new()));
    }
    run_width(topology, Width::PivotMds3d)
}

/// Both arms' pipeline, `layout::spectral`'s shape with Pivot MDS's own per-component solve.
fn run_width(
    topology: &Topology,
    width: Width,
) -> Result<(Geometry, Vec<ComponentReport>), MdsError> {
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
        let component = neighbors.component(members, &local_of);
        let (solved, pivots, peak_residual) = solve_component(&component, width);
        let ok = solved.is_some();
        if let Some(mut eig) = solved {
            pin_signs(&mut eig);
            scatter(&mut coords, members, &eig, width);
            any_solved = true;
        }
        reports.push(ComponentReport {
            min_index: members[0],
            size: members.len() as u32,
            pivots,
            solved: ok,
            peak_residual,
        });
    }

    if nothing_solved(&components, any_solved) {
        return Err(MdsError::NothingSolved);
    }
    if width.in_space() {
        pack_component_blocks_3d(&mut coords, &components);
        rescale_to_scale(&mut coords, width.dims(), SCALE);
    } else {
        pack_components(&mut coords, &components, width);
    }
    Ok((to_geometry(&coords, n, width), reports))
}

#[cfg(test)]
mod tests;
