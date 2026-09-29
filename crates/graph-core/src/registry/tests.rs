use super::*;
use crate::layout::grid::GridParams;
use crate::stage::{gate_node_count, run_with, seeded_model};
use crate::weights::REFERENCE_DEGREE;

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
