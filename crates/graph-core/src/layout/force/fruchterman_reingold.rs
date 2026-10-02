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
/// Parameters SciGraphs leaves at igraph's defaults (`niter` 500, `start_temp` sqrt(n)/10).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrParams {
    /// Iteration count; the loop always runs exactly this many.
    pub niter: u32,
    /// Initial temperature; `None` means `sqrt(n) / 10`.
    pub start_temp: Option<f64>,
    /// Seeds the start box and the tie-breaking noise (D5: no global generator).
    pub seed: u32,
    /// How many coordinates every node carries: `2` for the 2D arm, `3` for
    /// `layout.force.fruchterman_reingold.3d`. The one kernel serves both, so a 3D
    /// force is the 2D force with `dz` in the difference (`fruchterman_reingold.c:556-651`
    /// — igraph writes the 3D loop out separately, and its third axis is the same term).
    pub dim: usize,
}

impl Default for FrParams {
    fn default() -> Self {
        Self {
            niter: 500,
            start_temp: None,
            seed: 0,
            dim: 2,
        }
    }
}

/// The widest point this port keeps. A position is always three slots wide and `dim`
/// says how many are live, so one kernel serves both dimensions and the 2D arm's third
/// slot is never read.
pub(super) const MAX_DIM: usize = 3;

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

/// SciGraphs' `IGRAPH_FR` asks igraph for `dim = 3` (`igraph_layouts.py:74`), and this is
/// the same kernel with `dim` set to 3: `id` plus the 3D suffix, over the same code.
pub const ID_3D: &str = "layout.force.fruchterman_reingold.3d";

impl Stage for FruchtermanReingold {
    type Params = FrParams;
    const ID: &'static str = "layout.force.fruchterman_reingold";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        run_at_dim(topology, params, params.dim)
    }
}

/// The 3D arm: [`Stage::run`] with `dim` forced to 3.
///
/// A separate entry point rather than a second `Stage` impl, so `dim` stays a
/// *parameter* of one kernel and the registry's function-pointer slot can carry it.
pub fn run_3d(topology: &Topology, params: &FrParams) -> Result<Geometry, StageError> {
    run_at_dim(topology, params, MAX_DIM)
}

/// `dim` is 2 or 3; anything else is refused rather than clamped, because a caller that
/// asked for four dimensions and silently got three would not know.
fn run_at_dim(topology: &Topology, params: &FrParams, dim: usize) -> Result<Geometry, StageError> {
    if !(2..=MAX_DIM).contains(&dim) {
        return Err(StageError::Param {
            name: "dim",
            rule: "2 or 3 coordinates per node",
        });
    }
    let n = topology.node_count() as usize;
    let graph = simple_graph(topology);
    let mut pos = start_positions(n, params.seed, dim);
    let start_temp = params.start_temp.unwrap_or(sqrt(n as f64) / 10.0);
    let far = if is_connected(n, &graph) {
        None
    } else {
        Some(n as f64 * sqrt(n as f64))
    };
    let mut temp = start_temp;
    let mut disp = vec![[0.0; MAX_DIM]; n];
    for iter in 0..params.niter {
        disp.fill([0.0; MAX_DIM]);
        repel(&pos, far, (params.seed, iter), dim, &mut disp);
        attract(&graph, &pos, dim, &mut disp);
        step(&mut pos, &disp, temp, (params.seed, iter), dim);
        temp -= start_temp / f64::from(params.niter);
    }
    if pos.iter().flatten().take(dim).any(|v| !v.is_finite()) {
        return Err(StageError::NonFinite { column: "node.x" });
    }
    let column = |a: usize| pos.iter().map(|p| p[a] as f32).collect();
    Ok(Geometry::points(dim, column(0), column(1), column(2)))
}

pub(super) fn sqrt(v: f64) -> f64 {
    libm::sqrt(v)
}

/// Uniform in the `dim`-cube of side `sqrt(n)` centred on the origin.
///
/// Ponytail: igraph's own 3D start is a *sphere* (`igraph_i_layout_random_bounded` draws
/// `dim = 3` inside a ball), this is a cube, so the two starts differ and the pictures do
/// from the first iteration. Failing input: none — the start is a start, and FR converges
/// from either. Direction: cosmetic. Escape hatch: `seed` plus `dim`.
pub(super) fn start_positions(n: usize, seed: u32, dim: usize) -> Vec<[f64; MAX_DIM]> {
    let side = sqrt(n as f64);
    let mut rng = Mulberry32::new(seed);
    (0..n)
        .map(|_| {
            let mut p = [0.0; MAX_DIM];
            for slot in p.iter_mut().take(dim) {
                *slot = (rng.next_f64() - 0.5) * side;
            }
            p
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
fn repel(
    pos: &[[f64; MAX_DIM]],
    far: Option<f64>,
    key: (u32, u32),
    dim: usize,
    disp: &mut [[f64; MAX_DIM]],
) {
    for v in 0..pos.len() {
        for u in v + 1..pos.len() {
            let mut delta = [0.0; MAX_DIM];
            for a in 0..dim {
                delta[a] = pos[v][a] - pos[u][a];
            }
            let mut r2 = norm2(&delta, dim);
            if r2 == 0.0 {
                delta = coincident_nudge(key, (v as u32, u as u32), dim);
                r2 = norm2(&delta, dim);
            }
            let scale = match far {
                None => 1.0 / r2,
                Some(c) => (c - r2 * sqrt(r2)) / (r2 * c),
            };
            for a in 0..dim {
                disp[v][a] += delta[a] * scale;
                disp[u][a] -= delta[a] * scale;
            }
        }
    }
}

/// `sum of squares` over the live axes, in ascending axis order — so at `dim` 2 this is
/// the `dx*dx + dy*dy` the 2D arm always computed, in the same order and the same bits.
///
/// `zip`-style iteration over the live prefix rather than `for a in 0..dim { v[a] }`: same
/// ascending order, same terms in the same sequence, and it is what clippy wants because the
/// index exists only to reach the axis. The order is the load-bearing part (D2), and
/// `iter().take(dim)` states it.
pub(super) fn norm2(v: &[f64; MAX_DIM], dim: usize) -> f64 {
    let mut sum = 0.0;
    for x in v.iter().take(dim) {
        sum += x * x;
    }
    sum
}

fn coincident_nudge(key: (u32, u32), pair: (u32, u32), dim: usize) -> [f64; MAX_DIM] {
    let amp = NOISE * 2e6;
    let mut d = [0.0; MAX_DIM];
    for (a, slot) in d.iter_mut().enumerate().take(dim) {
        *slot = jiggle(key.0, key.1, (a + 2) as u32, pair) * amp;
    }
    if d.iter().take(dim).all(|v| *v == 0.0) {
        d[0] = NOISE;
    }
    d
}

/// Edge pull of magnitude `r^2` (unit weight): `delta * |delta|`.
fn attract(graph: &SimpleGraph, pos: &[[f64; MAX_DIM]], dim: usize, disp: &mut [[f64; MAX_DIM]]) {
    for (&a, &b) in graph.lo.iter().zip(&graph.hi) {
        let (v, u) = (a as usize, b as usize);
        let mut delta = [0.0; MAX_DIM];
        for k in 0..dim {
            delta[k] = pos[v][k] - pos[u][k];
        }
        let len = sqrt(norm2(&delta, dim));
        for k in 0..dim {
            disp[v][k] -= delta[k] * len;
            disp[u][k] += delta[k] * len;
        }
    }
}

/// Moves every vertex by its displacement capped at `temp`, after a tiny hash noise so a
/// perfectly balanced vertex still leaves a saddle.
fn step(
    pos: &mut [[f64; MAX_DIM]],
    disp: &[[f64; MAX_DIM]],
    temp: f64,
    key: (u32, u32),
    dim: usize,
) {
    for (v, d) in disp.iter().enumerate() {
        let vv = (v as u32, v as u32);
        let mut d = *d;
        for (a, slot) in d.iter_mut().enumerate().take(dim) {
            *slot += jiggle(key.0, key.1, a as u32, vv) * NOISE * 2e6;
        }
        let len = sqrt(norm2(&d, dim));
        if len > temp {
            for slot in d.iter_mut().take(dim) {
                *slot = *slot / len * temp;
            }
        }
        if len > 0.0 {
            for (slot, move_) in pos[v].iter_mut().zip(d.iter()).take(dim) {
                *slot += move_;
            }
        }
    }
}
