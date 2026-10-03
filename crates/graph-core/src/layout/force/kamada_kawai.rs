//! Kamada-Kawai (`layout.force.kamada_kawai`), written from the prose spec
//! `docs/layouts/layout.force.kamada_kawai.md` and the paper (Kamada and Kawai,
//! Information Processing Letters 31(1), 1989) only, per `docs/decisions/layouts-igraph.md`.
//! 2D, unweighted (every edge has length 1, so all-pairs distances are breadth-first).

#[cfg(test)]
mod tests;

mod solve;
mod start;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::force::{SimpleGraph, simple_graph};
use crate::stage::{Stage, StageError};

/// Parameters SciGraphs leaves at igraph's defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KkParams {
    /// Maximum single-vertex moves; `None` means `50 * n`.
    pub maxiter: Option<u32>,
    /// Stop when the largest squared gradient norm falls below this; 0 never stops early.
    pub epsilon: f64,
    /// Spring strength constant; `None` means `n`.
    pub kkconst: Option<f64>,
    /// How many coordinates every node carries: `2`, or `3` for
    /// `layout.force.kamada_kawai.3d` — one kernel, the dimension a parameter.
    pub dim: usize,
}

impl Default for KkParams {
    fn default() -> Self {
        Self {
            maxiter: None,
            epsilon: 0.0,
            kkconst: None,
            dim: 2,
        }
    }
}

/// The widest point this port keeps; see
/// [`super::fruchterman_reingold::MAX_DIM`].
const MAX_DIM: usize = 3;

/// Node count past which the O(n^2) matrices and 50 n^2 moves are no longer usable.
pub const KK_CEILING: u64 = 2_000;

/// Gradients below this norm are treated as equilibrium: the step is zero.
const KK_EPS: f64 = 1e-13;

/// Kamada-Kawai layout stage. Fully deterministic: the start is a circle, no generator.
///
/// Ponytail: Newton descent on one vertex at a time finds a local minimum of the spring
/// energy, not the global one, so a graph with a folded start can settle folded. An
/// edgeless graph with n >= 2 has no finite distance; igraph divides by zero there, this
/// port takes every distance as 1 instead, so it draws a regular polygon-like cloud.
pub struct KamadaKawai;

/// SciGraphs' `IGRAPH_KK` asks igraph for `dim = 3` (`igraph_layouts.py:99`); this is the
/// same kernel at 3.
pub const ID_3D: &str = "layout.force.kamada_kawai.3d";

impl Stage for KamadaKawai {
    type Params = KkParams;
    const ID: &'static str = "layout.force.kamada_kawai";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        run_at_dim(topology, params, params.dim)
    }
}

/// The 3D arm: [`Stage::run`] with `dim` forced to 3. See
/// [`super::fruchterman_reingold::run_3d`] for why it is a function and not a second
/// `Stage` impl.
pub fn run_3d(topology: &Topology, params: &KkParams) -> Result<Geometry, StageError> {
    run_at_dim(topology, params, MAX_DIM)
}

fn run_at_dim(topology: &Topology, params: &KkParams, dim: usize) -> Result<Geometry, StageError> {
    if !(2..=MAX_DIM).contains(&dim) {
        return Err(StageError::Param {
            name: "dim",
            rule: "2 or 3 coordinates per node",
        });
    }
    let n = topology.node_count() as usize;
    let mut pos = start::circle_start(n, dim);
    if n > 1 {
        let springs = Springs::new(&simple_graph(topology), n, params);
        descend(&mut pos, &springs, params, dim);
    }
    if pos.iter().flatten().take(dim).any(|v| !v.is_finite()) {
        return Err(StageError::NonFinite { column: "node.x" });
    }
    let column = |a: usize| pos.iter().map(|p| p[a] as f32).collect();
    Ok(Geometry::points(dim, column(0), column(1), column(2)))
}

/// `sum of squares` over the live axes, ascending — the same order the 2D arm summed
/// `dx*dx + dy*dy` in, so the 2D bits do not move.
fn norm2(v: &[f64; MAX_DIM], dim: usize) -> f64 {
    let mut sum = 0.0;
    for x in v.iter().take(dim) {
        sum += x * x;
    }
    sum
}

/// Hop distances and the two per-pair constants derived from them.
struct Springs {
    n: usize,
    dist: Vec<f64>,
    length_per_hop: f64,
    strength: f64,
}

impl Springs {
    fn new(graph: &SimpleGraph, n: usize, params: &KkParams) -> Self {
        let mut dist = all_pairs_hops(graph, n);
        let finite_max = dist
            .iter()
            .copied()
            .filter(|d| d.is_finite())
            .fold(0.0, f64::max);
        let d_max = if finite_max > 0.0 { finite_max } else { 1.0 };
        for d in dist.iter_mut().filter(|d| !d.is_finite() || **d == 0.0) {
            *d = d_max;
        }
        Springs {
            n,
            dist,
            length_per_hop: f64::sqrt(n as f64) / d_max,
            strength: params.kkconst.unwrap_or(n as f64),
        }
    }

    /// Spring `(k, l)` between `i` and `j`.
    fn spring(&self, i: usize, j: usize) -> (f64, f64) {
        let d = self.dist[i * self.n + j];
        (self.strength / (d * d), self.length_per_hop * d)
    }
}

/// Breadth-first hop counts from every vertex; unreachable is infinity.
fn all_pairs_hops(graph: &SimpleGraph, n: usize) -> Vec<f64> {
    let mut dist = vec![f64::INFINITY; n * n];
    let mut queue = Vec::with_capacity(n);
    for s in 0..n {
        queue.clear();
        queue.push(s);
        dist[s * n + s] = 0.0;
        let mut head = 0;
        while head < queue.len() {
            let v = queue[head];
            head += 1;
            for &e in graph.rows.row(v as u32) {
                let e = e as usize;
                let u = if graph.lo[e] as usize == v {
                    graph.hi[e]
                } else {
                    graph.lo[e]
                } as usize;
                if dist[s * n + u].is_infinite() {
                    dist[s * n + u] = dist[s * n + v] + 1.0;
                    queue.push(u);
                }
            }
        }
    }
    dist
}

/// Gradient contribution on `m` of its spring to `i`; a coincident pair contributes its
/// pure stretch term only (the direction is undefined, D9).
fn pull(
    pos: &[[f64; MAX_DIM]],
    springs: &Springs,
    m: usize,
    i: usize,
    dim: usize,
) -> [f64; MAX_DIM] {
    let (k, l) = springs.spring(m, i);
    let mut delta = [0.0; MAX_DIM];
    for a in 0..dim {
        delta[a] = pos[m][a] - pos[i][a];
    }
    let r = f64::sqrt(norm2(&delta, dim));
    let shrink = if r > 0.0 { l / r } else { 0.0 };
    let mut out = [0.0; MAX_DIM];
    for a in 0..dim {
        out[a] = k * (delta[a] - shrink * delta[a]);
    }
    out
}

fn full_gradient(
    pos: &[[f64; MAX_DIM]],
    springs: &Springs,
    m: usize,
    dim: usize,
) -> [f64; MAX_DIM] {
    let mut g = [0.0; MAX_DIM];
    for i in (0..springs.n).filter(|&i| i != m) {
        let p = pull(pos, springs, m, i, dim);
        for a in 0..dim {
            g[a] += p[a];
        }
    }
    g
}

/// The Newton step `H^-1 g` for vertex `m` alone; zero at equilibrium or a singular block.
///
/// `h` is the symmetric `dim x dim` Hessian, upper triangle only (`h[a][b]`, `a <= b`),
/// accumulated in ascending spring order so the sum is the same one the 2D arm always
/// made. Solving it is the one place the two dimensions genuinely differ: at 2 the
/// adjugate is two terms, at 3 it is nine, so [`solve`] carries both rather than
/// pretending one formula covers both.
fn newton_step(
    pos: &[[f64; MAX_DIM]],
    springs: &Springs,
    m: usize,
    g: [f64; MAX_DIM],
    dim: usize,
) -> [f64; MAX_DIM] {
    if norm2(&g, dim) < KK_EPS * KK_EPS {
        return [0.0; MAX_DIM];
    }
    let mut h = [[0.0; MAX_DIM]; MAX_DIM];
    for i in (0..springs.n).filter(|&i| i != m) {
        let (k, l) = springs.spring(m, i);
        let mut delta = [0.0; MAX_DIM];
        for a in 0..dim {
            delta[a] = pos[m][a] - pos[i][a];
        }
        let r = f64::sqrt(norm2(&delta, dim));
        if r == 0.0 {
            // A coincident pair contributes its pure stretch term: `k` on the diagonal,
            // nothing off it, the 2D arm's own `(a, c) = (a + k, c + k)`.
            for d in h.iter_mut().take(dim) {
                d[0] += k;
            }
            continue;
        }
        let r3 = r * r * r;
        for a in 0..dim {
            h[a][a] += k * (1.0 - l * delta[a] * delta[a] / r3);
            for b in (a + 1)..dim {
                h[a][b] += k * l * delta[a] * delta[b] / r3;
            }
        }
    }
    solve::solve(&h, &g, dim)
}

fn descend(pos: &mut [[f64; MAX_DIM]], springs: &Springs, params: &KkParams, dim: usize) {
    let n = springs.n;
    let mut grad: Vec<[f64; MAX_DIM]> = (0..n)
        .map(|m| full_gradient(pos, springs, m, dim))
        .collect();
    let maxiter = params.maxiter.unwrap_or(50 * n as u32);
    for _ in 0..maxiter {
        let (m, worst) = grad.iter().enumerate().fold((0, -1.0), |best, (i, g)| {
            let norm = norm2(g, dim);
            if norm > best.1 { (i, norm) } else { best }
        });
        if worst < params.epsilon {
            break;
        }
        let step = newton_step(pos, springs, m, grad[m], dim);
        for i in (0..n).filter(|&i| i != m) {
            let old = pull(pos, springs, i, m, dim);
            for a in 0..dim {
                grad[i][a] -= old[a];
            }
        }
        for a in 0..dim {
            pos[m][a] -= step[a];
        }
        for i in (0..n).filter(|&i| i != m) {
            let new = pull(pos, springs, i, m, dim);
            for a in 0..dim {
                grad[i][a] += new[a];
            }
        }
        grad[m] = full_gradient(pos, springs, m, dim);
    }
}
