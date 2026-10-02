//! `layout.mds.pivot` (`docs/decisions/eigensolver.md`): pivot selection by a
//! farthest-point BFS heuristic, double-centered squared hop-distances, then the
//! largest `dims = 2` eigenpairs of the `k x k` `Cᵀ C` Gram matrix
//! (`k = min(100, n_c)`), projected back to `n_c` points and sign-pinned. Ports
//! `_pivot_mds_coordinates`/`_pivot_mds_component_coordinates`
//! (`networkx_layouts.py:166-216`). Components, adjacency (C6) and 2D packing are
//! `layout::spectral`'s, reused here through its `pub(crate)` items.
//!
//! **Scope**: as `layout::spectral` — no [`crate::stage::Stage`], no external `scale`;
//! registered through [`super::spectral_stage`].
//!
//! **C12**: identical shape to `layout::spectral`'s — a component that fails the small
//! eigensolve's residual/orthonormality gate is skipped ((0, 0), then its packing
//! cell); [`MdsError::NothingSolved`] only when something was attempted and nothing
//! solved (`nothing_solved`, shared with `spectral`).

use crate::index::Topology;
use crate::linalg::dense_sym::eigh;
use crate::linalg::{EigBlock, orthonormal, pin_signs, residual_converged};

use super::Geometry;
use super::spectral::{
    DIMS, Neighbors, find_components, local_positions, nothing_solved, pack_components, scatter,
    simple_neighbors, to_geometry,
};

mod matrix;
use matrix::{Centered, gram, project};

/// `_MDS_PIVOTS` (reference constant).
pub const MAX_PIVOTS: usize = 100;

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
}

/// Why [`run`] produced nothing at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdsError {
    /// No component's solve passed the gate (C12: an explicit refusal, never a silent
    /// random fallback).
    NothingSolved,
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
/// just as well to Pivot MDS's own dense solve).
fn converged(gram: &[f64], k: usize, eig: &EigBlock) -> bool {
    let matvec = |x: &[f64], y: &mut [f64]| {
        for row in 0..k {
            y[row] = (0..k).map(|col| gram[row * k + col] * x[col]).sum();
        }
    };
    residual_converged(matvec, eig, 1e-2) && orthonormal(eig, 1e-6)
}

/// One component's solve: always dense (`k <= 100 < DENSE_EIG_LIMIT`). Returns the
/// projected, sign-pinnable coordinates (or `None` when the gate refuses them) and the
/// pivot count used.
fn solve_component(graph: &Neighbors) -> (Option<EigBlock>, u32) {
    let n = graph.len();
    let k = MAX_PIVOTS.min(n);
    let centered = Centered::new(pivot_hops(graph, k), n, k);
    let g = gram(&centered);
    let full = eigh(&g, k);
    let dims_eff = DIMS.min(k);
    let top = top_eigenpairs(&full, dims_eff);
    let ok = converged(&g, k, &top);
    let projected = project(&centered, &top);
    (ok.then_some(projected), k as u32)
}

/// Runs the Pivot MDS layout. `Ok` even when some components were skipped — see
/// [`MdsError::NothingSolved`] for the only failure this returns.
pub fn run(topology: &Topology) -> Result<(Geometry, Vec<ComponentReport>), MdsError> {
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
        let (solved, pivots) = solve_component(&neighbors.component(members, &local_of));
        let ok = solved.is_some();
        if let Some(mut eig) = solved {
            pin_signs(&mut eig);
            scatter(&mut coords, members, &eig);
            any_solved = true;
        }
        reports.push(ComponentReport {
            min_index: members[0],
            size: members.len() as u32,
            pivots,
            solved: ok,
        });
    }

    if nothing_solved(&components, any_solved) {
        return Err(MdsError::NothingSolved);
    }
    pack_components(&mut coords, &components);
    Ok((to_geometry(&coords, n), reports))
}

#[cfg(test)]
mod tests;
