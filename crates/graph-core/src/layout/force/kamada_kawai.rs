//! Kamada-Kawai (`layout.force.kamada_kawai`), written from the prose spec
//! `docs/layouts/layout.force.kamada_kawai.md` and the paper (Kamada and Kawai,
//! Information Processing Letters 31(1), 1989) only, per `docs/decisions/layouts-igraph.md`.
//! 2D, unweighted (every edge has length 1, so all-pairs distances are breadth-first).

#[cfg(test)]
mod tests;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::force::{SimpleGraph, simple_graph};
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// Parameters SciGraphs leaves at igraph's defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KkParams {
    /// Maximum single-vertex moves; `None` means `50 * n`.
    pub maxiter: Option<u32>,
    /// Stop when the largest squared gradient norm falls below this; 0 never stops early.
    pub epsilon: f64,
    /// Spring strength constant; `None` means `n`.
    pub kkconst: Option<f64>,
}

impl Default for KkParams {
    fn default() -> Self {
        Self {
            maxiter: None,
            epsilon: 0.0,
            kkconst: None,
        }
    }
}

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

impl Stage for KamadaKawai {
    type Params = KkParams;
    const ID: &'static str = "layout.force.kamada_kawai";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let n = topology.node_count() as usize;
        let mut pos = circle_start(n);
        if n > 1 {
            let springs = Springs::new(&simple_graph(topology), n, params);
            descend(&mut pos, &springs, params);
        }
        if pos.iter().flatten().any(|v| !v.is_finite()) {
            return Err(StageError::NonFinite { column: "node.x" });
        }
        Ok(Geometry::planar(
            NodeGeometry::Point {
                x: pos.iter().map(|p| p[0] as f32).collect(),
                y: pos.iter().map(|p| p[1] as f32).collect(),
            },
            EdgeGeometry::Line,
            Vec::new(),
        ))
    }
}

/// Vertices on a circle of radius `0.36 * sqrt(n)` (the spec's empirical start radius).
fn circle_start(n: usize) -> Vec<[f64; 2]> {
    let radius = 0.36 * libm::sqrt(n as f64);
    (0..n)
        .map(|i| {
            let angle = 2.0 * core::f64::consts::PI * i as f64 / n as f64;
            [radius * libm::cos(angle), radius * libm::sin(angle)]
        })
        .collect()
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
            length_per_hop: libm::sqrt(n as f64) / d_max,
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
fn pull(pos: &[[f64; 2]], springs: &Springs, m: usize, i: usize) -> [f64; 2] {
    let (k, l) = springs.spring(m, i);
    let delta = [pos[m][0] - pos[i][0], pos[m][1] - pos[i][1]];
    let r = libm::sqrt(delta[0] * delta[0] + delta[1] * delta[1]);
    let shrink = if r > 0.0 { l / r } else { 0.0 };
    [
        k * (delta[0] - shrink * delta[0]),
        k * (delta[1] - shrink * delta[1]),
    ]
}

fn full_gradient(pos: &[[f64; 2]], springs: &Springs, m: usize) -> [f64; 2] {
    let mut g = [0.0; 2];
    for i in (0..springs.n).filter(|&i| i != m) {
        let p = pull(pos, springs, m, i);
        g = [g[0] + p[0], g[1] + p[1]];
    }
    g
}

/// The Newton step `H^-1 g` for vertex `m` alone; zero at equilibrium or a singular block.
fn newton_step(pos: &[[f64; 2]], springs: &Springs, m: usize, g: [f64; 2]) -> [f64; 2] {
    if g[0] * g[0] + g[1] * g[1] < KK_EPS * KK_EPS {
        return [0.0; 2];
    }
    let (mut a, mut b, mut c) = (0.0, 0.0, 0.0);
    for i in (0..springs.n).filter(|&i| i != m) {
        let (k, l) = springs.spring(m, i);
        let (dx, dy) = (pos[m][0] - pos[i][0], pos[m][1] - pos[i][1]);
        let r = libm::sqrt(dx * dx + dy * dy);
        if r == 0.0 {
            (a, c) = (a + k, c + k);
            continue;
        }
        let r3 = r * r * r;
        a += k * (1.0 - l * dy * dy / r3);
        c += k * (1.0 - l * dx * dx / r3);
        b += k * l * dx * dy / r3;
    }
    let det = a * c - b * b;
    if det == 0.0 || !det.is_finite() {
        return [0.0; 2];
    }
    [(c * g[0] - b * g[1]) / det, (a * g[1] - b * g[0]) / det]
}

fn descend(pos: &mut [[f64; 2]], springs: &Springs, params: &KkParams) {
    let n = springs.n;
    let mut grad: Vec<[f64; 2]> = (0..n).map(|m| full_gradient(pos, springs, m)).collect();
    let maxiter = params.maxiter.unwrap_or(50 * n as u32);
    for _ in 0..maxiter {
        let (m, worst) = grad.iter().enumerate().fold((0, -1.0), |best, (i, g)| {
            let norm = g[0] * g[0] + g[1] * g[1];
            if norm > best.1 { (i, norm) } else { best }
        });
        if worst < params.epsilon {
            break;
        }
        let step = newton_step(pos, springs, m, grad[m]);
        for i in (0..n).filter(|&i| i != m) {
            let old = pull(pos, springs, i, m);
            grad[i] = [grad[i][0] - old[0], grad[i][1] - old[1]];
        }
        pos[m] = [pos[m][0] - step[0], pos[m][1] - step[1]];
        for i in (0..n).filter(|&i| i != m) {
            let new = pull(pos, springs, i, m);
            grad[i] = [grad[i][0] + new[0], grad[i][1] + new[1]];
        }
        grad[m] = full_gradient(pos, springs, m);
    }
}
