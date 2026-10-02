//! The stencil slot a row reports, and the cost it prices, on every cell of a grid —
//! border cells included (review finding R1: a border row is shorter than eight, so its
//! position in the row is not its stencil slot).

use super::*;
use crate::post::routed::csr::STENCIL;
use petgraph::visit::{EdgeRef, IntoEdges};

/// The Euclidean length of the step from `a` to `b`, two neighbouring cells.
fn step_length(graph: &GridCsr, a: u32, b: u32) -> f64 {
    let ((ax, ay), (bx, by)) = (graph.xy(a), graph.xy(b));
    f64::sqrt(f64::from((bx - ax).pow(2) + (by - ay).pow(2)))
}

#[test]
fn a_border_cells_steps_cost_their_stencil_lengths() {
    let (_, grid) = bare(&[(0.0, 0.0), (3.0, 3.0)], 3);
    assert_eq!(grid.shape(), (3, 3));
    let graph = build_csr(&grid);
    for cell in 0..graph.cells() {
        let (cx, cy) = graph.xy(cell);
        for (slot, next) in graph.row(cell) {
            let (nx, ny) = graph.xy(next);
            assert_eq!(STENCIL[slot], (nx - cx, ny - cy), "cell {cell} -> {next}");
            assert_eq!(graph.cost(slot), step_length(&graph, cell, next));
        }
        for edge in GridGraph::new(&graph).edges(cell) {
            let length = step_length(&graph, edge.source(), edge.target());
            assert_eq!(*edge.weight(), length, "edge {cell} -> {}", edge.target());
        }
    }
}

#[test]
fn a_route_along_the_bottom_border_stays_on_it() {
    // Node 1 at cell (4, 0), node 0 at cell (0, 0): the shortest route is the four axis
    // steps along the bottom row, cost 4, and no other path is as short.
    let points = [(0.0, 0.0), (4.0, 0.0), (8.0, 8.0)];
    let (nodes, mut grid) = bare(&points, 8);
    let edges = columns(points.len(), &[(1, 0)]);
    let routed = route_over(&mut grid, &nodes, &edges).expect("routes");
    let route = routed.route(0);
    assert!(!route.straight_fallback);
    assert_eq!(route.cells, vec![4, 3, 2, 1, 0]);
}
