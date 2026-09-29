//! The stages the hash gate hashes, and the bytes the native arm hashes for each
//! (`prompt.md` §7.1). Split out of `hashgate.rs` by the house's 300-line limit.
//!
//! The list is registry-driven (C1): the topology, then every layout of
//! `graph_core::registry::LAYOUTS`, so a new layout joins the gate with no change here,
//! then the transport — the real ABI over the same model, a stage of the gate in its
//! own right (C20).
//!
//! **The native arm's bytes are driven by the same list.** `stages()` and
//! [`stage_bytes`] are one derivation, not two: an arm that printed a line for every
//! stage the gate asked for and nothing else is what makes the per-stage comparison
//! meaningful, and a second layout registered in `LAYOUTS` therefore joins the gate by
//! being registered, with no edit to this file. [`stage_bytes_for`] takes the registry
//! slice as an argument so that is testable here rather than only at the next merge.

use super::Setting;
use graph_core::layout::Geometry;
use graph_core::layout::circle_packing;
use graph_core::layout::force::BarnesHut;
use graph_core::layout::forceatlas2::ForceAtlas2;
use graph_core::registry::{self as core, LAYOUTS};
use graph_core::{
    Grid, Stage, StageError, Sugiyama, Topology, gate_node_count, index_model, run_pipeline,
    seeded_model,
};
use std::collections::BTreeSet;

/// The layout the transport stage runs: `harness/wasm-run.mjs`'s `abiSnapshotBytes`
/// drives `gm_run` with exactly this one, so the transport stage is the real ABI over
/// this layout and nothing else.
pub const LAYOUT: &str = "layout.grid";

/// The Phase 3 hierarchy layouts' stage ids, named here for the knobs that perturb one of
/// them. `graph_core::registry` spells them as literals inside its own `LAYOUTS` and they
/// are not `Stage` impls, so there is no `Stage::ID` to take: these four constants and the
/// registry are the two places the ids exist, and
/// `the_p3_stage_ids_are_the_registry_s_own` is what keeps them in step.
pub const TIDY_TREE: &str = "layout.tree.tidy";
/// The squarified treemap's stage id — see [`TIDY_TREE`].
pub const TREEMAP: &str = "layout.treemap.squarified";
/// The circular layout's stage id — see [`TIDY_TREE`].
pub const CIRCULAR: &str = "layout.circular.radial";
/// Circle packing's stage id — see [`TIDY_TREE`].
pub const PACKING: &str = "layout.packing.circle";

/// The transport stage: `gm_seed_ingest → gm_alloc → gm_build → gm_run →
/// gm_snapshot_bytes` over the gate's own model — the real ABI, not the retained shim.
pub const TRANSPORT: &str = "transport.wasm.columnar";

/// Every stage, in the order both arms print them.
pub fn stages() -> Vec<&'static str> {
    let mut stages = vec!["topology"];
    stages.extend(LAYOUTS.iter().map(|layout| layout.id));
    stages.push(TRANSPORT);
    stages
}

/// Every stage's id and the bytes the native arm hashes for `seed`, in [`stages`] order.
pub fn stage_bytes(seed: u32, setting: &Setting) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    stage_bytes_for(seed, setting, &LAYOUTS)
}

/// [`stage_bytes`] over an explicit registry slice, so a test can stand a second
/// registered layout beside the grid without editing `graph_core`.
///
/// The grid's own stage is the perturbed `run_pipeline` run rather than the registry's
/// `run` closure, because `GM_MUTATE_GRID_SPACING` has to reach it — a hashed snapshot is
/// pinned to a *stated* set of parameters, and the grid is the one layout the gate has a
/// control for. Every other layout is hashed from `(layout.run)(&topology)` over the same
/// topology, which is byte for byte what `graph_core::run_with` would produce for it.
pub fn stage_bytes_for(
    seed: u32,
    setting: &Setting,
    layouts: &[core::Capability],
) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    check(layouts)?;
    let count = gate_node_count(seed) + setting.extra_nodes;
    let (nodes, edges) = seeded_model(seed, count, setting.reference_degree);
    let grid = run_pipeline::<Grid>(&nodes, &edges, &setting.grid).map_err(|e| e.to_string())?;
    if grid.layout != LAYOUT {
        return Err(format!("the pipeline ran {}, not {LAYOUT}", grid.layout));
    }
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let mut out = vec![("topology", grid.topology)];
    for layout in layouts {
        // The knob-aware stages run at `setting`'s parameters rather than the registry's
        // compiled-in default; the wasm arm cannot see the knobs, which is exactly the
        // divergence a wired control must surface.
        let bytes = match layout.id {
            LAYOUT => grid.snapshot.to_bytes(),
            BarnesHut::ID => run_force(&topology, |t| BarnesHut::run(t, &setting.force))?,
            ForceAtlas2::ID => run_force(&topology, |t| ForceAtlas2::run(t, &setting.fa2))?,
            Sugiyama::ID => run_pipeline::<Sugiyama>(&nodes, &edges, &setting.sugiyama)
                .map_err(|e| e.to_string())?
                .snapshot
                .to_bytes(),
            PACKING => run_force(&topology, |t| circle_packing::run_with(t, &setting.packing))?,
            _ if owns_own_model(layout.id, setting) => {
                stage_bytes_from_own_model(seed, setting, layout)?
            }
            _ => layout_bytes(&topology, layout)?,
        };
        out.push((layout.id, bytes));
    }
    out.push((TRANSPORT, grid.snapshot.to_bytes()));
    Ok(out)
}

/// A stage list the gate cannot print: no stage id twice (a repeated id would fold into
/// one key in the record and make the per-stage counts lie), and no missing [`LAYOUT`]
/// (the transport stage restates that layout's bytes, so without it there is nothing to
/// restate).
fn check(layouts: &[core::Capability]) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for layout in layouts {
        if !seen.insert(layout.id) {
            return Err(format!("{} appears twice in the registry", layout.id));
        }
    }
    if !layouts.iter().any(|layout| layout.id == LAYOUT) {
        return Err(format!(
            "the {TRANSPORT} stage restates {LAYOUT}, which is not registered"
        ));
    }
    Ok(())
}

/// Whether `id` is the one Phase 3 stage whose own model `setting` re-draws with extra
/// nodes ([`Setting::stage_nodes`]).
///
/// The perturbation is deliberately *not* the shared `extra_nodes`: that one grows the
/// gate's single model, so every stage that is a function of the topology moves with it
/// and the gate can no longer say which stage a divergence came from. This one re-draws
/// the model for the stage named in the setting alone, so `topology`, the other layouts
/// and the transport stage are byte-identical and the one stage that moved is named in
/// the gate's own output.
fn owns_own_model(id: &str, setting: &Setting) -> bool {
    setting.stage_nodes.is_some_and(|(stage, _)| stage == id)
}

/// [`stage_bytes`]'s one stage, over a model re-drawn with `count` more nodes than the
/// gate's own: the gate model plus that stage's control, and nothing else.
fn stage_bytes_from_own_model(
    seed: u32,
    setting: &Setting,
    layout: &core::Capability,
) -> Result<Vec<u8>, String> {
    let extra = setting.stage_nodes.map_or(0, |(_, count)| count);
    let count = gate_node_count(seed) + setting.extra_nodes + extra;
    let (nodes, edges) = seeded_model(seed, count, setting.reference_degree);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    layout_bytes(&topology, layout)
}

fn layout_bytes(topology: &Topology, layout: &core::Capability) -> Result<Vec<u8>, String> {
    run_force(topology, layout.run)
}

fn run_force(
    topology: &Topology,
    layout: impl FnOnce(&Topology) -> Result<Geometry, StageError>,
) -> Result<Vec<u8>, String> {
    let geometry = layout(topology).map_err(|e| e.to_string())?;
    graph_core::layout::snapshot(topology, geometry)
        .map(|snapshot| snapshot.to_bytes())
        .map_err(|e| e.to_string())
}
