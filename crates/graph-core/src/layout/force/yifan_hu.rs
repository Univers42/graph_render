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
use crate::layout::spectral::last_axis;
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

/// `layout.force.yifan_hu.2z`: the 2D multilevel layout, then a third axis derived from
/// graph structure — SciGraphs' `'2Z'` mode (`yifan_hu.py:346`, `:327-333`).
///
/// **Not a dimension of the force run.** `'2Z'` is `'2'` in the plane plus a `z` from
/// `_generate_z_component`, which reads the graph and never the simulation: the last axis
/// of a 2D spectral solve per component, peak-normalised, centred, scaled. So this arm is
/// the 2D kernel unchanged plus one column, and it is its own id because it is a different
/// picture and a different claim.
pub const ID_2Z: &str = "layout.force.yifan_hu.2z";

/// `_sfdp_z_scale`, the reference default (`yifan_hu.py:331`).
const Z_SCALE: f64 = 0.3;

/// The 2Z arm: [`YifanHu::run`] over the same 2D kernel, then the derived column.
///
/// Separate from `run_under` because the z is added *after* the whole multilevel run and
/// after its finiteness check, exactly as the reference adds it after sfdp returns
/// (`yifan_hu.py:327-333`); putting it inside the descent would let z steer the layout,
/// which the reference never does.
pub fn run_2z(topology: &Topology, params: &ForceParams) -> Result<Geometry, StageError> {
    let flat = <YifanHu as Stage>::run(topology, params)?;
    let z = derived_z(topology, flat.extent());
    let NodeGeometry::Point { x, y } = flat.nodes else {
        return Err(StageError::NonFinite { column: "node.z" });
    };
    if z.len() != x.len() {
        return Err(StageError::NonFinite { column: "node.z" });
    }
    Ok(Geometry::in_space(
        NodeGeometry::Point { x, y },
        flat.edges,
        flat.notes,
        z,
    ))
}

/// The z column: `_generate_z_component(..., 'SPECTRAL')`, then the reference's own
/// `z * z_scale * scale` (`yifan_hu.py:330-333`).
///
/// Three departures, each because a step the reference takes does not exist here:
///
/// - **the scale is the layout's own extent**, where the reference passes its `scale`
///   argument: our 2D port has no rescale step, so the ratio the reference means by
///   `z * z_scale * scale` — z is 0.3 of the drawing — is kept against the extent the
///   drawing actually has. A degenerate extent (a single node, or a layout that did not
///   move) yields a column of zeros rather than a division by zero.
/// - **a failed solve falls back to degree**, which is what `_generate_z_component`'s own
///   `except` branch does (`networkx_layouts.py:336-338`). Refusing instead would fail
///   `2Z` on exactly the graphs the 2D arm already lays out, for want of a third axis.
/// - **the peak normalisation is per component**, as the reference's loop over
///   `components` is (`:330-334`); that is [`last_axis`]'s own job, not this one's.
fn derived_z(topology: &Topology, extent: f32) -> Vec<f32> {
    let graph = simple_graph(topology);
    let raw = last_axis(topology).unwrap_or_else(|| degrees(&graph));
    crate::layout::spectral::center_z(&raw)
        .into_iter()
        .map(|v| (v * Z_SCALE * f64::from(extent)) as f32)
        .collect()
}

/// The reference's `DEGREE` fallback: `G.degree(n)` less two per self-loop
/// (`networkx_layouts.py:311-312`). Read over the shared [`SimpleGraph`], which has
/// already dropped the self-loops, so the subtraction is already done.
fn degrees(graph: &SimpleGraph) -> Vec<f64> {
    (0..graph.rows.rows())
        .map(|v| f64::from(graph.degree(v)))
        .collect()
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
