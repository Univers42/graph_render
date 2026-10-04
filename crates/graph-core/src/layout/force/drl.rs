//! DrL (`layout.force.drl`), written from `docs/layouts/layout.force.drl.md` and the
//! SAND2008-2936 report only, per `docs/decisions/layouts-igraph.md`. 2D, single process,
//! unweighted: sequential sweeps that move every node to the lower-energy of an analytic
//! candidate and a jittered copy, against a density field, on a fixed schedule.

mod density;
mod schedule;
#[cfg(test)]
mod tests;

use super::fruchterman_reingold::sqrt;
use super::simple_graph;
use crate::index::Topology;
use crate::layout::Geometry;
use crate::rng::Mulberry32;
use crate::stage::{Stage, StageError};
use density::Density;
use schedule::Schedule;

/// One phase of the schedule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Phase {
    /// Sweeps in the phase.
    pub iterations: u32,
    /// Jitter scale at the start of the phase.
    pub temperature: f64,
    /// Attraction at the start of the phase.
    pub attraction: f64,
    /// Fraction of the pull to the neighbours' centroid, in `[0, 1.5]`.
    pub damping_mult: f64,
}

/// The default preset: edge cut plus the six phases, in order init, liquid, expansion,
/// cooldown, crunch, simmer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrlParams {
    /// Edge-cutting strength in `[0, 1]`.
    pub edge_cut: f64,
    /// The six phases.
    pub phases: [Phase; 6],
    /// Seeds the jitter (D5).
    pub seed: u32,
    /// How many coordinates every node carries: `2`, or `3` for `layout.force.drl.3d`.
    /// SciGraphs' `IGRAPH_DRL` is the 3D one and `IGRAPH_DRL_2D` the 2D, so this
    /// parameter is what the two dispatcher arms differ on.
    pub dim: usize,
}

const fn phase(iterations: u32, temperature: f64, attraction: f64, damping_mult: f64) -> Phase {
    Phase {
        iterations,
        temperature,
        attraction,
        damping_mult,
    }
}

impl Default for DrlParams {
    fn default() -> Self {
        Self {
            edge_cut: 0.8,
            phases: [
                phase(0, 2000.0, 10.0, 1.0),
                phase(200, 2000.0, 10.0, 1.0),
                phase(200, 2000.0, 2.0, 1.0),
                phase(200, 2000.0, 1.0, 0.1),
                phase(50, 250.0, 1.0, 0.25),
                phase(100, 250.0, 0.5, 0.0),
            ],
            seed: 0,
            dim: 2,
        }
    }
}

/// Node count past which a run is no longer interactive.
pub const DRL_CEILING: u64 = 5_000;

/// The widest point this port keeps; see
/// [`super::fruchterman_reingold::MAX_DIM`].
const MAX_DIM: usize = 3;

/// DrL layout stage.
///
/// Ponytail: the source works in single precision and this port in double, so even from
/// one start the two drift, and the liquid stage amplifies it; the differential compares
/// layout stress, not positions. Sweeps are index-ordered, so node numbering changes the
/// picture. Edge cutting is permanent, one-directional and can disconnect the effective
/// graph. The starting minimum-edge count of 20 and the stage-transition sweep are our
/// reading of the spec, which gives neither as a number. A node that leaves the plane
/// is treated as sitting on the border wall instead of raising an error; the stage-transition
/// sweep the source adds is omitted.
pub struct Drl;

/// SciGraphs' `IGRAPH_DRL` asks igraph for `dim = 3` (`igraph_layouts.py:342`); this is
/// the same kernel at 3. `DRL_2D` (`_igraph_drl_2d`, `igraph_layouts.py:307`) stays on
/// `layout.force.drl`, which is this arm at `dim` 2.
pub const ID_3D: &str = "layout.force.drl.3d";

impl Stage for Drl {
    type Params = DrlParams;
    const ID: &'static str = "layout.force.drl";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        run_at_dim(topology, params, params.dim)
    }
}

/// The 3D arm: [`Stage::run`] with `dim` forced to 3. See
/// [`super::fruchterman_reingold::run_3d`] for why it is a function and not a second
/// `Stage` impl.
pub fn run_3d(topology: &Topology, params: &DrlParams) -> Result<Geometry, StageError> {
    run_at_dim(topology, params, MAX_DIM)
}

fn run_at_dim(topology: &Topology, params: &DrlParams, dim: usize) -> Result<Geometry, StageError> {
    if !(2..=MAX_DIM).contains(&dim) {
        return Err(StageError::Param {
            name: "dim",
            rule: "2 or 3 coordinates per node",
        });
    }
    let n = topology.node_count() as usize;
    let mut pos = vec![[0.0; MAX_DIM]; n];
    if n > 0 {
        sweeps(topology, params, dim, &mut pos);
    }
    if pos.iter().flatten().take(dim).any(|v| !v.is_finite()) {
        return Err(StageError::NonFinite { column: "node.x" });
    }
    let column = |a: usize| pos.iter().map(|p| p[a] as f32).collect();
    Ok(Geometry::points(dim, column(0), column(1), column(2)))
}

/// Neighbour lists, one per node, mutable so edge cutting can drop one direction.
type Adjacency = Vec<Vec<u32>>;

fn adjacency(topology: &Topology) -> Adjacency {
    let graph = simple_graph(topology);
    let mut adj: Adjacency = vec![Vec::new(); graph.rows.rows() as usize];
    for (&a, &b) in graph.lo.iter().zip(&graph.hi) {
        adj[a as usize].push(b);
        adj[b as usize].push(a);
    }
    adj.iter_mut().for_each(|row| row.sort_unstable());
    adj
}

fn sweeps(topology: &Topology, params: &DrlParams, dim: usize, pos: &mut [[f64; MAX_DIM]]) {
    let mut adj = adjacency(topology);
    let mut schedule = Schedule::new(params);
    let mut field = Density::new(dim);
    let mut rng = Mulberry32::new(params.seed);
    let mut first = true;
    while !schedule.finished() {
        if schedule.fine() && !field.is_fine() {
            field.switch_to_fine(pos);
        }
        for v in 0..pos.len() {
            let from = pos[v];
            if !first {
                field.remove(v as u32, from);
            }
            pos[v] = better_position(v, (&*pos, &mut adj), &schedule, (&field, &mut rng), dim);
            field.add(v as u32, pos[v]);
        }
        first = false;
        schedule.advance();
    }
}

/// Node `v`'s next position: the lower-energy of the analytic candidate and its jitter.
fn better_position(
    v: usize,
    (pos, adj): (&[[f64; MAX_DIM]], &mut Adjacency),
    schedule: &Schedule,
    (field, rng): (&Density, &mut Mulberry32),
    dim: usize,
) -> [f64; MAX_DIM] {
    let Some(centroid) = centroid(&adj[v], pos, dim) else {
        return pos[v];
    };
    if schedule.cutting() {
        cut_farthest(v, centroid, (pos, adj), schedule, dim);
    }
    let damping = 1.0 - schedule.damping_mult();
    let mut analytic = [0.0; MAX_DIM];
    let mut jittered = [0.0; MAX_DIM];
    let scale = 0.01 * schedule.temperature();
    // Ascending axis, and the jitter draws in the same order, so the 2D arm reads exactly
    // the two `rng.next_f64()` calls it always read, in the same sequence.
    for a in 0..dim {
        analytic[a] = damping * pos[v][a] + (1.0 - damping) * centroid[a];
        jittered[a] = analytic[a] + (0.5 - rng.next_f64()) * scale;
    }
    let energy =
        |at: [f64; MAX_DIM]| node_energy(v as u32, at, (pos, &adj[v]), (field, schedule), dim);
    if energy(jittered) <= energy(analytic) {
        jittered
    } else {
        analytic
    }
}

fn centroid(neighbours: &[u32], pos: &[[f64; MAX_DIM]], dim: usize) -> Option<[f64; MAX_DIM]> {
    if neighbours.is_empty() {
        return None;
    }
    let mut sum = [0.0; MAX_DIM];
    for &u in neighbours {
        for a in 0..dim {
            sum[a] += pos[u as usize][a];
        }
    }
    let count = neighbours.len() as f64;
    for slot in sum.iter_mut().take(dim) {
        *slot /= count;
    }
    Some(sum)
}

/// Drops the one neighbour of `v` that is farthest from the centroid (weighted by the
/// square root of its degree) when that beats the current cut length and `v` has enough
/// neighbours. The edge stays in the other node's list.
fn cut_farthest(
    v: usize,
    centroid: [f64; MAX_DIM],
    (pos, adj): (&[[f64; MAX_DIM]], &mut Adjacency),
    schedule: &Schedule,
    dim: usize,
) {
    if (adj[v].len() as f64) < schedule.min_edges() {
        return;
    }
    let mut worst: Option<(usize, f64)> = None;
    for (slot, &u) in adj[v].iter().enumerate() {
        let mut d = [0.0; MAX_DIM];
        for a in 0..dim {
            d[a] = pos[u as usize][a] - centroid[a];
        }
        let away = sqrt(adj[u as usize].len() as f64) * norm2(&d, dim);
        if worst.is_none_or(|(_, best)| away > best) {
            worst = Some((slot, away));
        }
    }
    if let Some((slot, away)) = worst
        && away > schedule.cut_length()
    {
        adj[v].remove(slot);
    }
}

/// `sum of w * A * q(s)` over the neighbours plus the density term.
fn node_energy(
    v: u32,
    at: [f64; MAX_DIM],
    (pos, neighbours): (&[[f64; MAX_DIM]], &[u32]),
    (field, schedule): (&Density, &Schedule),
    dim: usize,
) -> f64 {
    let a2 = schedule.attraction() * schedule.attraction();
    let a = 0.02 * (a2 * a2);
    let mut energy = field.energy(v, at, pos);
    for &u in neighbours {
        let mut d = [0.0; MAX_DIM];
        for k in 0..dim {
            d[k] = at[k] - pos[u as usize][k];
        }
        energy += a * schedule.pull(norm2(&d, dim));
    }
    energy
}

/// `sum of squares` over the live axes, ascending — the 2D arm's `d[0]*d[0] + d[1]*d[1]`
/// in the same order and the same bits.
fn norm2(v: &[f64; MAX_DIM], dim: usize) -> f64 {
    let mut sum = 0.0;
    for x in v.iter().take(dim) {
        sum += x * x;
    }
    sum
}
