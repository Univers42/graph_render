//! Test fixtures for post tests.

use crate::ingest;
use crate::post::CAPABILITIES;
use graph_core::index_model;
use graph_core::post::Bundled;

// Re-exported, not imported: the sibling test files reach these through
// `use super::fixtures::*`, so the fixture module is where this tree's one import of them
// lives rather than six copies of it.
pub use graph_contract::geometry::{EdgeGeometry, EdgeGeometryKind, NodeGeometry, Paths};
pub use graph_contract::notes::{Note, NoteCode};
pub use graph_core::Geometry;
pub use graph_core::Topology;
pub use graph_core::post::grid_index::GridParams;

fn node_json(id: &str) -> String {
    format!(
        r#"{{"id":"{id}","kind":"record","database_id":null,"source":"pg","label":"L","group":null,"weight":0.5,"version":0.0,"has_note":false,"icon":null}}"#
    )
}

fn edge_json(id: &str, source: &str, target: &str) -> String {
    format!(
        r#"{{"id":"{id}","source":"{source}","target":"{target}","kind":"relation","label":"","strength":0.5,"directed":false,"record_id":null}}"#
    )
}

/// A topology through the same reader `gm_build` uses, so these tests need no second
/// way of making a graph (and cannot drift from the ABI's own).
pub fn topology(node_ids: &[&str], edges: &[(&str, &str, &str)]) -> Topology {
    let nodes: Vec<String> = node_ids.iter().map(|id| node_json(id)).collect();
    let edges: Vec<String> = edges.iter().map(|(id, s, t)| edge_json(id, s, t)).collect();
    let text = format!(
        r#"{{"version":1,"nodes":[{}],"edges":[{}]}}"#,
        nodes.join(","),
        edges.join(",")
    );
    let (nodes, edges) = ingest::read(text.as_bytes()).expect("the fixture is valid");
    index_model(&nodes, &edges).expect("the fixture indexes")
}

/// Two nodes ten apart on one axis, and the single edge between them: the smallest
/// fixture every capability can be run over, with the geometry written by hand so the
/// expected coordinates are derived from the stated conventions and not from whatever
/// a layout happened to produce.
pub fn pair() -> (Topology, Geometry) {
    let t = topology(&["a", "b"], &[("e", "a", "b")]);
    (t, points(&[0.0, 10.0], &[0.0, 0.0], &[]))
}

pub fn points(x: &[f32], y: &[f32], notes: &[Note]) -> Geometry {
    Geometry {
        nodes: NodeGeometry::Point {
            x: x.to_vec(),
            y: y.to_vec(),
        },
        edges: EdgeGeometry::Line,
        notes: notes.to_vec(),
    }
}

pub fn paths(edges: &EdgeGeometry) -> &Paths {
    match edges {
        EdgeGeometry::Polyline(p) | EdgeGeometry::Curve { paths: p, .. } => p,
        EdgeGeometry::Line => panic!("a Line edge kind stores no path"),
    }
}

/// `None` for a `Line`, which stores no row at all — the one capability that does, so a
/// caller must not ask its columns for points.
pub fn row_points(edges: &EdgeGeometry) -> Option<Vec<f32>> {
    match edges {
        EdgeGeometry::Line => None,
        _ => Some(paths(edges).pts.clone()),
    }
}

pub fn run_at(t: &Topology, g: &Geometry, id: &str) -> Bundled {
    let i = index_of(id);
    (CAPABILITIES[i].run)(t, g).unwrap_or_else(|e| panic!("{id} over the pair fixture: {e}"))
}

pub fn index_of(id: &str) -> usize {
    IDS.iter()
        .position(|&candidate| candidate == id)
        .unwrap_or_else(|| panic!("{id} is not one of the pinned ids"))
}

/// The ids this table must answer with, in order. Written out as literals rather than
/// read back from the table: a registry that answered for whatever it happened to hold
/// would pass a test that only checks it against itself.
pub const IDS: [&str; 7] = [
    "post.bundle.fdeb",
    "post.bundle.mingle",
    "post.route.grid",
    "post.style.straight",
    "post.style.orthogonal",
    "post.style.quadratic",
    "post.style.bezier",
];