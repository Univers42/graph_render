//! The composability gate, and the two registries it ties together.
//!
//! **Composability is the gate, not a nice-to-have.** A bundler that only works over a force
//! layout is not a stage: it means the POST/LAYOUT boundary leaked a layout-specific
//! assumption, which is an architecture problem and not a bug to patch (phase 8, step 6). So
//! this module runs the cross-product — every registered POST capability over every
//! registered layout — and asserts on the *result*, not merely that nothing panicked.
//!
//! Adding a capability or a layout needs no edit here: both loops read the registries, and
//! the only list this module keeps is the geometry each capability is expected to emit, which
//! the registry already declares.

use super::*;
use crate::edgekind::EdgeKind;
use crate::index::{Topology, index_model};
use crate::layout::Geometry;
use crate::records::EdgeRecord;
use crate::registry::LAYOUTS;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

mod ink;
mod registration;
mod z_column;

/// The graph every layout can lay out: a nine-node tree plus a non-hierarchy edge, a back
/// edge and a self-loop, so a POST capability meets all three edge cases whatever it claims
/// to support. The tree is what makes the matrix meaningful — the hierarchy layouts put their
/// nodes on a lattice of levels a bundler has to cope with, and the grid, circular and
/// packing layouts put them somewhere else entirely.
fn topology() -> Topology {
    let nodes: Vec<_> = (0..9).map(|i| node(&format!("n{i}"))).collect();
    let edges = vec![
        tree("t0", "n0", "n1"),
        tree("t1", "n0", "n2"),
        tree("t2", "n1", "n3"),
        tree("t3", "n1", "n4"),
        tree("t4", "n2", "n5"),
        tree("t5", "n2", "n6"),
        tree("t6", "n5", "n7"),
        tree("t7", "n7", "n8"),
        edge("x0", "n3", "n6"),
        edge("x1", "n0", "n0"),
    ];
    index_model(&nodes, &edges).expect("fits")
}

fn node(id: &str) -> crate::records::NodeRecord {
    crate::records::build::node(id, "")
}

/// A hierarchy edge `source → target` of wire type `parent_of`, which is what the three tree
/// layouts read as parent-child.
fn tree(id: &str, source: &str, target: &str) -> EdgeRecord {
    EdgeRecord {
        kind: EdgeKind::Hierarchy,
        child_first: crate::edgekind::child_first_from_type(Some("parent_of")),
        label: "parent_of".into(),
        ..edge(id, source, target)
    }
}

fn edge(id: &str, source: &str, target: &str) -> EdgeRecord {
    crate::records::build::edge(id, source, target)
}

/// A straight-line drawing of `topology`'s edges, so a capability can also be checked over
/// geometry that is not the layout's own — the case a `Line` layout hands it.
fn lines(x: &[f32], y: &[f32]) -> Geometry {
    Geometry::planar(
        NodeGeometry::Point {
            x: x.to_vec(),
            y: y.to_vec(),
        },
        EdgeGeometry::Line,
        Vec::new(),
    )
}

/// **The composability gate.** Every registered POST capability over every registered layout:
/// the cross-product, not a sample. Also over a synthetic `Point` drawing, because a
/// capability that assumed its input's edges were already `Polyline` would pass the layout
/// rows and fail this one.
#[test]
fn post_composability() {
    let topology = topology();
    // At least three layouts, or the test is not testing composability. The registry lists
    // five, and every one of them is in the matrix.
    assert!(
        LAYOUTS.len() >= 3,
        "the matrix needs at least three layouts, has {}",
        LAYOUTS.len()
    );
    assert!(!POSTS.is_empty(), "the matrix needs a capability");
    for layout in &LAYOUTS {
        let geometry = (layout.run)(&topology)
            .unwrap_or_else(|e| panic!("{} over the test graph: {e}", layout.id));
        geometry
            .nodes
            .check(topology.node_count(), None)
            .unwrap_or_else(|e| panic!("{} emits bad node geometry: {e}", layout.id));
        let (x, y) = centres(&geometry.nodes);
        for input in [geometry.clone(), lines(x, y)] {
            for cap in &POSTS {
                let Some(bundled) = run_or_declined(cap, &topology, &input) else {
                    continue;
                };
                check_output(&bundled, cap, &topology, layout.id, &input);
            }
        }
    }
}

/// One capability's run, or `None` when it **declined** the geometry for a stated reason.
///
/// The matrix runs every capability over every layout, and five layouts emit a z column. A
/// `moves_nodes: true` row refuses those on purpose — separating 2D discs under a z column
/// would answer a question nobody asked (`docs/decisions/node-overlap.md` §3) — so a refusal
/// is a legitimate outcome, and this is where it is recognised rather than panicked on.
///
/// **The refusal is checked, not merely tolerated.** It must be exactly
/// `Param { name: "geometry.z" }`, and it must only happen when a z column is actually there:
/// a row that refused a planar geometry, or refused for any other reason, is a bug and is
/// panicked on. So a pass cannot pass the matrix by refusing everything, and a pass that
/// quietly processed a z column is caught here rather than in a review.
fn run_or_declined(cap: &Capability, topology: &Topology, input: &Geometry) -> Option<Bundled> {
    match (cap.run)(topology, input) {
        Ok(bundled) => Some(bundled),
        Err(StageError::Param {
            name: "geometry.z",
            rule: "must be absent",
        }) => {
            assert!(
                input.z.is_some(),
                "{} refused a planar geometry: the z refusal is the only one it may make",
                cap.id
            );
            assert!(
                cap.meta.moves_nodes,
                "{} refused a z column without declaring moves_nodes: only a pass that moves \
nodes can need the refusal, and it must say so",
                cap.id
            );
            None
        }
        Err(e) => panic!("{} over the test graph: {e}", cap.id),
    }
}

/// A bundled result is valid only if it is the right kind, fits the topology, and is finite
/// everywhere — and what it must do with the node positions it was handed depends on what the
/// capability **declared**.
///
/// That declaration is the point. The rule used to be flat: *a post pass may not move a node*,
/// asserted over every row. [`separate`] has to break that rule to exist, so the rule became
/// [`Metadata::moves_nodes`] and the assert follows it:
///
/// - `moves_nodes: false` — **the original assert, unchanged and still exact.** Node columns
///   byte-identical to the input. Every bundler and every style is held to the strong claim,
///   which is the point of declaring it rather than assuming it.
/// - `moves_nodes: true` — a weaker but still stated one. The node **kind** is preserved (a
///   pass that turned `Circle`s into `Point`s would be silently downgrading the drawing), the
///   size columns are untouched, every position is finite, and the edges are **byte-identical**
///   to the input, because this row declares `edges` pass-through.
///
/// The z-column assert applies to **both**: a pass carries `z` on or it is not a pass, it is a
/// silent downgrade of a 3D drawing to 2D. `separate` refuses a z column outright rather than
/// moving `x`/`y` under one, so it never reaches here with a `z` in hand.
fn check_output(
    bundled: &Bundled,
    cap: &Capability,
    topology: &Topology,
    layout: &str,
    input: &Geometry,
) {
    let where_ = format!("{} over {}", cap.id, layout);
    let expected = find(cap.id).expect("registered").meta.edges;
    if cap.meta.moves_nodes {
        check_moved_nodes(bundled, &where_, input);
    } else {
        assert_eq!(bundled.geometry.edges.kind(), expected, "{where_}");
        assert_eq!(
            bundled.geometry.nodes, input.nodes,
            "{where_}: a post pass may not move a node"
        );
    }
    bundled
        .geometry
        .edges
        .check(topology.edge_count())
        .unwrap_or_else(|e| panic!("{where_}: {e}"));
    assert_eq!(
        bundled.geometry.z, input.z,
        "{where_}: a post pass may not drop the z column"
    );
    bundled
        .geometry
        .nodes
        .check(topology.node_count(), None)
        .unwrap_or_else(|e| panic!("{where_}: {e}"));
    // Per-capability bounds, not one edge-shaped pair: `separate` counts **node** pairs, so
    // an edge-shaped bound would refuse it at two nodes and mean nothing above three. Neither
    // bound is deleted — each row is held to the count space it actually reports in.
    let (nodes_squared, edges_squared) = if cap.meta.moves_nodes {
        (nodes_squared(topology), topology.node_count())
    } else {
        (edges_squared(topology), topology.edge_count())
    };
    assert!(bundled.pairs <= nodes_squared, "{where_}: pairs");
    assert!(bundled.unbundled <= edges_squared, "{where_}: unbundled");
}

/// What a `moves_nodes: true` row owes instead: the kind and the sizes survive, every
/// coordinate is finite, and the edges come out exactly as they went in.
fn check_moved_nodes(bundled: &Bundled, where_: &str, input: &Geometry) {
    assert_eq!(
        bundled.geometry.nodes.kind(),
        input.nodes.kind(),
        "{where_}: a pass may not change the node kind"
    );
    assert_eq!(
        sizes(&bundled.geometry.nodes),
        sizes(&input.nodes),
        "{where_}: a pass may not resize a node"
    );
    assert_eq!(
        bundled.geometry.edges, input.edges,
        "{where_}: this row declares edges pass-through, so they must be untouched"
    );
    for (label, column) in columns(&bundled.geometry.nodes) {
        assert!(
            column.iter().all(|v| v.is_finite()),
            "{where_}: {label} is not finite"
        );
    }
}

/// The size columns of a node kind, in a fixed order, so two kinds of the same size compare
/// equal — which is what makes "a pass may not resize a node" mean something across kinds.
fn sizes(nodes: &NodeGeometry) -> Vec<Vec<f32>> {
    match nodes {
        NodeGeometry::Point { .. } => Vec::new(),
        NodeGeometry::Circle { r, .. } => vec![r.clone()],
        NodeGeometry::Box { w, h, .. } => vec![w.clone(), h.clone()],
    }
}

/// The coordinate columns of a node kind, in a fixed order.
fn columns(nodes: &NodeGeometry) -> Vec<(&'static str, &Vec<f32>)> {
    match nodes {
        NodeGeometry::Point { x, y }
        | NodeGeometry::Circle { x, y, .. }
        | NodeGeometry::Box { x, y, .. } => {
            vec![("x", x), ("y", y)]
        }
    }
}

/// The largest pair count an **edge**-shaped pass can report: every unordered pair of
/// distinct edges.
fn edges_squared(topology: &Topology) -> u32 {
    let m = topology.edge_count();
    m.saturating_mul(m.saturating_sub(1)) / 2
}

/// The largest pair count a **node**-shaped pass can report: every unordered pair of distinct
/// nodes. `separate` counts node pairs, so this is its ceiling.
fn nodes_squared(topology: &Topology) -> u32 {
    let n = topology.node_count();
    n.saturating_mul(n.saturating_sub(1)) / 2
}
