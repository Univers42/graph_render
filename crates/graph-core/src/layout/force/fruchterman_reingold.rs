//! Fruchterman-Reingold (`layout.force.fruchterman_reingold`), written from the prose spec
//! `docs/layouts/layout.force.fruchterman_reingold.md` and the paper (Fruchterman and
//! Reingold, Software: Practice and Experience 21(11), 1991) only, per
//! `docs/decisions/layouts-igraph.md`. 2D, dense, unweighted; the spec's grid variant
//! (n > 1000) is not implemented, so past that size this is the same O(n^2) loop.

#[cfg(test)]
mod tests;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::force::{SimpleGraph, simple_graph};
use crate::rng::{Mulberry32, jiggle};
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// Parameters SciGraphs leaves at igraph's defaults (`niter` 500, `start_temp` sqrt(n)/10).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrParams {
    /// Iteration count; the loop always runs exactly this many.
    pub niter: u32,
    /// Initial temperature; `None` means `sqrt(n) / 10`.
    pub start_temp: Option<f64>,
    /// Seeds the start box and the tie-breaking noise (D5: no global generator).
    pub seed: u32,
}

impl Default for FrParams {
    fn default() -> Self {
        Self {
            niter: 500,
            start_temp: None,
            seed: 0,
        }
    }
}

/// Node count past which the dense loop is no longer usable; see the registry entry.
pub const FR_CEILING: u64 = 2_000;

/// Amplitude of the coincident-pair fix and the per-move noise, per axis.
const NOISE: f64 = 1e-9;

/// Fruchterman-Reingold layout stage.
///
/// Ponytail: chaotic like every force layout (one added node is a different picture), and
/// the noise is our counter hash, not igraph's generator, so coordinates never match
/// igraph's; the differential compares layout stress, not positions. Above 1000 nodes
/// igraph switches to a stale-grid approximation that this port does not have, so the two
/// differ most exactly where igraph is least exact.
pub struct FruchtermanReingold;

impl Stage for FruchtermanReingold {
    type Params = FrParams;
    const ID: &'static str = "layout.force.fruchterman_reingold";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let n = topology.node_count() as usize;
        let graph = simple_graph(topology);
        let mut pos = start_positions(n, params.seed);
        let start_temp = params.start_temp.unwrap_or(sqrt(n as f64) / 10.0);
        let far = if is_connected(n, &graph) {
            None
        } else {
            Some(n as f64 * sqrt(n as f64))
        };
        let mut temp = start_temp;
        let mut disp = vec![[0.0; 2]; n];
        for iter in 0..params.niter {
            disp.fill([0.0; 2]);
            repel(&pos, far, (params.seed, iter), &mut disp);
            attract(&graph, &pos, &mut disp);
            step(&mut pos, &disp, temp, (params.seed, iter));
            temp -= start_temp / f64::from(params.niter);
        }
        if pos.iter().flatten().any(|v| !v.is_finite()) {
            return Err(StageError::NonFinite { column: "node.x" });
        }
        Ok(Geometry {
            nodes: NodeGeometry::Point {
                x: pos.iter().map(|p| p[0] as f32).collect(),
                y: pos.iter().map(|p| p[1] as f32).collect(),
            },
            edges: EdgeGeometry::Line,
            notes: Vec::new(),
        })
    }
}

pub(super) fn sqrt(v: f64) -> f64 {
    libm::sqrt(v)
}

/// Uniform in the square of side sqrt(n) centred on the origin.
pub(super) fn start_positions(n: usize, seed: u32) -> Vec<[f64; 2]> {
    let side = sqrt(n as f64);
    let mut rng = Mulberry32::new(seed);
    (0..n)
        .map(|_| {
            let x = (rng.next_f64() - 0.5) * side;
            [x, (rng.next_f64() - 0.5) * side]
        })
        .collect()
}

/// Weak connectivity by union-find over the simple edges (union by smaller index, so the
/// result does not depend on anything but edge order).
fn is_connected(n: usize, graph: &SimpleGraph) -> bool {
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(parent: &mut [usize], mut v: usize) -> usize {
        while parent[v] != v {
            parent[v] = parent[parent[v]];
            v = parent[v];
        }
        v
    }
    let mut components = n;
    for (&a, &b) in graph.lo.iter().zip(&graph.hi) {
        let (ra, rb) = (root(&mut parent, a as usize), root(&mut parent, b as usize));
        if ra != rb {
            parent[ra.max(rb)] = ra.min(rb);
            components -= 1;
        }
    }
    components <= 1
}

/// Pair repulsion `delta / r^2`, with the linear pull `-delta * r / far` on disconnected
/// graphs. A coincident pair is separated by a hash nudge, never divided by zero (D9).
fn repel(pos: &[[f64; 2]], far: Option<f64>, key: (u32, u32), disp: &mut [[f64; 2]]) {
    for v in 0..pos.len() {
        for u in v + 1..pos.len() {
            let mut delta = [pos[v][0] - pos[u][0], pos[v][1] - pos[u][1]];
            let mut r2 = delta[0] * delta[0] + delta[1] * delta[1];
            if r2 == 0.0 {
                delta = coincident_nudge(key, (v as u32, u as u32));
                r2 = delta[0] * delta[0] + delta[1] * delta[1];
            }
            let scale = match far {
                None => 1.0 / r2,
                Some(c) => (c - r2 * sqrt(r2)) / (r2 * c),
            };
            for a in 0..2 {
                disp[v][a] += delta[a] * scale;
                disp[u][a] -= delta[a] * scale;
            }
        }
    }
}

fn coincident_nudge(key: (u32, u32), pair: (u32, u32)) -> [f64; 2] {
    let amp = NOISE * 2e6;
    let d = [
        jiggle(key.0, key.1, 2, pair) * amp,
        jiggle(key.0, key.1, 3, pair) * amp,
    ];
    if d == [0.0, 0.0] { [NOISE, 0.0] } else { d }
}

/// Edge pull of magnitude `r^2` (unit weight): `delta * |delta|`.
fn attract(graph: &SimpleGraph, pos: &[[f64; 2]], disp: &mut [[f64; 2]]) {
    for (&a, &b) in graph.lo.iter().zip(&graph.hi) {
        let (v, u) = (a as usize, b as usize);
        let delta = [pos[v][0] - pos[u][0], pos[v][1] - pos[u][1]];
        let len = sqrt(delta[0] * delta[0] + delta[1] * delta[1]);
        for k in 0..2 {
            disp[v][k] -= delta[k] * len;
            disp[u][k] += delta[k] * len;
        }
    }
}

/// Moves every vertex by its displacement capped at `temp`, after a tiny hash noise so a
/// perfectly balanced vertex still leaves a saddle.
fn step(pos: &mut [[f64; 2]], disp: &[[f64; 2]], temp: f64, key: (u32, u32)) {
    for (v, d) in disp.iter().enumerate() {
        let vv = (v as u32, v as u32);
        let mut d = [
            d[0] + jiggle(key.0, key.1, 0, vv) * NOISE * 2e6,
            d[1] + jiggle(key.0, key.1, 1, vv) * NOISE * 2e6,
        ];
        let len = sqrt(d[0] * d[0] + d[1] * d[1]);
        if len > temp {
            d = [d[0] / len * temp, d[1] / len * temp];
        }
        if len > 0.0 {
            pos[v][0] += d[0];
            pos[v][1] += d[1];
        }
    }
}
