use super::*;
use crate::layout::grid::{Grid, GridParams};
use crate::layout::{circle_packing, circular, tidy_tree, treemap};
use crate::stage::{gate_node_count, run_with, seeded_model};
use crate::weights::REFERENCE_DEGREE;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

#[test]
fn every_layout_is_a_layout_stage_with_its_metadata_filled() {
    let mut ids: Vec<_> = LAYOUTS.iter().map(|layout| layout.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), LAYOUTS.len(), "unique ids");
    for layout in &LAYOUTS {
        let m = layout.meta;
        assert!(layout.id.starts_with("layout."), "{}", layout.id);
        assert_eq!(m.stage, "layout");
        assert!(m.scale_ceiling > 0, "{}", layout.id);
        for text in [m.oracle, m.complexity, m.degradation, m.ponytail] {
            assert!(!text.trim().is_empty(), "{}", layout.id);
        }
    }
}

#[test]
fn a_registered_layout_emits_the_kinds_it_declares_at_its_default_parameters() {
    let (nodes, edges) = seeded_model(5, gate_node_count(5), REFERENCE_DEGREE);
    for layout in &LAYOUTS {
        let run = run_with(&nodes, &edges, layout.id, layout.run).expect("runs");
        let header = run.snapshot.header();
        assert_eq!(
            (header.node_kind, header.edge_kind),
            (layout.meta.nodes, layout.meta.edges)
        );
    }
    let grid = find("layout.grid").expect("registered");
    let by_hand = run_with(&nodes, &edges, "layout.grid", |t| {
        Grid::run(t, &GridParams::default())
    });
    assert_eq!(run_with(&nodes, &edges, grid.id, grid.run), by_hand);
    assert!(find("layout.none").is_none());
}

/// L-28: the ids at the indices an out-of-crate caller pins, asserted.
///
/// `graph-wasm/src/handle.rs:184` and `crates/graph-core/src/post/tests.rs:201` both
/// call `LAYOUTS[0].run` and expect `layout.grid`; `bench/campaign.rs:128` is
/// `LAYOUTS[3]`, the default crossover arm. `graph-wasm`'s ABI is positional
/// (`exports/build.rs:32,143`), so an id at an index is also that layout's wire
/// `layout_id`. Nothing about `[Capability; N]` notices two rows swapped, which is
/// the whole defect: this assertion is what notices.
///
/// Only the front block is pinned, and that is the whole set of dependencies: nothing
/// outside this crate names an index above 4 (the SDK maps a string id by scanning
/// `0..gm_layout_count()` at init, `exports/build.rs:26-32`), and a row appended past
/// the block moves nothing here. Pinning every index would make each later append
/// depend on a file its own job is not allowed to touch.
#[test]
fn the_index_keyed_front_of_layouts_still_holds_the_ids_their_callers_name() {
    assert_eq!(
        LAYOUTS
            .iter()
            .take(5)
            .map(|layout| layout.id)
            .collect::<Vec<_>>(),
        [
            Grid::ID,
            tidy_tree::ID,
            treemap::ID,
            circular::ID,
            circle_packing::ID,
        ],
        "the index-keyed front of LAYOUTS moved: grid at 0 and circular at 3 are named \
         by index from graph-wasm and from bench/campaign.rs"
    );
}

/// L-27: the two ceilings that answer "the largest size `bench` accepts" read the one
/// constant the motor owns, so `bench/scale.rs` and both ceilings move together and a
/// drift between them is this test failing rather than a number copied twice that happens
/// to agree.
#[test]
fn the_radial_and_basic_3d_ceilings_are_the_one_bench_node_cap() {
    assert_eq!(RADIAL_CEILING, u64::from(MAX_BENCH_NODES));
    assert_eq!(BASIC_3D_CEILING, u64::from(MAX_BENCH_NODES));
    assert_eq!(
        find("layout.twopi").map(|l| l.meta.scale_ceiling),
        Some(RADIAL_CEILING)
    );
    assert_eq!(
        find("layout.hierarchical3d").map(|l| l.meta.scale_ceiling),
        Some(BASIC_3D_CEILING)
    );
}

#[test]
fn sugiyama_declares_polyline_edges_and_the_reference_dummy_budget() {
    let sugiyama = find("layout.dag.sugiyama").expect("registered");
    assert_eq!(sugiyama.meta.nodes, NodeGeometryKind::Point);
    assert_eq!(sugiyama.meta.edges, EdgeGeometryKind::Polyline);
    assert_eq!(sugiyama.meta.scale_ceiling, 200_000);
    assert!(sugiyama.meta.complexity.contains("heuristic"));
}

#[test]
fn lanes_declares_point_nodes_polyline_edges_and_its_two_spacings() {
    let lanes = find("layout.dag.lanes").expect("registered");
    assert_eq!(lanes.meta.nodes, NodeGeometryKind::Point);
    assert_eq!(lanes.meta.edges, EdgeGeometryKind::Polyline);
    let names: Vec<_> = lanes.params.specs.iter().map(|spec| spec.name).collect();
    assert_eq!(names, ["lane_spacing", "row_spacing", "horizontal"]);
    assert_eq!(
        LAYOUTS.last().map(|c| c.id),
        Some("layout.dag.lanes"),
        "appended last"
    );
    assert_eq!(lanes.meta.scale_ceiling, LANES_CEILING);
    // The three things a reader of this row needs and the plan's first draft left out
    // (`docs/decisions/dag-lanes.md` conditions 5, 6 and 7).
    assert!(
        lanes.meta.ponytail.contains("Ponytail (scale_ceiling)"),
        "{}",
        lanes.meta.ponytail
    );
    // Every clause carries one, whatever their number is: the house rule requires all three
    // parts, and a clause added later without one is the failure this pins.
    let clauses = lanes.meta.ponytail.matches("Ponytail (").count();
    assert_eq!(
        lanes.meta.ponytail.matches("Escape hatch:").count(),
        clauses,
        "{clauses} clauses, each needs one: {}",
        lanes.meta.ponytail
    );
    assert!(
        lanes.meta.degradation.contains("directed edges only"),
        "{}",
        lanes.meta.degradation
    );
    assert!(
        lanes.meta.degradation.contains("tie-break path only"),
        "{}",
        lanes.meta.degradation
    );
    for name in [
        "a_directed_cycle_is_broken_at_the_lowest_index_and_noted",
        "distinct_versions_break_a_cycle_and_the_heap_orders_by_version",
        "equal_versions_fall_back_to_index_order",
    ] {
        assert!(
            lanes.meta.oracle.contains(name),
            "{} is not named in the oracle",
            name
        );
    }
}

#[test]
fn the_force_layouts_are_registered_with_the_ceilings_this_branch_measured() {
    let bh = find("layout.force.barnes_hut").expect("barnes-hut registered");
    let fa2 = find("layout.forceatlas2").expect("fa2 registered");
    assert_eq!(
        (bh.meta.nodes, bh.meta.edges),
        (NodeGeometryKind::Point, EdgeGeometryKind::Line)
    );
    assert_eq!(
        (fa2.meta.nodes, fa2.meta.edges),
        (NodeGeometryKind::Point, EdgeGeometryKind::Line)
    );
    assert_eq!(bh.meta.scale_ceiling, FORCE_CEILING);
    assert_eq!(fa2.meta.scale_ceiling, FA2_CEILING);
    assert!(
        bh.meta.complexity.contains("O(n log n)"),
        "{}",
        bh.meta.complexity
    );
    assert!(
        fa2.meta.complexity.contains("O(n^2)"),
        "{}",
        fa2.meta.complexity
    );
    // The two ceilings differ by the algorithmic shape, not by taste: theta-
    // approximated many-body against networkx's dense all-pairs form. The measured
    // ratio is 100_000 / 14_000 = 7.14x, so pin "materially below" at 5x rather
    // than inventing a round factor the measurements do not support. A row that
    // claimed one number for both would hide the whole point of shipping both.
    const {
        assert!(
            FA2_CEILING * 5 < FORCE_CEILING,
            "the dense FA2 ceiling must sit materially below the theta-tree one"
        );
    }
    for text in [bh.meta.oracle, fa2.meta.oracle] {
        assert!(
            text.contains("d3-force") || text.contains("networkx"),
            "{text}"
        );
    }
    for text in [bh.meta.degradation, fa2.meta.degradation] {
        assert!(
            text.contains("Barnes-Hut") || text.contains("refus"),
            "{text}"
        );
    }
    for text in [bh.meta.ponytail, fa2.meta.ponytail] {
        // The chaos marker is spelled in caps in both rows, as the phase prompt
        // requires force layouts to name it; match it case-insensitively.
        assert!(
            text.to_lowercase().contains("chaotic"),
            "a force layout must name the chaos, its direction and its escape hatch: {text}"
        );
        assert!(
            text.contains("Escape hatch") || text.contains("escape hatch"),
            "{text}"
        );
    }
}
