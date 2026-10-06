//! What the arms run: the stage list, the registered pipeline's bytes, and which knob
//! moves which stage — so a control's reach is a measured fact and not a claim.

use super::super::staged;
use super::super::stages::stages as stage_ids;
use super::super::{TRANSPORT, stage_bytes};
use super::Setting;
use super::honest;
use graph_core::{GridParams, REFERENCE_DEGREE, SugiyamaParams, gate_node_count, seeded_model};

/// The seeds every per-stage property below is checked at. **More than one**, because a
/// claim read at a single seed is a claim about that seed: the review named "every per-stage
/// property is asserted at seed 4 only" (RG-48), and the registry-wide byte equality stays at
/// `SEEDS[0]` alone because it is the expensive half and is not seed-specific.
const SEEDS: [u32; 2] = [4, 30];

/// Where `layout.dag.sugiyama` sits in [`stages`].
fn dag() -> usize {
    index("layout.dag.sugiyama")
}

/// Where the treemap sits in [`stages`].
///
/// **By id, not by position** (RG-48). The test used to say `moved[3]`, so a layout inserted
/// above `layout.treemap.squarified` would compare the wrong stage's bytes and report the
/// result as "treemap reads node weight" — a wrong-stage failure dressed as the right one.
fn treemap() -> usize {
    index(graph_core::layout::treemap::ID)
}

fn index(id: &str) -> usize {
    stage_ids()
        .iter()
        .position(|seen| *seen == id)
        .unwrap_or_else(|| panic!("{id} is a registered stage"))
}

/// The gate's stage list: the topology, then every registered layout, then the ANALYSIS
/// and POST stages the graph-wasm registries name, then the transport. The layouts are
/// `graph_core::registry`'s own; the ANALYSIS and POST tail is `staged`'s, and a stage in
/// either list is found by id rather than by position.
#[test]
fn the_stages_are_the_topology_then_the_layouts_then_the_analysis_and_post_then_the_transport() {
    let layouts: Vec<&str> = graph_core::registry::LAYOUTS.iter().map(|l| l.id).collect();
    let mut want = vec!["topology"];
    want.extend(layouts);
    want.extend(staged::analyses());
    want.extend(staged::posts());
    want.push(TRANSPORT);
    assert_eq!(stage_ids(), want);
}

#[test]
fn stage_bytes_are_the_registered_pipeline_and_the_reference_and_spacing_knobs_move_theirs() {
    let honest_run = stage_bytes(SEEDS[0], &honest()).expect("runs");
    let ids: Vec<&str> = honest_run.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, stage_ids());
    assert_eq!(honest_run[treemap()].0, graph_core::layout::treemap::ID);
    let topology = honest_run[0].1.clone();
    let (nodes, edges) = seeded_model(SEEDS[0], gate_node_count(SEEDS[0]), REFERENCE_DEGREE);
    // The registered layouts only: the ANALYSIS and POST stages that follow them are hashed
    // from the graph-wasm registries, not from `graph_core::registry`, and have their own
    // tests in `hashgate/tests/knob.rs`.
    let layouts_end = 1 + graph_core::registry::LAYOUTS.len();
    for (id, bytes) in &honest_run[1..layouts_end] {
        let layout = graph_core::registry::find(id).expect("registered");
        let registered = graph_core::run_with(&nodes, &edges, layout.id, layout.run).expect("runs");
        assert_eq!(
            (&registered.topology, &registered.snapshot.to_bytes()),
            (&topology, bytes),
            "{id}"
        );
    }
    for &seed in &SEEDS {
        per_stage_claims(seed);
    }
}

/// Which knob moves which stage, at every seed in [`SEEDS`].
///
/// Split from the registry-equality check above so the per-stage claims are stated at more
/// than one seed without paying for a full registry comparison at each: one seed's agreement
/// is not the property being claimed (RG-48).
fn per_stage_claims(seed: u32) {
    let honest_run = stage_bytes(seed, &honest()).expect("runs");
    let (topology, layout) = (honest_run[0].1.clone(), honest_run[1].1.clone());
    let degree = Setting {
        reference_degree: REFERENCE_DEGREE + 1,
        ..honest()
    };
    let moved = stage_bytes(seed, &degree).expect("runs");
    assert_ne!(moved[0].1, topology);
    assert_eq!(moved[1].1, layout, "the grid ignores weights");
    assert_ne!(
        moved[treemap()].1,
        honest_run[treemap()].1,
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
    let spaced = stage_bytes(seed, &spacing).expect("runs");
    assert_eq!(spaced[0].1, topology);
    assert_ne!(spaced[1].1, layout);
    assert_eq!(spaced[dag()].1, honest_run[dag()].1);
}

#[test]
fn the_layer_spacing_knob_moves_only_the_layered_drawing_and_zero_is_refused() {
    let flat = Setting {
        sugiyama: SugiyamaParams {
            layer_spacing: 0.0,
            horizontal: false,
        },
        ..honest()
    };
    for &seed in &SEEDS {
        let honest_run = stage_bytes(seed, &honest()).expect("runs");
        assert_eq!(honest_run[dag()].0, "layout.dag.sugiyama");
        let layers = Setting {
            sugiyama: SugiyamaParams {
                layer_spacing: 2.0,
                horizontal: false,
            },
            ..honest()
        };
        let moved = stage_bytes(seed, &layers).expect("runs");
        for (position, (moved, honest)) in moved.iter().zip(&honest_run).enumerate() {
            assert_eq!(moved.1 != honest.1, position == dag(), "{}", moved.0);
        }
        let err = stage_bytes(seed, &flat).expect_err("zero layer spacing");
        assert_eq!(err, "parameter layer_spacing: finite and above 0");
    }
}

#[test]
fn the_node_count_knob_moves_every_stage_and_zero_spacing_is_refused() {
    let more_nodes = Setting {
        extra_nodes: 1,
        ..honest()
    };
    let refused = Setting {
        grid: GridParams { spacing: 0.0 },
        ..honest()
    };
    for &seed in &SEEDS {
        let honest_run = stage_bytes(seed, &honest()).expect("runs");
        let grown = stage_bytes(seed, &more_nodes).expect("runs");
        assert_eq!(
            grown.len(),
            honest_run.len(),
            "one run, the whole stage list"
        );
        for (grown, honest) in grown.iter().zip(&honest_run) {
            assert_ne!(
                grown.1, honest.1,
                "{}: one more node must move every stage",
                grown.0
            );
        }
        let err = stage_bytes(seed, &refused).expect_err("zero spacing");
        assert_eq!(err, "parameter spacing: finite and above 0");
    }
}
