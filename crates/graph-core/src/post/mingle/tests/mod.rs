//! `post.bundle.mingle` pinned: the ink accounting, the merge order and its tie-break, the
//! multilevel record, and the two inks' disagreement.
//!
//! Every figure is an exact one on a hand-worked input. A test that accepted "close enough"
//! would let a swapped pairing, a changed gain sign or a reordered sort through, and those
//! are exactly the claims this stage makes.

use super::*;
use crate::index::index_model;
use crate::layout::Geometry;
use crate::records::build::{edge, node};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// Two banks four long apart, every cross edge spanning the gap: the long-span case
/// `fixtures/post/long-span.json` is written from, and the one MINGLE exists for.
fn long_span() -> (
    Vec<crate::records::NodeRecord>,
    Vec<crate::records::EdgeRecord>,
) {
    let mut nodes = Vec::new();
    for (tag, y) in [("a", 0.0), ("b", 4.0)] {
        for i in 0..4 {
            nodes.push(node(&format!("{tag}{i}"), ""));
        }
        let _ = y;
    }
    let mut edges = Vec::new();
    for i in 0..4 {
        edges.push(edge(
            &format!("a{i}-b{i}"),
            &format!("a{i}"),
            &format!("b{i}"),
        ));
        edges.push(edge(
            &format!("a{i}-b{}", (i + 1) % 4),
            &format!("a{i}"),
            &format!("b{}", (i + 1) % 4),
        ));
    }
    (nodes, edges)
}

/// A layout's geometry from explicit positions, so a test states the drawing it means.
fn geometry(x: &[f32], y: &[f32]) -> Geometry {
    Geometry::planar(
        NodeGeometry::Point {
            x: x.to_vec(),
            y: y.to_vec(),
        },
        EdgeGeometry::Line,
        Vec::new(),
    )
}

/// The long-span graph laid out as its fixture describes it: two rows 0.1 apart, four long
/// across.
fn laid_out() -> (crate::index::Topology, Geometry) {
    let (nodes, edges) = long_span();
    let topology = index_model(&nodes, &edges).expect("fits");
    let mut x = Vec::new();
    let mut y = Vec::new();
    for i in 0..4 {
        x.push(i as f32 * 0.1);
        y.push(0.0);
    }
    for i in 0..4 {
        x.push(i as f32 * 0.1);
        y.push(4.0);
    }
    (topology, geometry(&x, &y))
}

mod ink;
mod order;
