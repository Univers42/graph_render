//! `post.bundle.fdeb` pinned: the schedule, the four compatibility terms, the pair list's
//! order, the gather-form iteration, and the fixtures.
//!
//! Every figure here is an exact one on a hand-worked input, not a tolerance. A test that
//! accepted "close enough" would let a swapped operator, a halved constant or a reordered
//! sum through, and those are precisely the changes this slice's claims are about.

use super::*;
use crate::index::index_model;
use crate::layout::Geometry;
use crate::records::build::{edge, node};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry, Paths};

/// A layout's geometry from explicit positions, so a test states the drawing it means.
fn points(n: usize, xy: &[(f32, f32)]) -> Geometry {
    Geometry {
        nodes: NodeGeometry::Point {
            x: xy.iter().map(|p| p.0).collect(),
            y: xy.iter().map(|p| p.1).collect(),
        },
        edges: EdgeGeometry::Line,
        notes: Vec::new(),
    }
    .piped(n)
}

impl Geometry {
    /// Asserts the node count the test meant, which a slice of the wrong length would
    /// otherwise hide.
    fn piped(self, n: usize) -> Geometry {
        assert_eq!(self.nodes.columns()[0].1.len(), n, "node count");
        self
    }
}

/// Two edges between the same pair of points, and one crossing edge: the smallest graph
/// where the pair list has something to keep and something to prune.
fn three_edges() -> crate::index::Topology {
    let nodes = ["a", "b", "c", "d"].map(|id| node(id, ""));
    let edges = [
        edge("e0", "a", "b"),
        edge("e1", "a", "b"),
        edge("e2", "c", "d"),
    ];
    index_model(&nodes, &edges).expect("fits")
}

mod run;
mod terms;
