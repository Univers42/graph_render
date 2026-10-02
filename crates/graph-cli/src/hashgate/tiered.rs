//! The threaded arm's own stage bytes: which stages `--tiers all` **recomputes** under
//! `std::thread`s, and the negative controls carried into them.
//!
//! Split out of `hashgate.rs` for the house's 300-line cap, and because the claim is worth
//! its own file: a stage this module does not list has a **vacuous** "N-way equal" — the
//! arm hashes the scalar run's bytes and nothing was compared. That is the failure mode
//! `THREADED_STAGES` and its test exist to make impossible to reintroduce silently.

#[cfg(test)]
mod tests;

use super::knob::Setting;
use super::stages;
use crate::exec_native::Threads;
use graph_core::Stage as _;
use graph_core::layout::force::{BarnesHut, YifanHu};
use graph_core::layout::{circular::ring, spiral};
use graph_core::{Grid, Topology};

/// The stages the threaded arm recomputes rather than reusing: every layout whose work a
/// [`graph_core::exec::Runner`] reaches, because their hot loop hands it range kernels or
/// gathered passes.
///
/// A named list, not an inline match, so a test can hold it against what the arm actually
/// compared. The two force layouts are here on the same evidence that put their settles
/// under a runner at all: they run the same three gathered passes through the same
/// `Sim::tick`, once per level for the multilevel one. The closed-form point layouts end
/// in one shared `coords` merge, which is the merge `GM_MUTATE_SPLIT_RESCALE` splits.
pub(crate) const THREADED_STAGES: [&str; 5] =
    [BarnesHut::ID, YifanHu::ID, Grid::ID, ring::ID, spiral::ID];

/// [`stage_bytes`](stages::stage_bytes) with the [`THREADED_STAGES`] run threaded, and with
/// both compute-tier controls carried into it.
///
/// Each control reaches the *stage*, not just the arm, so `GM_MUTATE_SPLIT_SUM=1 --tiers all`
/// diverges the threaded arms on the two force stages and `GM_MUTATE_SPLIT_RESCALE=1` on the
/// closed-form point layouts — and on no other stage either way. That is the shape the
/// phase prompt asks a control to have: a mutation a threaded arm cannot survive, so the
/// gate's red is proof the arms were compared.
pub(crate) fn stage_bytes_threaded(
    seed: u32,
    setting: &Setting,
    workers: u32,
) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    let count = stages::node_count(seed, setting, 0)?;
    let (nodes, edges) = graph_core::seeded_model(seed, count, setting.reference_degree);
    let topology = graph_core::index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (id, bytes) in stages::stage_bytes(seed, setting)? {
        let bytes = if THREADED_STAGES.contains(&id) {
            threaded_bytes(id, &topology, setting, workers)?
        } else {
            bytes
        };
        out.push((id, bytes));
    }
    Ok(out)
}

/// The stage `id` names over `workers` `std::thread`s, as snapshot bytes, or `None` for a
/// stage this arm does not recompute — which then keeps the scalar arm's own bytes.
///
/// One match for all five stages, and **one arm for the three closed-form layouts, not
/// three**: they share the control and the shape of the claim, and three arms spelling the
/// same routing three times is three places a stage could be added to one and missed in
/// another. Likewise one arm for the two force layouts, so the split control cannot reach
/// one of them and not the other.
fn threaded_bytes(
    id: &'static str,
    topology: &Topology,
    setting: &Setting,
    workers: u32,
) -> Result<Vec<u8>, String> {
    let geometry = geometry(id, topology, setting, workers).map_err(|e| format!("{id}: {e}"))?;
    graph_core::layout::snapshot(topology, geometry)
        .map(|snapshot| snapshot.to_bytes())
        .map_err(|e| format!("{id}: {e}"))
}

/// The threaded geometry for one of the [`THREADED_STAGES`], at the control its own merge
/// family carries.
///
/// **The last arm is the spiral's *by name*, and the fall-through is an error.** The two
/// arms are the same run and are deliberately not one arm: the spiral has its own
/// `SpiralParams`, its own merge, and its own negative control. What is refused is a stage
/// this arm does not handle — because the fall-through used to compute the spiral for *any*
/// id, so an id appended to [`THREADED_STAGES`] and forgotten here was "recomputed" as the
/// spiral, compared against the scalar arm's own bytes, and reported as an N-way-equal stage
/// while being computed at no width at all. A stage genuinely *outside* the list keeps the
/// other behaviour, which is not here but in [`stage_bytes_threaded`]: it is not recomputed
/// and keeps the scalar arm's bytes. So the two cases stay apart — one is "not threaded", the
/// other is "threaded but unimplemented", and only the second may not pass.
///
/// The error is a [`String`] rather than a [`graph_core::StageError`] because it is not a
/// stage that failed: it is a stage this file has no code for, which no stage-error variant
/// describes. Widening it here costs one `to_string` per stage and lets the refusal name the
/// id.
fn geometry(
    id: &'static str,
    topology: &Topology,
    setting: &Setting,
    workers: u32,
) -> Result<graph_core::layout::Geometry, String> {
    let ran = match id {
        BarnesHut::ID => BarnesHut::run_under(
            topology,
            &setting.force,
            &Threads,
            workers,
            setting.split_sum,
        ),
        YifanHu::ID => YifanHu::run_under(
            topology,
            &setting.force,
            &Threads,
            workers,
            setting.split_sum,
        ),
        Grid::ID => Grid::run_with(topology, &setting.grid, &Threads, workers),
        ring::ID => ring::run_under(topology, &Threads, workers, setting.split_rescale),
        _ => spiral::run_under(
            topology,
            &spiral::SpiralParams::default(),
            &Threads,
            workers,
            setting.split_rescale,
        ),
        other => {
            return Err(format!(
                "{other} has no threaded arm: it cannot be reported as recomputed at any width"
            ));
        }
    };
    ran.map_err(|e| e.to_string())
}
