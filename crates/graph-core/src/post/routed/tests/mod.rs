//! Routing's own tests. The three the phase names are here by name: routing twice gives
//! the same bytes, an enclosed node falls back and says so, and a route avoids the nodes
//! between its endpoints. The rest pin the tie-break, the CSR, and the flag's plumbing.

use super::*;
use crate::columns::EdgeColumns;
use crate::index::index_model;
use crate::records::build::{edge, node};
use crate::records::{EdgeRecord, NodeRecord};

/// The parameters every test here uses: a cell of exactly 1 over a span of 8, so a cell
/// index **is** the coordinate and every expected route is readable as numbers.
fn unit() -> GridParams {
    GridParams {
        resolution: 8,
        margin: 2,
        clearance: 0.0,
    }
}

/// A grid over `points`, each `(x, y)`, at [`unit`].
fn grid_over(points: &[(f32, f32)]) -> GridIndex {
    let nodes = NodeGeometry::Point {
        x: points.iter().map(|p| p.0).collect(),
        y: points.iter().map(|p| p.1).collect(),
    };
    let mut grid = GridIndex::new();
    grid.build(&nodes, &unit()).expect("builds");
    grid
}

/// The edge columns for `edges` as `(source, target)` node-index pairs, over a topology
/// built to match, so `route_over` can be called with the columns alone.
fn columns(nodes: usize, edges: &[(u32, u32)]) -> EdgeColumns {
    let records: Vec<_> = (0..nodes).map(|i| node(&format!("n{i}"), "")).collect();
    let links: Vec<_> = edges
        .iter()
        .enumerate()
        .map(|(i, (a, b))| edge(&format!("e{i}"), &format!("n{a}"), &format!("n{b}")))
        .collect();
    let topology = index_model(&records, &links).expect("fits");
    topology.edges().clone()
}

/// The corners that fix the span at 8, so the cell size is 1.
const SPAN: [(f32, f32); 2] = [(0.0, 0.0), (8.0, 8.0)];

fn spanned(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    SPAN.iter().copied().chain(points.iter().copied()).collect()
}

/// A grid over `points` at `resolution`, with no margin, so the drawing reaches the border.
fn bare(points: &[(f32, f32)], resolution: u32) -> (NodeGeometry, GridIndex) {
    let nodes = NodeGeometry::Point {
        x: points.iter().map(|p| p.0).collect(),
        y: points.iter().map(|p| p.1).collect(),
    };
    let params = GridParams {
        resolution,
        margin: 0,
        clearance: 0.0,
    };
    let mut grid = GridIndex::new();
    grid.build(&nodes, &params).expect("builds");
    (nodes, grid)
}

mod adapter;
mod limits;
mod paths;
mod routes;
mod stencil;
mod ties;
