//! `layout.pivot_mds` (`docs/decisions/eigensolver.md`): pivot selection by a
//! farthest-point BFS heuristic, double-centered squared hop-distances, then the
//! largest `dims = 2` eigenpairs of the `k x k` `Cᵀ C` Gram matrix
//! (`k = min(100, n_c)`), projected back to `n_c` points and sign-pinned. Ports
//! `_pivot_mds_coordinates`/`_pivot_mds_component_coordinates`
//! (`networkx_layouts.py:166-216`). Components, adjacency (C6) and 2D packing are
//! `layout::spectral`'s, reused here through its `pub(crate)` items.
//!
//! **Scope**: as `layout::spectral` — no [`crate::stage::Stage`], no external `scale`.
//!
//! **C12**: identical shape to `layout::spectral`'s — a component that fails the small
//! eigensolve's residual/orthonormality gate is skipped ((0, 0), then its packing
//! cell); [`MdsError::NothingSolved`] only when something was attempted and nothing
//! solved (`nothing_solved`, shared with `spectral`).

use std::collections::VecDeque;

use crate::index::Topology;
use crate::linalg::dense_sym::eigh;
use crate::linalg::{EigBlock, orthonormal, pin_signs, residual_converged};

use super::Geometry;
use super::spectral::{
    DIMS, find_components, local_index_map, nothing_solved, pack_components, scatter,
    simple_neighbors, to_geometry,
};

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

/// A component's members and collapsed adjacency, addressed by local index. Pivot MDS
/// needs BFS hop counts rather than a Laplacian matvec, so this is its own small type,
/// not `spectral::ComponentGraph`.
struct ComponentGraph<'a> {
    members: &'a [u32],
    neighbors: &'a [Vec<u32>],
    local_of: Vec<u32>,
}

impl<'a> ComponentGraph<'a> {
    fn build(members: &'a [u32], neighbors: &'a [Vec<u32>], n: usize) -> Self {
        let local_of = local_index_map(members, n);
        Self {
            members,
            neighbors,
            local_of,
        }
    }

    fn size(&self) -> usize {
        self.members.len()
    }

    /// Hop count from local index `start` to every member, BFS over the collapsed
    /// undirected adjacency (C6). Every entry is reached: the component is connected by
    /// construction, so the reference's `d[~np.isfinite(d)] = 0.0` branch never
    /// triggers on this caller and is not ported.
    fn bfs_hops(&self, start: usize) -> Vec<f64> {
        let n = self.size();
        let mut hops = vec![u32::MAX; n];
        hops[start] = 0;
        let mut queue = VecDeque::new();
        queue.push_back(start);
        while let Some(v) = queue.pop_front() {
            for &w in &self.neighbors[self.members[v] as usize] {
                let lw = self.local_of[w as usize] as usize;
                if hops[lw] == u32::MAX {
                    hops[lw] = hops[v] + 1;
                    queue.push_back(lw);
                }
            }
        }
        hops.iter().map(|&h| h as f64).collect()
    }
}

/// `np.argmax`'s own tie rule: the first (lowest-index) occurrence of the maximum.
fn argmax(v: &[f64]) -> usize {
    let mut best = 0;
    for i in 1..v.len() {
        if v[i] > v[best] {
            best = i;
        }
    }
    best
}

/// The `n x k` hop-distance matrix, one column per pivot, chosen by the farthest-point
/// heuristic (`_pivot_mds_coordinates:176-185`): pivot 0 is local index 0; each next
/// pivot is the node currently farthest (by minimum hop count) from every pivot chosen
/// so far, ties won by the lowest index.
fn pivot_distances(graph: &ComponentGraph, k: usize) -> Vec<f64> {
    let n = graph.size();
    let mut dist = vec![0.0; n * k];
    let mut covered = vec![f64::INFINITY; n];
    let mut chosen = 0usize;
    for j in 0..k {
        let d = graph.bfs_hops(chosen);
        for i in 0..n {
            dist[i * k + j] = d[i];
        }
        for i in 0..n {
            covered[i] = covered[i].min(d[i]);
        }
        covered[chosen] = -1.0;
        chosen = argmax(&covered);
    }
    dist
}

/// Squares `dist` in place, then double-centers it (`_pivot_mds_coordinates:187-194`),
/// sequential sums throughout (D3).
fn double_center(dist: &mut [f64], n: usize, k: usize) {
    for v in dist.iter_mut() {
        *v *= *v;
    }
    let col_mean: Vec<f64> = (0..k)
        .map(|j| (0..n).map(|i| dist[i * k + j]).sum::<f64>() / n as f64)
        .collect();
    let row_mean: Vec<f64> = (0..n)
        .map(|i| (0..k).map(|j| dist[i * k + j]).sum::<f64>() / k as f64)
        .collect();
    let grand_mean = col_mean.iter().sum::<f64>() / k as f64;
    for i in 0..n {
        for j in 0..k {
            let centered = dist[i * k + j] - col_mean[j] - row_mean[i] + grand_mean;
            dist[i * k + j] = centered * -0.5;
        }
    }
}

/// `distᵀ dist`, the `k x k` Gram matrix Pivot MDS's eigensolve runs on.
fn gram(dist: &[f64], n: usize, k: usize) -> Vec<f64> {
    let mut g = vec![0.0; k * k];
    for p in 0..k {
        for q in 0..k {
            g[p * k + q] = (0..n).map(|i| dist[i * k + p] * dist[i * k + q]).sum();
        }
    }
    g
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

/// `dist @ vectors`: the `k`-dimensional eigenvectors projected back to `n_c` points
/// (`_pivot_mds_coordinates:198`) — sign-pinned on *this* result, not on `top` itself.
fn project(dist: &[f64], n: usize, k: usize, top: &EigBlock) -> EigBlock {
    let dims_eff = top.k;
    let mut vectors = vec![0.0; n * dims_eff];
    for d in 0..dims_eff {
        let v = top.column(d);
        for i in 0..n {
            vectors[d * n + i] = (0..k).map(|p| dist[i * k + p] * v[p]).sum();
        }
    }
    EigBlock {
        values: top.values.clone(),
        vectors,
        n,
        k: dims_eff,
    }
}

/// One component's solve: always dense (`k <= 100 < DENSE_EIG_LIMIT`). Returns the
/// projected, sign-pinnable coordinates (or `None` when the gate refuses them) and the
/// pivot count used.
fn solve_component(graph: &ComponentGraph) -> (Option<EigBlock>, u32) {
    let n = graph.size();
    let k = MAX_PIVOTS.min(n);
    let mut dist = pivot_distances(graph, k);
    double_center(&mut dist, n, k);
    let g = gram(&dist, n, k);
    let full = eigh(&g, k);
    let dims_eff = DIMS.min(k);
    let top = top_eigenpairs(&full, dims_eff);
    let ok = converged(&g, k, &top);
    let projected = project(&dist, n, k, &top);
    (ok.then_some(projected), k as u32)
}

/// Runs the Pivot MDS layout. `Ok` even when some components were skipped — see
/// [`MdsError::NothingSolved`] for the only failure this returns.
pub fn run(topology: &Topology) -> Result<(Geometry, Vec<ComponentReport>), MdsError> {
    let n = topology.node_count() as usize;
    let neighbors = simple_neighbors(topology);
    let components = find_components(&neighbors);
    let mut coords = vec![0.0_f64; n * DIMS];
    let mut reports = Vec::new();
    let mut any_solved = false;

    for members in &components {
        if members.len() < 2 {
            continue;
        }
        let graph = ComponentGraph::build(members, &neighbors, n);
        let (solved, pivots) = solve_component(&graph);
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
