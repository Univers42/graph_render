//! Graphopt (`layout.force.graphopt`), written from `docs/layouts/layout.force.graphopt.md`
//! only, per `docs/decisions/layouts-igraph.md`. 2D; Coulomb repulsion plus Hooke springs,
//! synchronous steps, no cooling and no convergence test.

#[cfg(test)]
mod tests;

use super::fruchterman_reingold::sqrt;
use crate::index::Topology;
use crate::layout::Geometry;
use crate::rng::Mulberry32;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// Uniform in the square of side `sqrt(n)` centred on the origin.
///
/// Graphopt's own 2D start, kept local rather than borrowed from
/// [`super::fruchterman_reingold`]: that one is now `dim`-parameterised and hands back
/// three-wide positions, and this layout is 2D-only, so sharing it would mean reading the
/// third slot this kernel never wrote.
fn square_start(n: usize, seed: u32) -> Vec<[f64; 2]> {
    let side = sqrt(n as f64);
    let mut rng = Mulberry32::new(seed);
    (0..n)
        .map(|_| {
            let x = (rng.next_f64() - 0.5) * side;
            [x, (rng.next_f64() - 0.5) * side]
        })
        .collect()
}

/// Parameters, all at the values SciGraphs takes from the original graphopt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphoptParams {
    /// Step count; the loop runs exactly this many.
    pub niter: u32,
    /// Charge of every node; zero disables repulsion.
    pub node_charge: f64,
    /// Divisor from force to displacement.
    pub node_mass: f64,
    /// Spring rest length.
    pub spring_length: f64,
    /// Hooke constant.
    pub spring_constant: f64,
    /// Per-axis displacement cap.
    pub max_sa_movement: f64,
    /// Seeds the start positions (D5).
    pub seed: u32,
}

impl Default for GraphoptParams {
    fn default() -> Self {
        Self {
            niter: 500,
            node_charge: 0.001,
            node_mass: 30.0,
            spring_length: 0.0,
            spring_constant: 1.0,
            max_sa_movement: 5.0,
            seed: 0,
        }
    }
}

/// Node count past which the O(n^2) pair loop is no longer usable.
pub const GRAPHOPT_CEILING: u64 = 2_000;

const COULOMB: f64 = 8_987_500_000.0;

/// Pairs at least this far apart do not repel (the original's hard cut-off).
const CUTOFF: f64 = 500.0;

/// Graphopt layout stage.
///
/// Ponytail: the 500-unit repulsion cut-off is the original's heuristic; a layout much
/// wider than that stops repelling distant clusters and has no escape hatch. There is no
/// cooling, so the run may oscillate at the cap instead of settling. The start is our
/// seeded square, not igraph's random layout, so coordinates never match igraph's.
pub struct Graphopt;

impl Stage for Graphopt {
    type Params = GraphoptParams;
    const ID: &'static str = "layout.force.graphopt";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let n = topology.node_count() as usize;
        let mut pos = square_start(n, params.seed);
        let edges = topology.edges();
        let pairs: Vec<(usize, usize)> = (0..topology.edge_count() as usize)
            .map(|e| (edges.source[e] as usize, edges.target[e] as usize))
            .collect();
        let mut force = vec![[0.0; 2]; n];
        for _ in 0..params.niter {
            force.fill([0.0; 2]);
            if params.node_charge != 0.0 {
                repel(&pos, params.node_charge, &mut force);
            }
            springs(&pos, &pairs, params, &mut force);
            let cap = params.max_sa_movement;
            for (p, f) in pos.iter_mut().zip(&force) {
                p[0] += (f[0] / params.node_mass).clamp(-cap, cap);
                p[1] += (f[1] / params.node_mass).clamp(-cap, cap);
            }
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

/// Coulomb push `C q^2 / d^2` along the joining line for every pair with 0 < d < 500.
fn repel(pos: &[[f64; 2]], charge: f64, force: &mut [[f64; 2]]) {
    for v in 0..pos.len() {
        for u in v + 1..pos.len() {
            let delta = [pos[v][0] - pos[u][0], pos[v][1] - pos[u][1]];
            let d = sqrt(delta[0] * delta[0] + delta[1] * delta[1]);
            if d == 0.0 || d >= CUTOFF {
                continue;
            }
            let magnitude = COULOMB * charge * charge / (d * d);
            for a in 0..2 {
                let part = magnitude * delta[a] / d;
                force[v][a] += part;
                force[u][a] -= part;
            }
        }
    }
}

/// Each endpoint feels `k |d - L| / 2` toward (d > L) or away from (d < L) the other.
fn springs(
    pos: &[[f64; 2]],
    pairs: &[(usize, usize)],
    params: &GraphoptParams,
    force: &mut [[f64; 2]],
) {
    for &(v, u) in pairs {
        let delta = [pos[v][0] - pos[u][0], pos[v][1] - pos[u][1]];
        let d = sqrt(delta[0] * delta[0] + delta[1] * delta[1]);
        if d == 0.0 || d == params.spring_length {
            continue;
        }
        let half = params.spring_constant * (d - params.spring_length) / 2.0;
        for a in 0..2 {
            let part = half * delta[a] / d;
            force[v][a] -= part;
            force[u][a] += part;
        }
    }
}
