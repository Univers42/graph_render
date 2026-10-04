//! Davidson-Harel (`layout.force.davidson_harel`), written from
//! `docs/layouts/layout.force.davidson_harel.md` and the paper (ACM TOG 15(4), 1996) only,
//! per `docs/decisions/layouts-igraph.md`. 2D simulated annealing over five energy terms.

mod energy;
#[cfg(test)]
mod tests;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::rng::Mulberry32;
use crate::stage::{Stage, StageError};
use energy::{Field, Probe, delta, resting};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// Energy weights.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weights {
    /// Node-node repulsion.
    pub node_dist: f64,
    /// Border repulsion.
    pub border: f64,
    /// Squared edge length penalty.
    pub edge_lengths: f64,
    /// Crossing count penalty.
    pub edge_crossings: f64,
    /// Node-to-edge closeness penalty; fine-tuning rounds only.
    pub node_edge_dist: f64,
}

/// SciGraphs' defaults: fine-tuning off, so `node_edge_dist` never runs by default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DhParams {
    /// Annealing rounds.
    pub maxiter: u32,
    /// Fine-tuning rounds (strict improvement only).
    pub fineiter: u32,
    /// Radius multiplier per round, in (0, 1).
    pub cool_fact: f64,
    /// Energy weights.
    pub weights: Weights,
    /// Seeds placement, shuffles and acceptance draws (D5).
    pub seed: u32,
}

impl Default for DhParams {
    fn default() -> Self {
        Self {
            maxiter: 10,
            fineiter: 0,
            cool_fact: 0.95,
            weights: Weights {
                node_dist: 1.0,
                border: 0.0,
                edge_lengths: 1.0,
                edge_crossings: 1.0,
                node_edge_dist: 1.0,
            },
            seed: 0,
        }
    }
}

/// Node count past which the rounds are no longer usable: each round is O(30 n (n + deg m)).
pub const DH_CEILING: u64 = 500;

/// Candidate moves per node per round.
const TRIALS: usize = 30;

/// Davidson-Harel layout stage.
///
/// Ponytail: simulated annealing finds a local optimum only and quality depends on the
/// weights, which igraph calls graph dependent. Coincident nodes are floored at a squared
/// distance of 1e-12 where the spec divides by zero. The random stream is ours, so
/// coordinates never match igraph's.
pub struct DavidsonHarel;

impl Stage for DavidsonHarel {
    type Params = DhParams;
    const ID: &'static str = "layout.force.davidson_harel";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let n = topology.node_count() as usize;
        let pos = if n == 0 {
            Vec::new()
        } else {
            anneal(topology, params)?
        };
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

fn anneal(topology: &Topology, params: &DhParams) -> Result<Vec<[f64; 2]>, StageError> {
    let n = topology.node_count() as usize;
    let half = 5.0 * f64::sqrt(n as f64);
    let (edges, adj) = edge_lists(topology);
    let shape = Shape {
        adj: &adj,
        edges: &edges,
        half,
    };
    let mut rng = Mulberry32::new(params.seed);
    let mut run = Run {
        pos: scatter(n, half, &mut rng),
        bounds: [0.0; 4],
        rng,
    };
    run.bounds = bounding(&run.pos);
    let mut radius = half;
    for round in 0..params.maxiter + params.fineiter {
        let fine = round >= params.maxiter;
        if fine {
            radius = 0.01 * (run.bounds[2] - run.bounds[0]).min(run.bounds[3] - run.bounds[1]);
        }
        one_round(
            &shape,
            Round {
                fine,
                radius,
                weights: params.weights,
            },
            &mut run,
        );
        radius *= params.cool_fact;
    }
    if run.pos.iter().flatten().any(|v| !v.is_finite()) {
        return Err(StageError::NonFinite { column: "node.x" });
    }
    Ok(run.pos)
}

/// The whole mutable state of a run: the positions, the bounding box they currently
/// occupy, and the one generator. Held as one value so a round is a single borrow and
/// [`one_round`] stays inside the house's parameter cap.
struct Run {
    pos: Vec<[f64; 2]>,
    bounds: [f64; 4],
    rng: Mulberry32,
}

/// `n` positions drawn from `rng`, uniform over the square `[-half, half]` on both axes.
/// igraph draws its own start for the same purpose, which is why the two arms' pictures
/// differ from the first round on.
fn scatter(n: usize, half: f64, rng: &mut Mulberry32) -> Vec<[f64; 2]> {
    (0..n)
        .map(|_| {
            let x = (rng.next_f64() - 0.5) * 2.0 * half;
            [x, (rng.next_f64() - 0.5) * 2.0 * half]
        })
        .collect()
}

/// One round: every node moved once, in a shuffled order, at this round's radius.
///
/// The node-to-edge term is zeroed on the coarse rounds and present on the fine ones, which
/// is the source's own split and the reason the two phases are one loop rather than two:
/// `params.fineiter` rounds run at `fine = true` and reach the edge term.
fn one_round(shape: &Shape, round_state: Round, run: &mut Run) {
    let mut weights = round_state.weights;
    if !round_state.fine {
        weights.node_edge_dist = 0.0;
    }
    let state = Round {
        weights,
        ..round_state
    };
    let mut order: Vec<u32> = (0..run.pos.len() as u32).collect();
    shuffle(&mut order, &mut run.rng);
    for &v in &order {
        let (pos, bounds) = (&mut run.pos, &mut run.bounds);
        try_node(shape, &state, (pos, bounds), (v, &mut run.rng));
    }
}

/// The graph and canvas, fixed for the whole run.
struct Shape<'a> {
    adj: &'a [Vec<u32>],
    edges: &'a [(u32, u32)],
    half: f64,
}

impl<'a> Shape<'a> {
    /// `Shape` as the [`Field`] the energy terms read, over the positions as they are now.
    fn field<'b>(&'b self, pos: &'b [[f64; 2]]) -> Field<'b> {
        Field {
            pos,
            adj: self.adj,
            edges: self.edges,
            half_width: self.half,
        }
    }
}

/// What one round holds constant while its nodes move.
struct Round {
    fine: bool,
    radius: f64,
    weights: Weights,
}

/// Up to 30 shuffled trial moves of node `v`, each judged against the current positions.
fn try_node(
    shape: &Shape,
    round: &Round,
    state: (&mut Vec<[f64; 2]>, &mut [f64; 4]),
    who: (u32, &mut Mulberry32),
) {
    let (pos, bounds) = state;
    let (v, rng) = who;
    let mut angles: Vec<usize> = (0..TRIALS).collect();
    shuffle(&mut angles, rng);
    // What this node already contributes to the two per-candidate terms, recorded once per
    // position rather than once per candidate: all 30 candidates are tried from the same
    // place, and only an accepted move changes the answer. Same values in the same order,
    // so the energy is unchanged.
    let mut rest = resting(&shape.field(pos), v, pos[v as usize]);
    for k in angles {
        let q = candidate(pos[v as usize], round.radius, k, shape.half);
        let probe = Probe::new(shape.field(pos), &rest);
        let d_e = delta(&probe, &round.weights, v, (pos[v as usize], q));
        let accept = d_e < 0.0 || (!round.fine && rng.next_f64() < libm::exp(-d_e / round.radius));
        if accept {
            pos[v as usize] = q;
            grow(bounds, q);
            rest = resting(&shape.field(pos), v, q);
        }
    }
}

/// Directed edge list and per-node neighbour list (multi-edges repeated), ids from the topology.
fn edge_lists(t: &Topology) -> (Vec<(u32, u32)>, Vec<Vec<u32>>) {
    let e = t.edges();
    let mut adj = vec![Vec::new(); t.node_count() as usize];
    let mut edges = Vec::new();
    for i in 0..t.edge_count() as usize {
        let (a, b) = (e.source[i], e.target[i]);
        edges.push((a, b));
        adj[a as usize].push(b);
        adj[b as usize].push(a);
    }
    (edges, adj)
}

/// Move by `radius` at angle `2 pi k / 30`; a coordinate past the canvas is set to the edge
/// minus 1e-6, exactly as the spec's (asymmetric) clamp says.
fn candidate(at: [f64; 2], radius: f64, k: usize, half: f64) -> [f64; 2] {
    let angle = 2.0 * core::f64::consts::PI * k as f64 / TRIALS as f64;
    let mut q = [
        at[0] + radius * libm::cos(angle),
        at[1] + radius * libm::sin(angle),
    ];
    for c in &mut q {
        if *c > half {
            *c = half - 1e-6;
        } else if *c < -half {
            *c = -half - 1e-6;
        }
    }
    q
}

fn shuffle<T>(items: &mut [T], rng: &mut Mulberry32) {
    for i in (1..items.len()).rev() {
        items.swap(i, rng.pick(i + 1));
    }
}

/// `[min_x, min_y, max_x, max_y]`.
fn bounding(pos: &[[f64; 2]]) -> [f64; 4] {
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for &p in pos {
        grow(&mut b, p);
    }
    b
}

/// Else-if chain of the spec: a point that is a new minimum does not also test the maximum.
fn grow(b: &mut [f64; 4], p: [f64; 2]) {
    for a in 0..2 {
        if p[a] < b[a] {
            b[a] = p[a];
        } else if p[a] > b[a + 2] {
            b[a + 2] = p[a];
        }
    }
}
