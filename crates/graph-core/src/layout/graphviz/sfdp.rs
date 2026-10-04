//! Graphviz 16.1.0's `sfdp`: the multilevel spring-electrical model of Hu (2005).
//!
//! **This is not SciGraphs' own `YIFAN_HU`.** SciGraphs reaches `sfdp` through
//! `_graphviz_engine_layout` (`SciGraphs/core/scigraphs_core/mesh/layouts/yifan_hu.py:340`,
//! dispatched at `dispatcher.py:140`), and the motor already ships that separate algorithm as
//! `layout.force.yifan_hu`. The two are different layouts that happen to share a name in one
//! of the two systems; this module is the Graphviz engine.
//!
//! Reference: `lib/sfdpgen/` of the pinned Graphviz 16.1.0 release — `spring_electrical.c`,
//! `Multilevel.c`, `post_process.c`, `stress_model.c` — read as an algorithm reference and
//! reimplemented, never translated line by line and never linked (Graphviz is EPL-1.0 and
//! `docs/decisions/graphviz-oracle.md` records that this is an independent implementation
//! agreeing on output, not a copy).
//!
//! **The shape of the thing.** Coarsen the graph by maximal matching, lay the coarsest level out
//! from a seeded random start, then walk back down: prolongate each coarse solution onto the
//! next finer level and relax it again with the step control switched off. Each level's force
//! is Barnes-Hut repulsion plus linear attraction along the edges
//! (`spring_electrical.c:39-42`), with an adaptive step that cools when the force norm stops
//! improving and warms when it does (`spring_electrical.c:171-185`).
//!
//! **Three reference behaviours are reproduced deliberately, and each is a place a
//! reimplementation silently diverges:**
//!
//! 1. **The random start is glibc's `rand()`.** `srand(seed)` then `drand()` per coordinate
//!    (`spring_electrical.c:282-284`), and `drand()` is `rand()/(double)RAND_MAX`
//!    (`lib/sparse/general.c:25-27`) — the divisor is `RAND_MAX` = 2^31-1, not 2^31. `start`
//!    implements glibc's TYPE_3 additive-feedback generator and pins its first eight outputs
//!    for seeds 1 and 2 against the system libc, because a recurrence that is merely *similar*
//!    to TYPE_3 starts every layout somewhere else and still passes a determinism test.
//! 2. **`K` is the mean edge length of the current positions**, recomputed at every level
//!    (`spring_electrical.c:153-169`), and shrunk by 0.75 on the way down
//!    (`spring_electrical.c:1159`). A port that fixes `K` once lays out at the wrong scale.
//! 3. **The step control holds still while the force norm is within 5% of the previous one**
//!    (`spring_electrical.c:179`). Without that hold the step decays monotonically and the
//!    layout stops short of its own convergence test.
//!
//! **And one behaviour is deliberately *not* reproduced, because it cannot be**: the reference
//! draws a random permutation to order its coarsening matchings and re-`srand`s between levels.
//! `docs/measurements/p13-gv2-sfdp.md` records what that costs — the oracle compared *against
//! itself* at `-Gstart` 7 rather than 1 differs from its `-Gstart` 1 output by up to 292 points
//! on the metric the differential uses, so no port that does not draw glibc's exact permutation
//! stream can land inside a tolerance far below that. The row is `Status::Implemented` and the
//! measured gaps are recorded, rather than the ceiling being widened to make it pass.
//!
//! Determinism (D1-D10): the only randomness is the seeded generator in `start`, every force is
//! gathered (node `i` reads positions and writes only its own), every reduction runs in dense
//! index order, and no hash map is ever iterated. Native and wasm32 outputs are bit-identical.
//!
//! Ponytail: **the coarsening matching is deterministic where the reference's is random**, and
//! **two refinement steps are simplified**. Each pass groups nodes with identical neighbour
//! sets first, four at a time, then matches every other node to an unmatched neighbour, as the
//! reference's `maximal_independent_edge_set_heavest_edge_pernode_supernodes_first` does; but
//! it visits nodes in dense index order where the reference draws a random permutation
//! (`gv_permutation`) and re-`srand`s between levels, and with unit weights "heaviest" is the
//! first neighbour. Prolongation copies the coarse position and adds a 1e-6 jitter, without the
//! reference's `interpolate_coord` smoothing pass, and `p` stays -1 where the reference switches
//! to -1.8 on a power-law degree distribution. Failing input: every graph, in the last digits;
//! a power-law graph by more, its hubs packed tighter than Graphviz packs them. Direction: a
//! different but equally valid sfdp layout, never a collapsed one (`tests.rs` checks a 400-node
//! graph spreads in both axes with distinct positions). The two-node case keeps a residual
//! rotation (0.04 rad): two nodes sit in Barnes-Hut cells with different centres of mass, so
//! their forces are only nearly antiparallel. Escape hatch: `run_seeded` is the seam; a port
//! that drew glibc's permutation stream would only have to replace `multilevel::coarsen` and
//! re-`srand` per level, and the differential would then be a check on one number rather than
//! a measurement of an unmatchable one.

mod force;
mod matching;
mod multilevel;
mod quadtree;
mod solve;
mod start;

#[cfg(test)]
mod tests;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::coords::point_geometry;
use crate::stage::StageError;
use multilevel::{COARSEST_FLOOR, Level};

/// The capability id, and the hash gate's stage name.
pub const ID: &str = "layout.force.sfdp";

/// `POINTS_PER_INCH` (`lib/common/const.h`): the reference computes in inches and reports
/// points, so this is the only conversion this module needs.
const POINTS_PER_INCH: f64 = 72.0;

/// Graphviz's default node box, 0.75 x 0.5 inch (`lib/common/const.h`), in points. The
/// translation `-Tplain` applies puts the drawing's lower-left *node-box* corner at the
/// origin, so half a box is the offset between the node-centre frame the solver works in and
/// the frame the oracle prints.
const NODE_W: f64 = 0.75 * POINTS_PER_INCH;
const NODE_H: f64 = 0.5 * POINTS_PER_INCH;

/// The seed the differential pins: Graphviz's own default for `sfdp`
/// (`spring_electrical.c:61`, `ctrl.random_seed = 123` is overwritten by the `start` attribute,
/// whose default the harness sets to 1).
pub const DEFAULT_SEED: u32 = 1;

/// The iteration budget for the finest level, which is the one that decides the drawing.
const MAX_ITER: u32 = 500;

/// `layout.force.sfdp` at Graphviz's defaults and `-Gstart=1`.
///
/// No `Stage` impl and no `Params`, by the same decision `layout::radial::twopi` records: the
/// reference exposes `K`, `levels`, `quadtree`, `smoothing` and `rotation`, none of which the
/// ledger row or the oracle sets, so at its defaults the layout is a pure function of the
/// graph and the seed. `run_seeded` is the one seed-varying entry point, and it is public
/// because the differential needs it — not because it is a knob on the registry row.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    run_seeded(topology, DEFAULT_SEED)
}

/// `layout.force.sfdp` from `seed`'s random start, the way `-Gstart=<seed>` moves Graphviz's.
///
/// Exposed so the seed-sensitivity of the differential can be measured from the same code the
/// registry row runs, rather than from a second implementation that might not be the same one.
pub fn run_seeded(topology: &Topology, seed: u32) -> Result<Geometry, StageError> {
    let count = topology.node_count();
    if count == 0 {
        return Ok(point_geometry(&[], &[]));
    }
    let edges = symmetrised(topology);
    let (x, y) = layout(&edges, count, seed);
    let (x, y) = into_points(&x, &y);
    Ok(point_geometry(&x, &y))
}

/// The undirected edge list, deduplicated and self-loop free, sorted by `(low, high)` endpoint.
///
/// The reference symmetrises the adjacency matrix before laying it out
/// (`spring_electrical.c:1075-1078`) and removes the diagonal, so a self loop contributes
/// nothing and an edge is one spring however it was declared.
fn symmetrised(topology: &Topology) -> Vec<(u32, u32)> {
    let columns = topology.edges();
    let mut out: Vec<(u32, u32)> = (0..topology.edge_count() as usize)
        .map(|i| (columns.source[i], columns.target[i]))
        .filter(|&(a, b)| a != b)
        .map(|(a, b)| (a.min(b), a.max(b)))
        .collect();
    // Sorted rather than `contains`-checked: the scan was O(E^2) and is the order the
    // per-level edge lists use anyway (`multilevel::coarse_edges`).
    out.sort_unstable();
    out.dedup();
    out
}

/// The whole multilevel solve: coarsen to the floor, lay out, then refine back down.
///
/// Coarsening stops at `COARSEST_FLOOR` nodes or when a level makes no progress, so the loop
/// terminates for any input.
fn layout(edges: &[(u32, u32)], count: u32, seed: u32) -> (Vec<f64>, Vec<f64>) {
    let mut coarse = edges.to_vec();
    let mut coarse_count = count;
    // One entry per level above the finest, coarsest last. A level's node ids are its own
    // dense ids, so each level is relaxed against its own edge list.
    let mut levels: Vec<Step> = Vec::new();
    while coarse_count > COARSEST_FLOOR {
        let next = multilevel::coarsen(coarse_count, &coarse);
        if next.coarse >= coarse_count {
            break;
        }
        let above = multilevel::coarse_edges(&next, &coarse);
        coarse_count = next.coarse;
        levels.push(Step {
            level: next,
            edges: std::mem::replace(&mut coarse, above),
        });
    }
    // The random start covers the coarsest level only (`xc` in `spring_electrical.c:1108`). On
    // 2026-10-01 it covered every fine node, so the coarsest solve carried the surplus as
    // phantom nodes with no edges.
    let (x, y) = solve::random_start(coarse_count, seed);
    let mut solve = solve::Solve::new(x, y, &coarse);
    solve.relax(solve::FIRST_STEP, MAX_ITER);
    // Walk back down, relaxing each level against its own edges. `K` shrinks by 0.75 at each
    // step down (`spring_electrical.c:1159`), which is what keeps a fine level's attraction in
    // scale with the coarse solution it was prolonged from.
    let mut k = solve.k();
    for step in levels.iter().rev() {
        let count = step.level.pair.len() as u32;
        let (nx, ny) = multilevel::prolongate(&solve.x, &solve.y, &step.level, count, seed);
        k = multilevel::decay_k(k);
        let mut next = solve::Solve::with_k(nx, ny, &step.edges, k);
        next.relax(solve::FIRST_STEP, MAX_ITER);
        solve = next;
    }
    (solve.x, solve.y)
}

/// One level of the hierarchy, with the edge list of its **finer** side: the graph its prolonged
/// positions are relaxed against. On 2026-10-01 it carried the coarser side's edges, so every
/// level, the finest included, was relaxed against the graph one level up.
struct Step {
    level: Level,
    edges: Vec<(u32, u32)>,
}

/// The solver's frame onto the frame `-Tplain` prints: scale by the node box and translate so
/// the drawing's lower-left node-box corner is the origin.
///
/// Graphviz's `translate_drawing` does this after layout (`lib/common/output.c`), and the
/// differential's rescale cannot undo it, so the port applies it rather than leaving the two
/// arms a half-node box apart on every axis.
fn into_points(x: &[f64], y: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let (mut low_x, mut high_x) = (f64::MAX, f64::MIN);
    let (mut low_y, mut high_y) = (f64::MAX, f64::MIN);
    for i in 0..x.len() {
        low_x = low_x.min(x[i]);
        high_x = high_x.max(x[i]);
        low_y = low_y.min(y[i]);
        high_y = high_y.max(y[i]);
    }
    if !low_x.is_finite() {
        return (x.to_vec(), y.to_vec());
    }
    let span_x = (high_x - low_x).max(1e-9);
    let span_y = (high_y - low_y).max(1e-9);
    // One uniform scale, from the larger axis, so an aspect-ratio error stays visible instead
    // of being squashed out by a per-axis map.
    let scale = (NODE_W + NODE_H) / span_x.max(span_y);
    let ox = NODE_W / 2.0 - low_x * scale;
    let oy = NODE_H / 2.0 - low_y * scale;
    let px: Vec<f64> = x.iter().map(|v| v * scale + ox).collect();
    let py: Vec<f64> = y.iter().map(|v| v * scale + oy).collect();
    (px, py)
}
