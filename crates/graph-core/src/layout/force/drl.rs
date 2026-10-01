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
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
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
        }
    }
}

/// Node count past which a run is no longer interactive.
pub const DRL_CEILING: u64 = 5_000;

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

impl Stage for Drl {
    type Params = DrlParams;
    const ID: &'static str = "layout.force.drl";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let n = topology.node_count() as usize;
        let mut pos = vec![[0.0; 2]; n];
        if n > 0 {
            sweeps(topology, params, &mut pos);
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

fn sweeps(topology: &Topology, params: &DrlParams, pos: &mut [[f64; 2]]) {
    let mut adj = adjacency(topology);
    let mut schedule = Schedule::new(params);
    let mut field = Density::new();
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
            pos[v] = better_position(v, (&*pos, &mut adj), &schedule, (&field, &mut rng));
            field.add(v as u32, pos[v]);
        }
        first = false;
        schedule.advance();
    }
}

/// Node `v`'s next position: the lower-energy of the analytic candidate and its jitter.
fn better_position(
    v: usize,
    (pos, adj): (&[[f64; 2]], &mut Adjacency),
    schedule: &Schedule,
    (field, rng): (&Density, &mut Mulberry32),
) -> [f64; 2] {
    let Some(centroid) = centroid(&adj[v], pos) else {
        return pos[v];
    };
    if schedule.cutting() {
        cut_farthest(v, centroid, (pos, adj), schedule);
    }
    let damping = 1.0 - schedule.damping_mult();
    let analytic = [
        damping * pos[v][0] + (1.0 - damping) * centroid[0],
        damping * pos[v][1] + (1.0 - damping) * centroid[1],
    ];
    let scale = 0.01 * schedule.temperature();
    let jittered = [
        analytic[0] + (0.5 - rng.next_f64()) * scale,
        analytic[1] + (0.5 - rng.next_f64()) * scale,
    ];
    let energy = |at: [f64; 2]| node_energy(v as u32, at, (pos, &adj[v]), (field, schedule));
    if energy(jittered) <= energy(analytic) {
        jittered
    } else {
        analytic
    }
}

fn centroid(neighbours: &[u32], pos: &[[f64; 2]]) -> Option<[f64; 2]> {
    if neighbours.is_empty() {
        return None;
    }
    let sum = neighbours.iter().fold([0.0; 2], |acc, &u| {
        [acc[0] + pos[u as usize][0], acc[1] + pos[u as usize][1]]
    });
    let count = neighbours.len() as f64;
    Some([sum[0] / count, sum[1] / count])
}

/// Drops the one neighbour of `v` that is farthest from the centroid (weighted by the
/// square root of its degree) when that beats the current cut length and `v` has enough
/// neighbours. The edge stays in the other node's list.
fn cut_farthest(
    v: usize,
    centroid: [f64; 2],
    (pos, adj): (&[[f64; 2]], &mut Adjacency),
    schedule: &Schedule,
) {
    if (adj[v].len() as f64) < schedule.min_edges() {
        return;
    }
    let mut worst: Option<(usize, f64)> = None;
    for (slot, &u) in adj[v].iter().enumerate() {
        let d = [
            pos[u as usize][0] - centroid[0],
            pos[u as usize][1] - centroid[1],
        ];
        let away = sqrt(adj[u as usize].len() as f64) * (d[0] * d[0] + d[1] * d[1]);
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
    at: [f64; 2],
    (pos, neighbours): (&[[f64; 2]], &[u32]),
    (field, schedule): (&Density, &Schedule),
) -> f64 {
    let a2 = schedule.attraction() * schedule.attraction();
    let a = 0.02 * (a2 * a2);
    let mut energy = field.energy(v, at, pos);
    for &u in neighbours {
        let d = [at[0] - pos[u as usize][0], at[1] - pos[u as usize][1]];
        energy += a * schedule.pull(d[0] * d[0] + d[1] * d[1]);
    }
    energy
}
