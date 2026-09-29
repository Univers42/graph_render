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
use graph_core::registry::{self as core, LAYOUTS};
use graph_core::{Grid, Topology, gate_node_count, index_model, run_pipeline, seeded_model};
use std::collections::BTreeSet;

/// The layout the transport stage runs: `harness/wasm-run.mjs`'s `abiSnapshotBytes`
/// drives `gm_run` with exactly this one, so the transport stage is the real ABI over
/// this layout and nothing else.
pub const LAYOUT: &str = "layout.grid";

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
    let (nodes, edges) = seeded_model(seed, gate_node_count(seed), setting.reference_degree);
    let grid = run_pipeline::<Grid>(&nodes, &edges, &setting.grid).map_err(|e| e.to_string())?;
    if grid.layout != LAYOUT {
        return Err(format!("the pipeline ran {}, not {LAYOUT}", grid.layout));
    }
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let mut out = vec![("topology", grid.topology)];
    for layout in layouts {
        let bytes = if layout.id == LAYOUT {
            grid.snapshot.to_bytes()
        } else {
            layout_bytes(&topology, layout)?
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

fn layout_bytes(topology: &Topology, layout: &core::Capability) -> Result<Vec<u8>, String> {
    let geometry = (layout.run)(topology).map_err(|e| e.to_string())?;
    graph_core::layout::snapshot(topology, geometry)
        .map(|snapshot| snapshot.to_bytes())
        .map_err(|e| e.to_string())
}
