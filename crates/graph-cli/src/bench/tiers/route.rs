//! Which layout a timed arm is, and the one call per layout that routes it.
//!
//! Split out of `tiers.rs` by the house's 300-line limit. The rule it exists to keep is
//! one sentence long: **`bench --tiers` times the layout `--layout` names, and refuses
//! every other one by name.** Before this, the sweep answered the flag with
//! `BarnesHut::run_under` whatever was asked for, so a `--layout layout.grid --tiers` run
//! measured Barnes-Hut and printed the grid's name nowhere — a speedup column that could
//! not be attributed to a stage.
//!
//! Two things it deliberately does *not* do. It does not resolve `--layout` through the
//! registry: a registered layout with no threaded route is not a tier the sweep can time,
//! and the list below is short enough to be the whole truth. And it does not default an
//! unknown id to Barnes-Hut, because a caller who asked to time the grid and got
//! Barnes-Hut's milliseconds has been lied to; `layouts` refuses it instead.

use super::Tier;
use crate::bench::Plan;
use crate::exec_native::Threads;
use graph_core::exec::Serial;
use graph_core::layout::Geometry;
use graph_core::layout::force::{BarnesHut, ForceParams, Split, YifanHu};
use graph_core::layout::{circular::ring, spiral};
use graph_core::{Grid, GridParams, Stage, StageError, Topology};

/// Every layout `bench --tiers` can time, in the order a report prints them.
///
/// Barnes-Hut is first because it is the **default** (see [`layouts`]), and it is the row
/// every `docs/measurements/phase11-threads.md` number was measured on. The next two are
/// the force stages, which reach a runner through the same three gathered passes per tick
/// — the multilevel solve once per coarsening level. The last three are the stages whose
/// hot loop is a per-node gather with no reduction crossing elements, so a width is a
/// schedule of the same computation.
pub const ROUTES: [&str; 5] = [BarnesHut::ID, YifanHu::ID, Grid::ID, ring::ID, spiral::ID];

/// The layouts the plan asks for, or the error naming the ones it can time.
///
/// Barnes-Hut when the plan named none, which is what keeps every row of
/// `docs/measurements/phase11-threads.md` reproducible with no `--layout` at all: the
/// default arm is the one that was measured, not a fallback for a name that did not
/// resolve.
pub fn layouts(plan: &Plan) -> Result<Vec<&'static str>, String> {
    if plan.layouts.is_empty() {
        return Ok(vec![ROUTES[0]]);
    }
    plan.layouts
        .iter()
        .map(|id| {
            ROUTES
                .iter()
                .copied()
                .find(|known| *known == id)
                .ok_or_else(|| {
                    format!(
                        "--layout {id}: bench --tiers times {} only",
                        ROUTES.join(", ")
                    )
                })
        })
        .collect()
}

/// One stage run under `tier`, with the negative controls as arguments and never a second
/// code path: the control a test calls is the control the gate reaches.
///
/// Every arm is the same call its serial twin makes with [`Serial`] and one worker, so each
/// row is a **schedule** of the computation the serial row measured rather than a second
/// implementation of it. That is the whole basis of the `equal` column, and it is why the
/// controls are arguments here instead of a second pair of functions.
pub fn run_once(
    topology: &Topology,
    id: &'static str,
    tier: Tier,
    control: Control,
) -> Result<Geometry, String> {
    let geometry = match (id, tier) {
        (BarnesHut::ID, Tier::Scalar) => barnes_hut(topology, &Serial, 1, control),
        (BarnesHut::ID, Tier::Threads(w)) => barnes_hut(topology, &Threads, w, control),
        (YifanHu::ID, Tier::Scalar) => yifan_hu(topology, &Serial, 1, control),
        (YifanHu::ID, Tier::Threads(w)) => yifan_hu(topology, &Threads, w, control),
        (Grid::ID, Tier::Scalar) => grid(topology, &Serial, 1),
        (Grid::ID, Tier::Threads(w)) => grid(topology, &Threads, w),
        (ring::ID, Tier::Scalar) => ring(topology, &Serial, 1, control),
        (ring::ID, Tier::Threads(w)) => ring(topology, &Threads, w, control),
        (spiral::ID, Tier::Scalar) => spiral(topology, &Serial, 1, control),
        (spiral::ID, Tier::Threads(w)) => spiral(topology, &Threads, w, control),
        (_, tier) => return Err(format!("no tier route ({tier:?})")),
    };
    geometry.map_err(|e| format!("{id} {tier:?}: {e}"))
}

fn barnes_hut(
    topology: &Topology,
    runner: &impl graph_core::exec::Runner,
    workers: u32,
    control: Control,
) -> Result<Geometry, StageError> {
    BarnesHut::run_under(
        topology,
        &ForceParams::default(),
        runner,
        workers,
        control.split,
    )
}

/// The multilevel solve: the same `ForceParams`, the same `Split` control, and one settle
/// per coarsening level — so a width is a schedule of the whole hierarchy, not of its
/// coarsest level alone.
fn yifan_hu(
    topology: &Topology,
    runner: &impl graph_core::exec::Runner,
    workers: u32,
    control: Control,
) -> Result<Geometry, StageError> {
    YifanHu::run_under(
        topology,
        &ForceParams::default(),
        runner,
        workers,
        control.split,
    )
}

fn grid(
    topology: &Topology,
    runner: &impl graph_core::exec::Runner,
    workers: u32,
) -> Result<Geometry, StageError> {
    Grid::run_with(topology, &GridParams::default(), runner, workers)
}

fn ring(
    topology: &Topology,
    runner: &impl graph_core::exec::Runner,
    workers: u32,
    control: Control,
) -> Result<Geometry, StageError> {
    ring::run_under(topology, runner, workers, control.split_rescale)
}

fn spiral(
    topology: &Topology,
    runner: &impl graph_core::exec::Runner,
    workers: u32,
    control: Control,
) -> Result<Geometry, StageError> {
    spiral::run_under(
        topology,
        &spiral::SpiralParams::default(),
        runner,
        workers,
        control.split_rescale,
    )
}

/// The negative controls a sweep arm runs under, and the one the test entry point passes.
///
/// A struct rather than two more parameters, and the reason is the house's four-parameter
/// ceiling: `time` needs the topology, the layout, the tier and these, and a fifth argument
/// is the one that would let the arm list and the controls drift apart in a call site.
#[derive(Debug, Clone, Copy, Default)]
pub struct Control {
    /// The force stages' shared one: which of the tick's range-kernel merges the control
    /// splits, for the single-level solve and for every level of the multilevel one.
    pub split: Split,
    /// The closed-form layouts' shared `rescale` merge — one merge, so a flag and not a
    /// [`Split`], and inert for the two stages that do not end in one.
    pub split_rescale: bool,
}

impl Control {
    /// The honest setting: no control anywhere, and what a scalar arm always runs under.
    pub const HONEST: Self = Self {
        split: Split::None,
        split_rescale: false,
    };
}
