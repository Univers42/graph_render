//! The stages the hash gate hashes, and the bytes the native arm hashes for each
//! (`prompt.md` §7.1). Split out of `hashgate.rs` by the house's 300-line limit.
//!
//! The list is registry-driven (C1): the topology, then every layout of
//! `graph_core::registry::LAYOUTS`, then every analysis of `graph_wasm::analysis::ANALYSES`,
//! then every post of `graph_wasm::post::CAPABILITIES`, then the transport — the real ABI
//! over the same model, a stage of the gate in its own right (C20).
//!
//! **The native arm's bytes are driven by the same list.** `stages()` and
//! [`stage_bytes`] are one derivation, not two: an arm that printed a line for every
//! stage the gate asked for and nothing else is what makes the per-stage comparison
//! meaningful, and a second layout registered in `LAYOUTS` therefore joins the gate by
//! being registered, with no edit to this file. [`stage_bytes_for`] takes the registry
//! slice as an argument so that is testable here rather than only at the next merge. The
//! fifteen ANALYSIS and POST stages are the same argument over the two graph-wasm
//! registries, and their bytes come from the graph-wasm functions the wasm arm itself
//! calls — [`staged`], which is where that lives.
//!
//! **The four Phase 3 hierarchy layouts' ids are the layout modules' own**, not copies
//! spelled here: `graph_core::layout::{tidy_tree, treemap, circular, circle_packing}::ID`.
//! None of the four has an `impl Stage` — their modules pin every convention and say so,
//! and `Stage` requires a `Params: Default` — so each publishes a `pub const ID` the way
//! `graph_core::post::fdeb::ID` does, and both the knobs below and
//! `graph_core::registry::LAYOUTS` take the id from there. There is one place each id is
//! written, and `the_p3_stage_ids_are_the_registry_s_own` keeps the registry row and the
//! stage the knobs name the same one. The fifteen ANALYSIS and POST ids are the same
//! arrangement one level up: `knobs::ANALYSIS_POST_STAGES` names them from the graph-core
//! constants their own modules publish, and `each_stage_id_is_the_constant_its_own_module
//! _publishes` holds every one against the graph-wasm registry the gate walks.

mod checks;

use super::{Setting, staged};
pub(crate) use checks::node_count;
use checks::{check, stage_list};
use graph_core::layout::Geometry;
use graph_core::layout::circle_packing;
use graph_core::layout::force::BarnesHut;
use graph_core::layout::force::spring::{self, Spring, Spring3D};
use graph_core::layout::forceatlas2::{ForceAtlas2, ForceAtlas2BarnesHut};
use graph_core::layout::graphviz::neato;
use graph_core::registry::{self as core, LAYOUTS};
use graph_core::{
    Grid, Stage, StageError, Sugiyama, Topology, index_model, run_pipeline, seeded_model,
};
use std::borrow::Cow;

/// The layout the transport stage runs: `harness/wasm-run.mjs`'s `abiSnapshotBytes`
/// drives `gm_run` with exactly this one, so the transport stage is the real ABI over
/// this layout and nothing else.
pub const LAYOUT: &str = "layout.grid";

/// The transport stage: `gm_seed_ingest → gm_alloc → gm_build → gm_run →
/// gm_snapshot_bytes` over the gate's own model — the real ABI, not the retained shim.
///
/// **The native arm deliberately gives this stage the grid's bytes, digest for digest**, so
/// the two rows of every native arm carry one repeated digest and a reader who expects two
/// distinct ones is not looking at a bug. The reason is that the native arm has no ABI of
/// its own to run: it is the same process, and the row that proves the *real* ABI is the
/// wasm arm's. That evidence is the C20 tally in `hashgate/transport.rs`, which counts the
/// seeds on which the **wasm** arm's `transport.wasm.columnar` line matches the **wasm**
/// arm's `layout.grid` line — two lines the wasm arm printed from two different calls, one
/// through `gm_snapshot_bytes` and one through the retained shim. So the native arm's
/// transport row restates the layout row as a *witness that the row is present and in
/// order*, and the tally beside it is what says the ABI agrees with the shim.
pub const TRANSPORT: &str = "transport.wasm.columnar";

/// Every stage, in the order both arms print them: the topology, every registered layout,
/// every registered analysis, every registered POST capability, then the transport.
///
/// **The last row is [`TRANSPORT`], and in a native arm its digest equals the
/// [`LAYOUT`] row's.** That is deliberate — the native arm runs no ABI of its own, so it
/// restates the grid's bytes to keep the row present and in order. The evidence that the
/// real ABI matches the retained shim is the C20 tally in `hashgate/transport.rs`, which
/// compares the *wasm* arm's two rows against each other. See [`TRANSPORT`].
///
/// **The list [`check`] audits is this list**, over whatever registry slice that check was
/// handed: one derivation, not two, so a stage the gate cannot print is a stage the gate
/// already refused to build rather than a discrepancy between two walkers. See
/// [`stage_list`] for what went wrong when they were two.
pub fn stages() -> Vec<&'static str> {
    stage_list(&LAYOUTS)
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
    let count = node_count(seed, setting, 0)?;
    let (nodes, edges) = seeded_model(seed, count, setting.reference_degree);
    let grid = run_pipeline::<Grid>(&nodes, &edges, &setting.grid).map_err(|e| e.to_string())?;
    if grid.layout != LAYOUT {
        return Err(format!("the pipeline ran {}, not {LAYOUT}", grid.layout));
    }
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let mut out = vec![("topology", grid.topology)];
    for layout in layouts {
        // A per-stage control's own re-drawn model comes **first**, ahead of every
        // parameter arm below, and the ordering is load-bearing rather than cosmetic: only
        // one control may be set at a time, so when `stage_nodes` names this stage the
        // parameter this arm would have read is the compiled-in default anyway. Matching
        // it first is what lets a stage that has *both* a parameter arm and a per-stage
        // control — `layout.force.spring3d` is the one today — answer to the control that
        // is actually set. With this arm below, `GM_MUTATE_FORCE_SPRING3D_NODES` would be
        // swallowed by the parameter arm and perturb nothing.
        //
        // The knob-aware stages that reach here run at `setting`'s parameters rather than
        // the registry's compiled-in default; the wasm arm cannot see the knobs, which is
        // exactly the divergence a wired control must surface.
        let bytes = if owns_own_model(layout.id, setting) {
            stage_bytes_from_own_model(seed, setting, layout)?
        } else {
            match layout.id {
                LAYOUT => grid.snapshot.to_bytes(),
                BarnesHut::ID => run_force(&topology, |t| BarnesHut::run(t, &setting.force))?,
                ForceAtlas2::ID => run_force(&topology, |t| ForceAtlas2::run(t, &setting.fa2))?,
                ForceAtlas2BarnesHut::ID => {
                    run_force(&topology, |t| ForceAtlas2BarnesHut::run(t, &setting.fa2))?
                }
                Spring::ID => run_force(&topology, |t| Spring::run(t, &setting.spring))?,
                // The 3D sibling is the *same* kernel at `D = 3` over the same
                // `SpringParams` (`force/spring3d.rs:45`), so the iterations budget is one
                // parameter and `GM_MUTATE_SPRING_ITERATIONS` reaches both dimensions.
                // Matched explicitly rather than left to the registry's `run_default`, which
                // would pin it to the compiled-in default and make the control vacuous here
                // — the MEDIUM finding p12-t3 recorded. It is still a two-stage control, so
                // `GM_MUTATE_FORCE_SPRING3D_NODES` is what names this one alone.
                spring::ID_3D => run_force(&topology, |t| Spring3D::run(t, &setting.spring))?,
                Sugiyama::ID => run_pipeline::<Sugiyama>(&nodes, &edges, &setting.sugiyama)
                    .map_err(|e| e.to_string())?
                    .snapshot
                    .to_bytes(),
                circle_packing::ID => {
                    run_force(&topology, |t| circle_packing::run_with(t, &setting.packing))?
                }
                neato::ID => run_force(&topology, |t| neato::run_with(t, setting.neato_epsilon()))?,
                _ => layout_bytes(&topology, layout)?,
            }
        };
        out.push((layout.id, bytes));
    }
    out.extend(staged_bytes(seed, setting, &topology)?);
    out.push((TRANSPORT, grid.snapshot.to_bytes()));
    Ok(out)
}

/// The fifteen ANALYSIS and POST stages' bytes, in [`stages`] order.
///
/// **The geometry every POST stage is run over is the grid's own** — the same
/// `gm_build → gm_run(layout.grid) → gm_post_run` order `harness/wasm-run.mjs` drives
/// and the transport stage already states. A POST pass reads positions, so hashing one
/// over some other layout's drawing would be a stage the wasm arm cannot reproduce at
/// all, and the differential would compare two different questions.
///
/// A stage whose control is set is drawn from that control's own re-drawn model (see
/// [`owns_own_model`]), the same probe the three Phase 3 node controls use; every other
/// ANALYSIS and POST stage is drawn from the gate's one model, so a control moves one
/// stage and names it.
fn staged_bytes(
    seed: u32,
    setting: &Setting,
    gate: &Topology,
) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    let mut out = Vec::new();
    for id in staged::analyses() {
        let own = own_topology(seed, setting, gate, id)?;
        out.push((id, staged::analysis_bytes(id, &own)?));
    }
    for id in staged::posts() {
        let own = own_topology(seed, setting, gate, id)?;
        let geometry = Grid::run(&own, &setting.grid).map_err(|err| format!("{id}: {err}"))?;
        out.push((id, staged::post_bytes(id, &own, &geometry)?));
    }
    Ok(out)
}

/// The topology stage `id` is drawn over: the gate's one model, or — for the single stage
/// whose control is set — that stage's own re-drawn one, with the control's extra nodes.
///
/// The one place "re-drawn" is defined, so the analysis and POST arms cannot come to
/// disagree about it. Every other stage reads `gate` unchanged, which is what makes a
/// control move one stage and name it.
fn own_topology<'a>(
    seed: u32,
    setting: &Setting,
    gate: &'a Topology,
    id: &str,
) -> Result<Cow<'a, Topology>, String> {
    match setting.stage_nodes {
        Some((stage, count)) if stage == id => Ok(Cow::Owned(redraw(seed, setting, count)?)),
        _ => Ok(Cow::Borrowed(gate)),
    }
}

/// `seeded_model(seed, gate_node_count + shared extras + own extras)` indexed: one
/// derivation for the gate's model and for a stage's re-drawn one, so a control cannot
/// perturb a stage by a node the gate's own model would not also have grown by.
fn redraw(seed: u32, setting: &Setting, own: u32) -> Result<Topology, String> {
    let count = node_count(seed, setting, own)?;
    let (nodes, edges) = seeded_model(seed, count, setting.reference_degree);
    index_model(&nodes, &edges).map_err(|e| e.to_string())
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
    let count = node_count(seed, setting, extra)?;
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
