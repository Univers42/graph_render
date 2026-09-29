//! The grid index's own tests: the cell arithmetic, the boundary tie-break, node
//! occupancy, and the reused buffer.

use super::*;
use graph_contract::geometry::NodeGeometry;

/// A layout of `n` nodes on a row, one unit apart, centred on the origin.
fn row(n: u32) -> NodeGeometry {
    let half = f64::from(n - 1) / 2.0;
    NodeGeometry::Point {
        x: (0..n)
            .map(|i| f64::from(i) - half)
            .map(|v| v as f32)
            .collect(),
        y: vec![0.0; n as usize],
    }
}

fn small() -> GridParams {
    GridParams {
        resolution: 8,
        margin: 0,
        clearance: 0.0,
    }
}

mod cells;
mod point;
mod reuse;
mod shapes;
