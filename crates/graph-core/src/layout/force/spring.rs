//! `layout.force.spring`: SciGraphs' `SPRING` at two dimensions — a port of networkx 3.6
//! `spring_layout(G, dim=2, iterations, scale, seed=...)`, which
//! `SciGraphs/core/scigraphs_core/mesh/layouts/networkx_layouts.py:16-24` calls with
//! nothing else. `Point` nodes, straight `Line` edges.
//!
//! **`SPRING_3D` is this same function at `dim=3`**
//! (`networkx_layouts.py:26-34`, one wrapper over `spring_layout` with the dimension
//! literal changed and nothing else), and SciGraphs offers both names for the one
//! algorithm. The motor is 2D (`NodeGeometry::Point` carries `x, y` only), so this id
//! answers the `SPRING` row and `SPRING_3D` stays unimplemented until `contract-3d`
//! exists — at which point the port is `dim = 3` and the same differential runs again.
//!
//! This is **not** `layout.forceatlas2` and **not** `layout.force.barnes_hut`:
//! Fruchterman–Reingold attraction and repulsion, dense, with a linear cooling schedule,
//! where FA2 is ForceAtlas2 with cumulative swing and Barnes-Hut approximates the
//! many-body term with a theta-tree. Nor is it d3-force, which is a velocity-Verlet
//! integrator with velocity decay rather than a temperature schedule.
//!
//! **Not Graphviz `neato`,** and nothing was compared against it: `neato` overlaps FR but
//! applies its own overlap removal and adaptive temperature, and `p13-gv2-neato` is the
//! branch that owns that comparison.
//!
//! **Reference, and where the port stops.** `spring_layout` (`layout.py:452-651`) computes
//! `k = sqrt(1/n)`, an opening temperature of a tenth of the start's larger coordinate
//! span, and up to `iterations` steps of [`forces::Solver::gather`], each node's
//! displacement being `sum_j delta_ij * (k*k/d_ij^2 - A_ij * d_ij / k)` with `d` clipped
//! to 0.01; it then rescales to `scale` (`layout.py:646`). Four departures, all stated
//! rather than hidden:
//!
//! 1. **Initial positions** come from graph-core's own seeded `Mulberry32` at [`SEED`],
//!    not numpy's `RandomState` (D5: there is no global RNG to reach for, and the same
//!    graph must hash the same on every target). The reference draws `seed.rand(n, 2)`, so
//!    no coordinate of ours equals networkx's for any seed.
//! 2. **The reduction is split** — repulsion over all `j`, then attraction over the node's
//!    own row — where the reference fuses them into one pass over a dense `n x n` matrix.
//!    The same sum, a different rounding (see [`forces`]).
//! 3. **The graph is the undirected simple one**
//!    ([`crate::layout::force::simple_graph`]), so a parallel edge is one edge and a
//!    self-loop none, and `A[i, j]` is 0 or 1 and never an edge weight. The reference's
//!    `weight="weight"` reads a networkx attribute the SciGraphs caller never sets, so 0/1
//!    is what it sees too.
//! 4. **`n < 500`.** `spring_layout` picks `method="auto"` (`layout.py:140-141`), which is
//!    `"force"` under 500 nodes and `"energy"` — an L-BFGS-B minimisation of the same
//!    energy, `_energy_fruchterman_reingold` (`layout.py:814-880`) — at or above it. This
//!    port is the force branch only and says so; a 500-node spring is *not* the
//!    reference's answer. See the module Ponytail.
//!
//! **The differential is a stress ratio, never a coordinate gap.** Both arms start from
//! different positions and diverge inside the first step, so a byte or tolerance
//! comparison would measure the chaos rather than the port. What is compared is whether
//! the drawing *respects the graph's own distances*: Pearson correlation between BFS hop
//! distance and Euclidean distance, each arm scored separately over 32 max-min pivots by
//! the crate's own `crates/graph-cli/src/stress/metric.rs`, and what is gated is the
//! deficit `max(0, theirs - ours)` — not a ratio of ours to networkx's, since both
//! correlations may be negative. The honest claim is "different, but not worse". The
//! analytically-determined small cases — one node, two nodes, a path, a star — are
//! asserted exactly in `tests`, because those have a closed answer no statistic can
//! replace, and the harness asserts the reference's own rescale contract on both arms.
//!
//! Ponytail: a force layout is CHAOTIC, and this one is the most so of the three the motor
//! ships: one added node is a different picture, not a perturbed one, and the port is not
//! even the reference's algorithm above 500 nodes. Failing input: any graph dense enough
//! that a one-ulp difference in reduction order grows over `iterations` — which is every
//! graph at the default budget. Direction: the drawing is *different*, and the stress
//! ratio cannot tell a mirrored or rotated but otherwise equivalent embedding from a
//! different one, so it certifies distances and nothing about orientation. It is also a
//! *statistic*, so the gate is on its **median** over 1000 seeds — 7.288e-3 against a 1e-1
//! ceiling — and its worst (1.861e-1) is recorded rather than gated: on a small graph 50
//! iterations from a random start is far from settled, so the tail is noise and a ceiling
//! loose enough to admit it could not be falsified. The exact assertions are what carry the
//! claim. The one real loss is the `n >= 500` fork, where
//! the reference's energy minimiser is not reproduced
//! at all. Escape hatch, from both ends: [`SpringParams::iterations`] and
//! [`SpringParams::scale`] are parameters, so `run_with` lays the graph out at any
//! budget and `--max-iter` on `emit-spring-fixtures` re-measures the whole comparison at
//! another one; `GM_MUTATE_SPRING_ITERATIONS` perturbs this stage against the same gate so
//! the claim is falsifiable from the port's side.

mod forces;
#[cfg(test)]
mod tests;

use super::simple_graph;
use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::{Stage, StageError};
use crate::synthetic::Mulberry32;
use forces::{Field, Solver};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// The layout's capability id, which is also its hash-gate stage.
pub const ID: &str = "layout.force.spring";

/// Fixed stream seed, as `layout/random.rs` pins its own. Changing it moves every hashed
/// snapshot, so it is a `const` and never a parameter.
const SEED: u32 = 0x00_5EED;

/// networkx's own `threshold=1e-4` (`layout.py:457`), which the port keeps.
const THRESHOLD: f64 = 1e-4;

/// The reference's minimum separation, `np.clip(distance, 0.01, None)` (`layout.py:713`),
/// and the same bound on the displacement length (`layout.py:720`).
const MIN_DISTANCE: f64 = 0.01;
const MIN_LENGTH: f64 = 0.01;

/// The parameters `spring_layout` takes, as a `Params` so a caller and the differential's
/// `--max-iter` can both move one.
///
/// The defaults are **networkx 3.6's own**, which is also what SciGraphs passes for
/// `iterations` and `scale` (`dispatcher.py:14`): `iterations=50`, `scale=5.0`,
/// `threshold=1e-4`. `scale` is applied by [`rescale_to`], after the solve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringParams {
    /// Maximum gathers, `layout.py:457`'s `iterations`.
    pub iterations: u32,
    /// Early-exit bound on `norm(delta_pos) / n`, `layout.py:458`.
    pub threshold: f64,
    /// Final extent, `layout.py:460`.
    pub scale: f64,
}

impl Default for SpringParams {
    fn default() -> Self {
        SpringParams {
            iterations: 50,
            threshold: THRESHOLD,
            scale: 5.0,
        }
    }
}

/// The spring layout over the Fruchterman–Reingold dense force pass.
pub struct Spring;

impl Stage for Spring {
    type Params = SpringParams;
    const ID: &'static str = ID;

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let n = topology.node_count();
        // `spring_layout` returns `center` for a single node and `{}` for none
        // (`layout.py:618-624`), before any force is computed; ported rather than
        // approximated, because a one-node graph's answer *is* the centre.
        if n < 2 {
            return Ok(point_field(&Field::zeros(n)));
        }
        let graph = simple_graph(topology);
        let mut field = Solver::new(&graph, n).settle(start(n), *params);
        rescale_to(&mut field, params.scale);
        if field.x.iter().chain(&field.y).any(|v| !v.is_finite()) {
            return Err(StageError::NonFinite { column: "node.x" });
        }
        Ok(point_field(&field))
    }
}

/// `Point` nodes at the field's own `f32` columns, straight `Line` edges, no notes.
fn point_field(field: &Field) -> Geometry {
    let narrow = |column: &[f64]| column.iter().map(|&v| v as f32).collect::<Vec<f32>>();
    Geometry {
        nodes: NodeGeometry::Point {
            x: narrow(&field.x),
            y: narrow(&field.y),
        },
        edges: EdgeGeometry::Line,
        notes: Vec::new(),
    }
}

/// The start positions, `layout.py:621` reading as "uniform in the unit square": this
/// crate's `Mulberry32` at [`SEED`], `x` then `y` per node, as `layout/random.rs` and
/// `forceatlas2` do. Departure 1 in the module doc.
fn start(n: u32) -> Field {
    let mut stream = Mulberry32::new(SEED);
    let (mut x, mut y) = (
        Vec::with_capacity(n as usize),
        Vec::with_capacity(n as usize),
    );
    for _ in 0..n {
        x.push(stream.next_f64());
        y.push(stream.next_f64());
    }
    Field { x, y }
}

/// networkx 3.6 `rescale_layout(pos, scale)` (`layout.py:1882-1924`), which
/// `spring_layout` applies at `layout.py:646` when no node is fixed: subtract the mean per
/// axis, then scale so the largest magnitude over both axes is exactly `scale`.
///
/// **Not [`crate::layout::coords::rescale`]**, which is the same function at `scale = 1`
/// and is what the closed-form layouts call. Reached for a `scale` other than 1, which
/// only this layout passes; the arithmetic is the same order, ascending index, and
/// `scale` goes in as one reciprocal multiply so the result is the reference's
/// `pos *= scale / lim` (D3).
fn rescale_to(field: &mut Field, scale: f64) {
    if field.x.is_empty() {
        return;
    }
    let count = field.x.len() as f64;
    let mean_x = field.x.iter().sum::<f64>() / count;
    let mean_y = field.y.iter().sum::<f64>() / count;
    let mut limit = 0.0_f64;
    for i in 0..field.x.len() {
        field.x[i] -= mean_x;
        field.y[i] -= mean_y;
        limit = limit.max(field.x[i].abs()).max(field.y[i].abs());
    }
    if limit > 0.0 {
        let factor = scale / limit;
        for value in field.x.iter_mut().chain(field.y.iter_mut()) {
            *value *= factor;
        }
    }
}
