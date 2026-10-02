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
    DIMS, Neighbors, find_components, nothing_solved, pack_components, scatter,
    simple_neighbors, to_geometry,
};

mod matrix;
use matrix::{double_center, gram, project};

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

/// Hop counts from one node at a time, over the whole graph's adjacency in dense indices,
/// so a walk needs no local index map. Both buffers are allocated once per run: the queue
/// ends holding exactly the nodes the last walk reached, which is what the next one resets.
struct Bfs<'a> {
    neighbors: &'a Neighbors,
    hops: Vec<u32>,
    queue: Vec<u32>,
}

impl<'a> Bfs<'a> {
    fn new(neighbors: &'a Neighbors) -> Self {
        Self {
            neighbors,
            hops: vec![u32::MAX; neighbors.len()],
            queue: Vec::new(),
        }
    }

    /// Hop count from `start` to every node of its component, into `hops`; every other
    /// entry is `u32::MAX`. A component is connected by construction, so the reference's
    /// `d[~np.isfinite(d)] = 0.0` branch never triggers on this caller and is not ported.
    fn walk(&mut self, start: u32) {
        for &v in &self.queue {
            self.hops[v as usize] = u32::MAX;
        }
        self.queue.clear();
        self.queue.push(start);
        self.hops[start as usize] = 0;
        let mut head = 0;
        while let Some(&v) = self.queue.get(head) {
            head += 1;
            let next = self.hops[v as usize] + 1;
            for &w in self.neighbors.row(v) {
                if self.hops[w as usize] == u32::MAX {
                    self.hops[w as usize] = next;
                    self.queue.push(w);
                }
            }
        }
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
fn pivot_distances(bfs: &mut Bfs, members: &[u32], k: usize) -> Vec<f64> {
    let n = members.len();
    let mut dist = vec![0.0; n * k];
    let mut covered = vec![f64::INFINITY; n];
    let mut chosen = 0usize;
    for j in 0..k {
        bfs.walk(members[chosen]);
        for (i, &node) in members.iter().enumerate() {
            let d = f64::from(bfs.hops[node as usize]);
            dist[i * k + j] = d;
            covered[i] = covered[i].min(d);
        }
        covered[chosen] = -1.0;
        chosen = argmax(&covered);
    }
    dist
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
fn solve_component(bfs: &mut Bfs, members: &[u32]) -> (Option<EigBlock>, u32) {
    let n = members.len();
    let k = MAX_PIVOTS.min(n);
    let mut dist = pivot_distances(bfs, members, k);
    double_center(&mut dist, n, k);
    let g = gram(&dist, k);
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
    let mut bfs = Bfs::new(&neighbors);
    let mut coords = vec![0.0_f64; n * DIMS];
    let mut reports = Vec::new();
    let mut any_solved = false;

    for members in &components {
        if members.len() < 2 {
            continue;
        }
        let (solved, pivots) = solve_component(&mut bfs, members);
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
