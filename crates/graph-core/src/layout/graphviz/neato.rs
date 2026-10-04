//! Graphviz 16.1.0's `neato` at its defaults: stress majorization over the all-pairs
//! shortest-path distances.
//!
//! Reference: `lib/neatogen/stress.c` (`stress_majorization_kD_mkernel`), `conjgrad.c`
//! (`conjugate_gradient_mkernel`), `bfs.c` and `neatoinit.c` (`checkStart`, `majorization`,
//! `makeGraphData`), of the pinned release, read as an algorithm reference and
//! reimplemented — never translated, never linked (`docs/decisions/graphviz-oracle.md`).
//! The pipeline, in the reference's order:
//!
//! 1. `bfs` per source, packed into the upper triangle: `d_ij` is the hop count, and an
//!    unreachable node is `closestDist + 10` (`bfs.c:47-49`).
//! 2. `lap2 = 1 / d_ij^2` — the constant `stresswt = 2` normalises the stress by `d_ij^2`
//!    (`stress.c:938-941`).
//! 3. `initLayout`: every coordinate is a `drand48` draw seeded from `-Gstart`, then each
//!    column is centred (`stress.c:131-160`).
//! 4. Repeat, at most `DFLT_ITERATIONS = 200` times: build the Laplacian of
//!    `1 / (d_ij * |p_i - p_j|)`, take `b = L p`, measure the stress, and solve
//!    `L x' = b` by conjugate gradient. Stop when the stress changes by less than
//!    `Epsilon = 1e-4` in relative terms, or falls below it outright.
//!
//! **`-Gstart` is load-bearing here, and that is the whole difference from `twopi`.** twopi
//! is closed form and reads no `start`; neato's `initLayout` seeds a generator and reads
//! two draws per node, so the drawing is a function of the seed. Measured: all 1000
//! differential fixtures draw differently at `-Gstart` 1, 7 and 99. `rng` is therefore
//! part of the port, not a convenience — reproducing the layout and not the generator
//! would be reproducing half of it.
//!
//! **Units are the reference's own `ND_pos`, not points.** Graphviz's report-time
//! `PSinputscale` multiplies `ND_pos` to get `ND_coord`, and the differential's
//! bounding-box rescale divides the scale back out, so this port emits `ND_pos` and claims
//! nothing about a unit it is not asked for. The initial placement is `drand48` in `[0,1)`,
//! which is the scale the drawing inherits: neato's stress is invariant under a similarity,
//! so no absolute length in the output is meaningful without fixing that seed.
//!
//! Determinism: every pass is a loop over a node-index range or a CSR row in ascending
//! order, the breadth-first searches are queue-ordered, and the only non-exact operations
//! are the reference's own `sqrtf` and the `double` divisions of the conjugate gradient
//! (`prompt.md` §6 D1-D10). Nothing reads a clock, a `HashMap` or an unseeded generator.
//! The iteration is sequential by nature — a node's position depends on every other node's
//! — so it is not handed to a runner and is not written in gather form; D10's condition is
//! not met and that is stated rather than implied.
//!
//! Ponytail: a **disconnected** graph. The reference gives an unreachable node a distance
//! of `closestDist + 10` where `closestDist` is the distance of the last node its
//! breadth-first queue emptied on, so the answer depends on the order the queue happened to
//! drain in; this port uses its own queue and reproduces the arithmetic, not that choice.
//! Failing input: any graph with a node no edge reaches. Direction: the unreachable node
//! lands a different, still finite, distance off its component — cosmetic, and every
//! reachable node is still placed by exactly the same arithmetic. Escape hatch: connect the
//! graph; the differential's fixtures are connected by construction, so the two arms are
//! only ever compared where they agree.
//!
//! Ponytail: the **iteration is stopped, not finished.** `Epsilon` is a relative change in
//! the stress and the conjugate gradient's own `1e-3` is a bound on a residual, so the
//! drawing is within a tolerance of the stress optimum and not at it. Failing input: any
//! graph, at the far end of its convergence. Direction: spacing ratios are off by the
//! tolerance rather than exact — a 3-path's ends measure 1.9830 where the hop counts say
//! 2. Escape hatch: the differential, which compares this port with `neato -Tplain` itself
//! and reports the gap the stopping rule leaves.

mod conjugate;
mod distance;
mod matrix;
mod rng;
mod solve;

use crate::budget;
use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::coords::point_geometry;
use crate::stage::StageError;

/// The capability id, and the hash gate's stage name.
pub const ID: &str = "layout.force.neato";

/// `DFLT_ITERATIONS` (`stress.h:21`): `neatoLayout`'s budget when `mode=major` and no
/// `maxiter` is set (`neatoinit.c:1281-1283`).
const MAX_ITERATIONS: u32 = 200;

/// `DFLT_TOLERANCE` (`stress.h:25`), the default `Epsilon` for `mode=major`
/// (`stuff.c:257`): the iteration stops when the stress changes by less than this in
/// relative terms, or falls below it outright.
pub const EPSILON: f64 = 1e-4;

/// `tolerance_cg` (`stress.h:21`): the conjugate gradient's own stopping bound, on the
/// largest absolute residual component.
const CG_TOLERANCE: f64 = 1e-3;

/// The stress weight `exp` (`neatoinit.c:955-960`): the reference's default is 2, which
/// normalises the stress terms by `d_ij^2`. The port implements **that case only** and says
/// so where it squares, because `exp = 1` is a different normalisation and a different
/// layout, and a port that quietly ignored the value would be one.
const STRESS_WEIGHT: u32 = 2;

/// The graph the differential runs the engine over is connected, so the port fixes `-Gstart`
/// at the differential's own seed rather than reading an attribute: the ledger row and the
/// oracle both run it at `-Gstart=1`, and a layout that took a seed from a caller would be
/// a different answer from the one gated.
const START_SEED: u32 = 1;

/// `layout.force.neato` at its defaults. No `Stage` impl and no `Params`, by the same
/// decision `layout::radial::twopi` records: the reference exposes `maxiter`, `epsilon`,
/// `stresswt`, `start`, `overlap` and `pack`, none of which the ledger row or the oracle
/// uses, and publishing a `Params` to gain a hash-gate knob would be the tail wagging the
/// dog. The hash gate's `GM_MUTATE_NEATO_EPSILON` reaches [`run_with`] instead, which is the
/// same shape as `circle_packing::run_with`: the registry's `run` is the default, and a gate
/// row that needs a different tolerance asks for it by name.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    run_with(topology, EPSILON)
}

/// [`run`] at an explicit stopping tolerance, which is [`run_with`].
///
/// Published for one reason and one reason only: the hash gate needs a control that perturbs
/// a *parameter* of this stage, and a parameter it cannot name is not a parameter. It is not
/// a general interface — `maxiter`, `stresswt` and `start` are the reference's other knobs
/// and are deliberately absent, because a `Params` carrying all of them would invite callers
/// to configurations nothing gates.
pub fn run_with(topology: &Topology, epsilon: f64) -> Result<Geometry, StageError> {
    let count = topology.node_count() as usize;
    if count < 2 {
        // `neatoLayout` returns before the majorization when `nG < 2` (`neatoinit.c:1290`),
        // leaving every coordinate where it was: the origin.
        return Ok(point_geometry(&vec![0.0; count], &vec![0.0; count]));
    }
    // Three f32 triangles: the hop distances, their weights and the weighted Laplacian.
    budget::quadratic(budget::triangle(count as u64), 12)?;
    let pairs = undirected(topology);
    let neighbours = crate::csr::Csr::from_pairs(topology.node_count(), pairs.into_iter())
        .map_err(StageError::Capacity)?;
    let solution = solve::majorize(&neighbours, count, START_SEED, MAX_ITERATIONS, epsilon);
    Ok(point_geometry(&solution.x, &solution.y))
}

/// Every edge as an unordered pair, both directions, with duplicates left in place.
///
/// Duplicates are deliberate and harmless: the search reads the CSR as an adjacency *set*
/// in effect — a node already reached is not reached again — so a repeated edge costs a
/// comparison and changes nothing. The reference folds them in `makeGraphData` for a
/// reason this port does not have: it keeps edge *lengths*, and a doubled edge would
/// otherwise contribute twice.
///
/// Self-loops are dropped for the reference's reason (`neatoinit.c:794-795`): a node that
/// reaches itself has not reached anything, and a loop in the CSR would only make the
/// breadth-first search test a node against itself.
fn undirected(topology: &Topology) -> Vec<(u32, u32)> {
    let edges = topology.edges();
    let mut out: Vec<(u32, u32)> = edges
        .source
        .iter()
        .zip(edges.target.iter())
        .filter(|pair| pair.0 != pair.1)
        .map(|(&source, &target)| (source, target))
        .collect();
    let incoming: Vec<(u32, u32)> = edges
        .target
        .iter()
        .zip(edges.source.iter())
        .filter(|pair| pair.0 != pair.1)
        .map(|(&target, &source)| (target, source))
        .collect();
    out.extend(incoming);
    out
}

#[cfg(test)]
mod tests;
