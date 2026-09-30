//! The threaded arm's own stage bytes: which stages `--tiers all` **recomputes** under
//! `std::thread`s, and the negative control carried into them.
//!
//! Split out of `hashgate.rs` for the house's 300-line cap, and because the claim is worth
//! its own file: a stage this module does not list has a **vacuous** "N-way equal" — the
//! arm hashes the scalar run's bytes and nothing was compared. That is the failure mode
//! `FORCE_STAGES` and its test exist to make impossible to reintroduce silently.

use super::knob::Setting;
use super::stages;
use crate::exec_native::Threads;
use graph_core::Stage as _;
use graph_core::layout::force::{BarnesHut, YifanHu};

/// The stages the threaded arm recomputes rather than reusing: the two force layouts, the
/// only two a [`graph_core::exec::Runner`] reaches, because their ticks hand it gathered
/// passes.
///
/// A named list, not an inline match, so a test can hold it against what the arm actually
/// compared. `YifanHu` is here on the same evidence that put its settle under a runner at
/// all: it runs the same three gathered passes through the same `Sim::tick`, just once per
/// level.
pub(crate) const FORCE_STAGES: [&str; 2] = [BarnesHut::ID, YifanHu::ID];

/// [`stage_bytes`](stages::stage_bytes) with the [`FORCE_STAGES`] run threaded, and with
/// `setting.split_sum` — the negative control — carried into it.
///
/// The control reaches the *stage*, not just the arm, so `GM_MUTATE_SPLIT_SUM=1 --tiers all`
/// diverges the threaded arms from the scalar one on the force stages and nowhere else: a
/// mutation a threaded arm cannot survive, so the gate's red is proof the arms were compared.
pub(super) fn stage_bytes_threaded(
    seed: u32,
    setting: &Setting,
    workers: u32,
) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    let count = graph_core::gate_node_count(seed) + setting.extra_nodes;
    let (nodes, edges) = graph_core::seeded_model(seed, count, setting.reference_degree);
    let topology = graph_core::index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (id, bytes) in stages::stage_bytes(seed, setting)? {
        let bytes = if FORCE_STAGES.contains(&id) {
            threaded_force(id, &topology, setting, workers)?
        } else {
            bytes
        };
        out.push((id, bytes));
    }
    Ok(out)
}

/// The force stage `id` names over `workers` `std::thread`s, as snapshot bytes.
///
/// One function for both force stages, so the branches cannot drift apart in anything but
/// which stage they construct: an arm that threaded one and reused the other's scalar
/// bytes would still report "equal", and the report would be the evidence that hid it.
fn threaded_force(
    id: &str,
    topology: &graph_core::Topology,
    setting: &Setting,
    workers: u32,
) -> Result<Vec<u8>, String> {
    let geometry = if id == BarnesHut::ID {
        BarnesHut::run_under(
            topology,
            &setting.force,
            &Threads,
            workers,
            setting.split_sum,
        )
    } else {
        YifanHu::run_under(
            topology,
            &setting.force,
            &Threads,
            workers,
            setting.split_sum,
        )
    }
    .map_err(|e| e.to_string())?;
    graph_core::layout::snapshot(topology, geometry)
        .map(|snapshot| snapshot.to_bytes())
        .map_err(|e| e.to_string())
}
