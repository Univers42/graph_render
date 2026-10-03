use super::*;
use crate::layout::grid::{Grid, GridParams};
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

#[test]
fn sugiyama_declares_polyline_edges_and_the_reference_dummy_budget() {
    let sugiyama = find("layout.dag.sugiyama").expect("registered");
    assert_eq!(sugiyama.meta.nodes, NodeGeometryKind::Point);
    assert_eq!(sugiyama.meta.edges, EdgeGeometryKind::Polyline);
    assert_eq!(sugiyama.meta.scale_ceiling, 200_000);
    assert!(sugiyama.meta.complexity.contains("heuristic"));
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
