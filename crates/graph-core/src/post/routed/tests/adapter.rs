//! The petgraph adapter's own contract: an empty or out-of-range query answers empty, the
//! whole edge set is enumerable, and an edge id names its edge (review findings M16, M17,
//! M18, R11).

use super::*;
use crate::post::routed::csr::STENCIL;
use petgraph::visit::{EdgeRef, IntoEdgeReferences, IntoEdges, IntoNeighbors};
use std::collections::BTreeSet;

/// A 3 × 3 grid: four corners, four sides and one interior cell, so every row length a
/// grid can have appears once.
fn three_by_three() -> GridCsr {
    let (_, grid) = bare(&[(0.0, 0.0), (3.0, 3.0)], 3);
    assert_eq!(grid.shape(), (3, 3));
    build_csr(&grid)
}

#[test]
fn an_empty_grids_csr_answers_a_cell_query_without_dividing_by_zero() {
    let graph = build_csr(&GridIndex::new());
    assert_eq!(graph.cells(), 0);
    assert_eq!(graph.xy(0), (0, 0));
}

#[test]
fn a_cell_out_of_range_has_no_neighbours_and_no_edges() {
    let graph = three_by_three();
    let view = GridGraph::new(&graph);
    let past = graph.cells();
    assert_eq!(graph.row(past).count(), 0);
    assert_eq!(view.neighbors(past).count(), 0);
    assert_eq!(view.edges(past).count(), 0);
}

#[test]
fn edge_references_list_every_edge_once_in_cell_order() {
    let graph = three_by_three();
    let view = GridGraph::new(&graph);
    let all: Vec<_> = view.edge_references().collect();
    // 4 corners × 3 + 4 sides × 5 + 1 interior × 8.
    assert_eq!(all.len(), 40);
    let per_cell: Vec<_> = (0..graph.cells()).flat_map(|c| view.edges(c)).collect();
    assert_eq!(all, per_cell);
}

#[test]
fn an_edge_id_names_its_source_and_its_stencil_slot() {
    let graph = three_by_three();
    let view = GridGraph::new(&graph);
    let mut seen = BTreeSet::new();
    for edge in (0..graph.cells()).flat_map(|c| view.edges(c)) {
        let id = edge.id();
        assert!(seen.insert(id), "id {id} repeats");
        assert_eq!(id / 8, edge.source());
        let ((sx, sy), (tx, ty)) = (graph.xy(edge.source()), graph.xy(edge.target()));
        assert_eq!(STENCIL[(id % 8) as usize], (tx - sx, ty - sy), "id {id}");
    }
}
