//! What the arms run: the stage list, the registered pipeline's bytes, and which knob
//! moves which stage — so a control's reach is a measured fact and not a claim.

use super::super::stages::stages as stage_ids;
use super::super::{TRANSPORT, stage_bytes};
use super::Setting;
use super::honest;
use graph_core::{GridParams, REFERENCE_DEGREE, SugiyamaParams, gate_node_count, seeded_model};

/// Where `layout.dag.sugiyama` sits in [`stages`].
fn dag() -> usize {
    stage_ids()
        .iter()
        .position(|id| *id == "layout.dag.sugiyama")
        .expect("registered")
}

#[test]
fn the_stages_are_the_topology_then_every_registered_layout_then_the_transport() {
    let layouts: Vec<&str> = graph_core::registry::LAYOUTS.iter().map(|l| l.id).collect();
    let mut want = vec!["topology"];
    want.extend(layouts);
    want.push(TRANSPORT);
    assert_eq!(stage_ids(), want);
}

#[test]
fn stage_bytes_are_the_registered_pipeline_and_the_reference_and_spacing_knobs_move_theirs() {
    let honest_run = stage_bytes(4, &honest()).expect("runs");
    let ids: Vec<&str> = honest_run.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, stage_ids());
    let (topology, layout) = (honest_run[0].1.clone(), honest_run[1].1.clone());
    let (nodes, edges) = seeded_model(4, gate_node_count(4), REFERENCE_DEGREE);
    for (id, bytes) in &honest_run[1..honest_run.len() - 1] {
        let layout = graph_core::registry::find(id).expect("registered");
        let registered = graph_core::run_with(&nodes, &edges, layout.id, layout.run).expect("runs");
        assert_eq!(
            (&registered.topology, &registered.snapshot.to_bytes()),
            (&topology, bytes),
            "{id}"
        );
    }
    let degree = Setting {
        reference_degree: REFERENCE_DEGREE + 1,
        ..honest()
    };
    let moved = stage_bytes(4, &degree).expect("runs");
    assert_ne!(moved[0].1, topology);
    assert_eq!(moved[1].1, layout, "the grid ignores weights");
    assert_ne!(
        moved[3].1, honest_run[3].1,
        "treemap reads node weight, so the reference degree moves it too"
    );
    assert_eq!(
        moved[dag()].1,
        honest_run[dag()].1,
        "the layered drawing ignores weights"
    );
    let spacing = Setting {
        grid: GridParams { spacing: 2.0 },
        ..honest()
    };
    let spaced = stage_bytes(4, &spacing).expect("runs");
    assert_eq!(spaced[0].1, topology);
    assert_ne!(spaced[1].1, layout);
    assert_eq!(spaced[dag()].1, honest_run[dag()].1);
}

#[test]
fn the_layer_spacing_knob_moves_only_the_layered_drawing_and_zero_is_refused() {
    let honest_run = stage_bytes(4, &honest()).expect("runs");
    assert_eq!(honest_run[dag()].0, "layout.dag.sugiyama");
    let layers = Setting {
        sugiyama: SugiyamaParams { layer_spacing: 2.0 },
        ..honest()
    };
    let moved = stage_bytes(4, &layers).expect("runs");
    for (index, (moved, honest)) in moved.iter().zip(&honest_run).enumerate() {
        assert_eq!(moved.1 != honest.1, index == dag(), "{}", moved.0);
    }
    let flat = Setting {
        sugiyama: SugiyamaParams { layer_spacing: 0.0 },
        ..honest()
    };
    let err = stage_bytes(4, &flat).expect_err("zero layer spacing");
    assert_eq!(err, "parameter layer_spacing: finite and above 0");
}

#[test]
fn the_node_count_knob_moves_every_stage_and_zero_spacing_is_refused() {
    let honest_run = stage_bytes(4, &honest()).expect("runs");
    let more_nodes = Setting {
        extra_nodes: 1,
        ..honest()
    };
    let grown = stage_bytes(4, &more_nodes).expect("runs");
    for (grown, honest) in grown.iter().zip(&honest_run) {
        assert_ne!(
            grown.1, honest.1,
            "{}: one more node must move every stage",
            grown.0
        );
    }
    let refused = Setting {
        grid: GridParams { spacing: 0.0 },
        ..honest()
    };
    let err = stage_bytes(4, &refused).expect_err("zero spacing");
    assert_eq!(err, "parameter spacing: finite and above 0");
}
