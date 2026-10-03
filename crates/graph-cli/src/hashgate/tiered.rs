//! The threaded arm's own stage bytes: which stages `--tiers all` **recomputes** under
//! `std::thread`s, and the negative controls carried into them.
//!
//! Split out of `hashgate.rs` for the house's 300-line cap, and because the claim is worth
//! its own file: a stage this module does not list has a **vacuous** "N-way equal" — the
//! arm hashes the scalar run's bytes and nothing was compared. That is the failure mode
//! `THREADED_STAGES` and its test exist to make impossible to reintroduce silently.

use super::knob::Setting;
use super::stages;
use crate::exec_native::Threads;
use graph_core::Stage as _;
use graph_core::layout::force::{BarnesHut, ParticleMesh, YifanHu};
use graph_core::layout::{circular::ring, spiral};
use graph_core::{Grid, Topology};

/// The stages the threaded arm recomputes rather than reusing: every layout whose work a
/// [`graph_core::exec::Runner`] reaches, because their hot loop hands it range kernels or
/// gathered passes.
///
/// A named list, not an inline match, so a test can hold it against what the arm actually
/// compared. The three force layouts are here on the same evidence that put their settles
/// under a runner at all: they run the same three gathered passes through the same
/// `Sim::tick`, once per level for the multilevel one. The closed-form point layouts end
/// in one shared `coords` merge, which is the merge `GM_MUTATE_SPLIT_RESCALE` splits.
pub(crate) const THREADED_STAGES: [&str; 6] = [
    BarnesHut::ID,
    YifanHu::ID,
    ParticleMesh::ID,
    Grid::ID,
    ring::ID,
    spiral::ID,
];

/// [`stage_bytes`](stages::stage_bytes) with the [`THREADED_STAGES`] run threaded, and with
/// both compute-tier controls carried into it.
///
/// Each control reaches the *stage*, not just the arm, so `GM_MUTATE_SPLIT_SUM=1 --tiers all`
/// diverges the threaded arms on the three force stages and `GM_MUTATE_SPLIT_RESCALE=1` on the
/// closed-form point layouts — and on no other stage either way. That is the shape the
/// phase prompt asks a control to have: a mutation a threaded arm cannot survive, so the
/// gate's red is proof the arms were compared.
pub(crate) fn stage_bytes_threaded(
    seed: u32,
    setting: &Setting,
    workers: u32,
) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    let count = graph_core::gate_node_count(seed) + setting.extra_nodes;
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
/// One match for all six stages, and **one arm for the three closed-form layouts, not
/// three**: they share the control and the shape of the claim, and three arms spelling the
/// same routing three times is three places a stage could be added to one and missed in
/// another. Likewise one arm for the three force layouts, so the split control cannot reach
/// one of them and not the others.
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
fn geometry(
    id: &'static str,
    topology: &Topology,
    setting: &Setting,
    workers: u32,
) -> Result<graph_core::layout::Geometry, graph_core::StageError> {
    match id {
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
        ParticleMesh::ID => ParticleMesh::run_under(
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
    }
}
