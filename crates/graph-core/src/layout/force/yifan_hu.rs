//! `layout.force.yifan_hu`: multilevel force layout. The graph is coarsened by repeated
//! matching, the coarsest level is laid out by the Barnes-Hut force simulation, and each
//! finer level starts from its parent's positions and is refined by the same simulation.
//!
//! This is NOT Graphviz `sfdp`. It borrows sfdp's multilevel idea (Hu 2005) and nothing
//! else: sfdp's own force model (the `K^2 / d` repulsion, adaptive step, its coarsening
//! by edge collapsing with weights) is not reproduced, and no `sfdp` output was
//! compared against. The per-level solver is `layout::force::barnes_hut`, reused, not a
//! second tick (`prompts/phase-06-iterative-spectral-mds.md`, section 4).
//!
//! **Coarsening and prolongation stay serial, and cannot be anything else:** coarsening
//! is a greedy matching whose quotient must exist before the next level can be built from
//! it, and every refinement starts from its parent's positions, so the levels are a
//! chain rather than a set of independent jobs. What is handed to a runner is the set of
//! settle ticks between them — the same three gathered passes per tick that
//! `layout.force.barnes_hut` runs, which is where essentially all of this stage's time
//! goes (`registry/force.rs`'s `O(n log n) × (112 + 48 × levels)` sizing).

mod coarsen;
#[cfg(test)]
mod tests;

use super::barnes_hut::{Split, Tier, golden_seed, settle};
use super::params::{ForceParams, TICKS};
use super::{SimpleGraph, simple_graph};
use crate::exec::Serial;
use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::{Stage, StageError};
use coarsen::coarsen;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// Coarsening stops at or below this many nodes.
const MIN_COARSE: u32 = 16;
/// Hard bound on the number of coarsening steps.
const MAX_LEVELS: usize = 12;
/// Ticks and starting heat for each refinement of a finer level.
const REFINE: (u32, f64) = (48, 0.3);
const GOLDEN_ANGLE: f64 = 2.399963229728653;

/// Multilevel force layout over the Barnes-Hut simulation.
///
/// Ponytail: coarsening is a greedy matching and the refinement budget (48 ticks from
/// alpha 0.3) is fixed, not adaptive; a long, thin graph whose coarse layout is folded
/// can keep the fold, since refinement is local. It is not sfdp, so positions and
/// scale differ from Graphviz's for the same graph. Direction: cosmetic. Escape hatch:
/// `layout.force.barnes_hut` for a single-level solve.
pub struct YifanHu;

impl Stage for YifanHu {
    type Params = ForceParams;
    const ID: &'static str = "layout.force.yifan_hu";

    /// The serial tier, which is the stage: [`run_with`](Self::run_with) over
    /// [`Serial`] with one worker, and the arm every other runner must hash-equal.
    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        Self::run_with(topology, params, &Serial, 1)
    }
}

impl YifanHu {
    /// The same layout, with every level's ticks handed to `runner` over `workers` workers.
    ///
    /// The point of the signature is what it does **not** change: the level hierarchy, the
    /// greedy coarsening, each refinement's restart from its parent and the per-level tick
    /// budget are all the same code the serial stage runs, and only the division of the
    /// three gathered passes per tick differs. So `run_with(..., &Serial, 1)` is
    /// [`Stage::run`] by construction, and any other runner is a *schedule* of the same
    /// computation — which is the claim the N-way hash gate checks and
    /// `yifan_hu/tests.rs` pins at the unit level.
    ///
    /// `workers` below 2 is the serial path (see [`crate::exec::Runner`]), so a host
    /// reporting no threads gets the same bytes rather than a fast failure.
    pub fn run_with(
        topology: &Topology,
        params: &ForceParams,
        runner: &impl crate::exec::Runner,
        workers: u32,
    ) -> Result<Geometry, StageError> {
        Self::run_under(topology, params, runner, workers, Split::None)
    }

    /// [`run_with`](Self::run_with) with the negative control reachable, so the host can
    /// run a *deliberately wrong* tier and the gate must go red. Separate from
    /// [`run_with`](Self::run_with) for the reason
    /// [`BarnesHut::run_under`](super::BarnesHut::run_under) is separate from its
    /// `run_with`: a control a caller can forget to pass is not a control.
    pub fn run_under(
        topology: &Topology,
        params: &ForceParams,
        runner: &impl crate::exec::Runner,
        workers: u32,
        split: Split,
    ) -> Result<Geometry, StageError> {
        let graph = simple_graph(topology);
        let n = topology.node_count();
        let tier = Tier {
            runner,
            workers,
            split,
        };
        positions(multilevel(graph, n, *params, tier))
    }
}

/// The stage's own points, refused if any coordinate is not finite.
fn positions((x, y): (Vec<f64>, Vec<f64>)) -> Result<Geometry, StageError> {
    if x.iter().chain(&y).any(|v| !v.is_finite()) {
        return Err(StageError::NonFinite { column: "node.x" });
    }
    Ok(Geometry::planar(
        NodeGeometry::Point {
            x: x.iter().map(|&v| v as f32).collect(),
            y: y.iter().map(|&v| v as f32).collect(),
        },
        EdgeGeometry::Line,
        Vec::new(),
    ))
}

/// Graphs finest first, and the map from each level's nodes to the next one's.
fn hierarchy(fine: SimpleGraph, n: u32) -> (Vec<(SimpleGraph, u32)>, Vec<Vec<u32>>) {
    let mut levels = vec![(fine, n)];
    let mut maps = Vec::new();
    while levels.len() < MAX_LEVELS {
        let (g, count) = levels.last().expect("one level");
        // Stall guard: under 10 percent shrink is not worth another level.
        if *count <= MIN_COARSE {
            break;
        }
        let c = coarsen(g, *count);
        if u64::from(c.node_count) * 10 > u64::from(*count) * 9 {
            break;
        }
        maps.push(c.map);
        levels.push((c.graph, c.node_count));
    }
    (levels, maps)
}

/// The levels, each settled under the same `tier`: the coarse solve first, then every
/// refinement from its parent's positions. Sequential in the levels by construction — the
/// loop is the reason `tier` is a parameter here rather than a per-level choice.
fn multilevel<R: crate::exec::Runner>(
    fine: SimpleGraph,
    n: u32,
    params: ForceParams,
    tier: Tier<'_, R>,
) -> (Vec<f64>, Vec<f64>) {
    let (levels, maps) = hierarchy(fine, n);
    let (top, top_n) = levels.last().expect("one level").clone();
    let mut pos = settle(top, params, golden_seed(top_n), (TICKS, 1.0), tier);
    for k in (0..maps.len()).rev() {
        let start = prolong(&pos, &maps[k], params.link_distance);
        pos = settle(levels[k].0.clone(), params, start, REFINE, tier);
    }
    pos
}

/// Each fine node starts at its coarse parent, nudged by a golden-angle offset of its
/// own index so two children of one parent do not coincide.
fn prolong(coarse: &(Vec<f64>, Vec<f64>), map: &[u32], link_distance: f64) -> (Vec<f64>, Vec<f64>) {
    let mut x = Vec::with_capacity(map.len());
    let mut y = Vec::with_capacity(map.len());
    for (i, &p) in map.iter().enumerate() {
        let angle = i as f64 * GOLDEN_ANGLE;
        x.push(coarse.0[p as usize] + 0.25 * link_distance * libm::cos(angle));
        y.push(coarse.1[p as usize] + 0.25 * link_distance * libm::sin(angle));
    }
    (x, y)
}
