//! The gate's stage list and the native arm's bytes for it, both derived from
//! `graph_core::registry::LAYOUTS`. Split from the parent test module by the house's
//! 300-line limit, and the module that pins the one invariant the whole gate rests on:
//! **every stage the gate asks for has a native producer**, in the same order.
//!
//! Without it the two derivations drift: `stages()` grew with the registry while
//! `stage_bytes` named three stages literally, so the arm printed fewer lines than
//! `compare::diverged` demanded and the gate refused its own honest run the moment a
//! second layout was registered. The stand-in layouts below live in `marked` — a child
//! module, because the helpers that build them are only this file's business and this
//! file is at the line limit. The tests that drive the shipped wasm arm itself live in
//! `arm`, for the same reason: they need a second program and a real artifact.

mod arm;
mod marked;

use super::*;
use graph_core::registry::Capability;
use graph_core::{gate_node_count, seeded_model};
use marked::{crowded, ids_of, marked_layouts};

/// A copy of the grid registered under a second id — what the gate sees the day p3's
/// layouts land. Built from the registry's own row, so it needs no new layout code.
fn second_layout() -> Capability {
    let grid = graph_core::registry::find(LAYOUT).expect("the grid is registered");
    Capability {
        id: "layout.grid.second",
        ..*grid
    }
}

/// The ids `stage_bytes` would produce for `layouts`, or the refusal it gives.
fn ids(layouts: &[Capability], setting: &Setting) -> Result<Vec<&'static str>, String> {
    Ok(ids_of(&stage_bytes_for(4, setting, layouts)?))
}

#[test]
fn a_second_registered_layout_joins_the_gate_with_no_change_to_the_stage_list() {
    let grid = graph_core::registry::find(LAYOUT).expect("the grid is registered");
    let one = [*grid];
    let two = [*grid, second_layout()];
    let setting = honest();
    assert_eq!(ids(&one, &setting).expect("one layout"), stages());
    assert_eq!(
        ids(&two, &setting).expect("two layouts"),
        ["topology", LAYOUT, "layout.grid.second", TRANSPORT],
        "one stage per registered layout, in registry order, transport last"
    );
}

#[test]
fn every_registered_layouts_native_bytes_are_its_own_run_over_the_same_topology() {
    let grid = graph_core::registry::find(LAYOUT).expect("the grid is registered");
    let two = [*grid, second_layout()];
    let setting = honest();
    let stages = stage_bytes_for(4, &setting, &two).expect("runs");
    let bytes = |id: &str| {
        stages
            .iter()
            .find(|(stage, _)| *stage == id)
            .map(|(_, bytes)| bytes.clone())
            .unwrap_or_else(|| panic!("{id} is a stage"))
    };
    let (nodes, edges) = seeded_model(4, gate_node_count(4), REFERENCE_DEGREE);
    for layout in &two {
        let registered = graph_core::run_with(&nodes, &edges, layout.id, layout.run).expect("runs");
        assert_eq!(
            bytes(layout.id),
            registered.snapshot.to_bytes(),
            "{} is hashed from the pipeline's own snapshot, not a re-derivation",
            layout.id
        );
    }
    let grid_run = graph_core::run_with(&nodes, &edges, grid.id, grid.run).expect("runs");
    assert_eq!(bytes("topology"), grid_run.topology);
    // The grid's stage is the perturbed `run_pipeline` run, so the spacing control still
    // bites it; at the default parameters that run and the registry's own are one run.
    assert_eq!(bytes(LAYOUT), bytes("layout.grid.second"));
    assert_eq!(bytes(LAYOUT), grid_run.snapshot.to_bytes());
    assert_eq!(bytes(TRANSPORT), grid_run.snapshot.to_bytes());
}

#[test]
fn a_registry_without_the_transports_layout_or_with_a_repeated_id_is_refused() {
    let grid = graph_core::registry::find(LAYOUT).expect("the grid is registered");
    let setting = honest();
    let without = stage_bytes_for(4, &setting, &[]).expect_err("nothing to restate");
    assert!(without.contains(LAYOUT), "{without}");
    assert!(without.contains("restates"), "{without}");
    let repeated = stage_bytes_for(4, &setting, &[*grid, second_layout(), second_layout()])
        .expect_err("two stages with one id");
    assert!(repeated.contains("layout.grid.second"), "{repeated}");
    assert!(repeated.contains("twice"), "{repeated}");
}

#[test]
fn the_stages_are_the_topology_then_every_registered_layout_then_the_transport() {
    let layouts: Vec<&str> = graph_core::registry::LAYOUTS.iter().map(|l| l.id).collect();
    let mut want = vec!["topology"];
    want.extend(layouts);
    want.push(TRANSPORT);
    assert_eq!(stages(), want);
    assert_eq!(
        stage_bytes(4, &honest()).expect("runs").len(),
        stages().len(),
        "one native producer per stage the gate asks for"
    );
}

#[test]
fn the_transport_stage_runs_the_layout_the_wasm_arm_names() {
    assert!(
        LAYOUT == "layout.grid",
        "harness/wasm-run.mjs's abiSnapshotBytes"
    );
    assert!(graph_core::registry::find(LAYOUT).is_some(), "registered");
}

#[test]
fn the_native_arm_hashes_the_transport_stage_from_the_pipelines_own_snapshot() {
    let lines = arm_lines(2, &honest()).expect("runs");
    let digests = |stage: &str| -> Vec<String> {
        lines
            .lines()
            .filter_map(|l| l.strip_prefix(&format!("{stage} ")))
            .map(|l| l.rsplit_once(' ').expect("digest").1.to_owned())
            .collect()
    };
    let transport = digests(TRANSPORT);
    assert_eq!(transport.len(), 2, "one digest per seed");
    assert_eq!(transport, digests(LAYOUT), "the transport stage's bytes");
}

/// `stage_bytes`'s stages as their bytes, in [`stages`] order.
fn bytes_under(setting: &Setting) -> Vec<(&'static str, Vec<u8>)> {
    stage_bytes(4, setting).expect("runs")
}

/// The bytes of the stage `stage` names, or a panic naming what the gate does list.
fn stage<'a>(stages: &'a [(&'static str, Vec<u8>)], id: &str) -> &'a [u8] {
    stages
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, bytes)| bytes.as_slice())
        .unwrap_or_else(|| panic!("{id} is not a stage"))
}

#[test]
fn every_layout_gets_its_own_bytes_and_a_perturbed_one_diverges_from_its_own_default() {
    let three = crowded();
    let honest = stage_bytes_for(4, &honest(), &three).expect("runs");
    assert_eq!(
        ids_of(&honest),
        [
            "topology",
            LAYOUT,
            "layout.grid.left",
            "layout.grid.right",
            TRANSPORT
        ]
    );
    let [left, right] = marked_layouts();
    let (nodes, edges) = seeded_model(4, gate_node_count(4), REFERENCE_DEGREE);
    assert_eq!(
        stage(&honest, left.id),
        graph_core::run_with(&nodes, &edges, left.id, left.run)
            .expect("runs")
            .snapshot
            .to_bytes(),
        "left is hashed from its own run, not the grid's"
    );
    assert_ne!(
        stage(&honest, left.id),
        stage(&honest, right.id),
        "a marked layout's bytes are its own, so a stage that hashed the grid for every \
         registered layout would fail here"
    );
    assert_ne!(stage(&honest, left.id), stage(&honest, LAYOUT));
}

#[test]
fn the_degree_knob_moves_the_topology_and_the_grid_knob_the_two_grid_stages() {
    let base = honest();
    let honest_run = bytes_under(&base);
    let layout = stage(&honest_run, LAYOUT);
    let degree = Setting {
        reference_degree: REFERENCE_DEGREE + 1,
        ..base
    };
    let by_degree = bytes_under(&degree);
    assert_ne!(
        stage(&by_degree, "topology"),
        stage(&honest_run, "topology")
    );
    for id in [LAYOUT, TRANSPORT] {
        assert_eq!(
            stage(&by_degree, id),
            layout,
            "{id}: the grid ignores weights"
        );
    }
    let spacing = Setting {
        grid: GridParams { spacing: 2.0 },
        ..base
    };
    let by_spacing = bytes_under(&spacing);
    assert_eq!(
        stage(&by_spacing, "topology"),
        stage(&honest_run, "topology")
    );
    assert_ne!(stage(&by_spacing, LAYOUT), layout);
    assert_eq!(
        stage(&by_spacing, TRANSPORT),
        stage(&by_spacing, LAYOUT),
        "the transport stage follows the layout it restates"
    );
    let refused = Setting {
        grid: GridParams { spacing: 0.0 },
        ..base
    };
    let err = stage_bytes(4, &refused).expect_err("zero spacing");
    assert_eq!(err, "parameter spacing: finite and above 0");
}

#[test]
fn an_arm_prints_every_seed_of_one_stage_before_the_next() {
    let seeds = 2;
    let lines = arm_lines(seeds, &honest()).expect("runs");
    let prefixes: Vec<_> = lines
        .lines()
        .map(|l| l.rsplit_once(' ').expect("digest").0)
        .collect();
    let want: Vec<String> = stages()
        .iter()
        .flat_map(|stage| (0..seeds).map(move |seed| format!("{stage} {seed}")))
        .collect();
    assert_eq!(prefixes, want);
    assert_eq!(prefixes.len(), stages().len() * seeds as usize);
    let refused = Setting {
        grid: GridParams { spacing: -1.0 },
        ..honest()
    };
    assert!(
        arm_lines(seeds, &refused)
            .expect_err("refused")
            .starts_with("seed 0: ")
    );
}
