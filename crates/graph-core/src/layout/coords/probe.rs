//! Builders and comparisons shared by the closed-form layouts' tests.

use crate::index::{Topology, index_model};
use crate::layout::Geometry;
use crate::records::build::{edge, node};
use graph_contract::geometry::NodeGeometry;

/// A topology of `count` nodes `n0..` and the given `(from, to)` edges, in order.
pub fn graph(count: u32, edges: &[(u32, u32)]) -> Topology {
    let nodes: Vec<_> = (0..count).map(|i| node(&format!("n{i}"), "")).collect();
    let edges: Vec<_> = edges
        .iter()
        .enumerate()
        .map(|(i, &(a, b))| edge(&format!("e{i}"), &format!("n{a}"), &format!("n{b}")))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

/// The point columns as `(x, y)` pairs; panics on any other geometry.
pub fn points(g: &Geometry) -> Vec<(f32, f32)> {
    let NodeGeometry::Point { x, y } = &g.nodes else {
        panic!("point nodes");
    };
    x.iter().copied().zip(y.iter().copied()).collect()
}

/// Every coordinate within `tolerance` of `want`, and finite.
pub fn assert_close(got: &[(f32, f32)], want: &[(f32, f32)], tolerance: f32) {
    assert_eq!(got.len(), want.len(), "{got:?} vs {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(g.0.is_finite() && g.1.is_finite(), "{got:?}");
        assert!(
            (g.0 - w.0).abs() <= tolerance && (g.1 - w.1).abs() <= tolerance,
            "{got:?} vs {want:?}"
        );
    }
}
