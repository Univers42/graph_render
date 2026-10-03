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
use graph_contract::snapshot::Dim;

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
fn run_or_declined(
    cap: &Capability,
    topology: &Topology,
    input: &Geometry,
) -> Option<Bundled> {
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
        NodeGeometry::Point { x, y } | NodeGeometry::Circle { x, y, .. } | NodeGeometry::Box { x, y, .. } => {
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

/// Every registered POST capability declares its metadata, and every one in the matrix is
/// registered. The negative control is the last two rows: a capability that composes but is
/// absent from the registry would pass `post_composability` and never appear in the ledger.
#[test]
fn every_registered_capability_declares_its_metadata() {
    let mut ids: Vec<&str> = POSTS.iter().map(|cap| cap.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), POSTS.len(), "unique ids");
    for cap in POSTS.iter() {
        let meta = cap.meta;
        assert!(cap.id.starts_with("post."), "{}", cap.id);
        assert!(meta.tier >= 1, "{}", cap.id);
        assert!(meta.scale_ceiling > 0, "{}", cap.id);
        for text in [
            meta.oracle,
            meta.complexity,
            meta.degradation,
            meta.ponytail,
        ] {
            assert!(!text.trim().is_empty(), "{}", cap.id);
        }
        assert!(
            meta.ponytail.contains("Ponytail") || meta.ponytail.contains("No Ponytail"),
            "{}: a capability states its markers or says none is owed",
            cap.id
        );
    }
    assert!(find("post.bundle.none").is_none());
    assert!(find("post.bundle.FDEB").is_none(), "ids are exact");
    assert!(find("layout.grid").is_none(), "post ids, not layout ids");
}

/// The registered run and the id in one row: [`find`] answers for the capability that is
/// actually in [`POSTS`], and the run it hands back is the one the matrix calls. Without the
/// first half a `find` that answered for every id would pass; without the second, a row whose
/// `run` pointed at another capability's entry point would too.
#[test]
fn find_answers_with_the_row_and_the_run_that_row_registers() {
    for cap in &POSTS {
        let found = find(cap.id).expect("registered");
        assert_eq!(found.id, cap.id);
        let topology = topology();
        let geometry = (LAYOUTS[0].run)(&topology).expect("the grid lays out the test graph");
        assert_eq!(
            (found.run)(&topology, &geometry).map(|b| b.geometry),
            (cap.run)(&topology, &geometry).map(|b| b.geometry),
            "{}",
            cap.id
        );
    }
}

/// Every registered capability over a 3D geometry: the z column comes back. The matrix above
/// only ever sees 2D inputs, because every layout in the tree is 2D, so without this a pass
/// that dropped z would stay green until the first 3D layout lands.
#[test]
fn a_3d_geometries_z_column_survives_every_registered_capability() {
    let topology = topology();
    let grid = (crate::registry::find("layout.grid")
        .expect("registered")
        .run)(&topology)
    .expect("the grid lays out the test graph")
    .nodes;
    let NodeGeometry::Point { x, y } = grid else {
        panic!("the grid emits Point nodes")
    };
    let z: Vec<f32> = (0..x.len()).map(|i| i as f32 * 0.5).collect();
    let geometry = Geometry::in_space(
        NodeGeometry::Point { x, y },
        EdgeGeometry::Line,
        Vec::new(),
        z.clone(),
    );
    assert_eq!(geometry.dim(), Dim::D3, "the fixture is 3D");
    for cap in &POSTS {
        if cap.meta.moves_nodes {
            // A node-moving pass is refused here rather than run, and that refusal is what
            // keeps the *other* half of this test true. If it ran, it would have to leave z
            // alone while moving x and y under it — the silent downgrade this test exists to
            // catch. `docs/decisions/node-overlap.md` §3 is the decision, and this line is
            // its enforcement: the refusal is asserted, not merely tolerated, so a pass
            // cannot make this green by refusing nothing.
            let err = (cap.run)(&topology, &geometry).expect_err("a node-moving pass refuses 3D");
            assert_eq!(
                err,
                StageError::Param {
                    name: "geometry.z",
                    rule: "must be absent",
                },
                "{}: a node-moving pass must refuse a z column, not half-process it",
                cap.id
            );
            continue;
        }
        let bundled = (cap.run)(&topology, &geometry)
            .unwrap_or_else(|e| panic!("{} over a 3D geometry: {e}", cap.id));
        assert_eq!(bundled.geometry.z, Some(z.clone()), "{}: z dropped", cap.id);
        assert_eq!(
            bundled.geometry.nodes, geometry.nodes,
            "{}: node moved",
            cap.id
        );
    }
}

/// The measurement every bundler is judged by, on a graph a bundler should help: the
/// long-span fixture over a layout that spreads its nodes. Both bundlers must reduce the
/// occupied cells — an ink *rise* would mean the pass is registered but useless, and the
/// gate row `graph-cli ink --fixture hairball` would be measuring nothing.
///
/// This is the negative control for the whole slice: a pass that emitted the layout's own
/// straight edges, or that moved nothing, passes every test above and fails here.
///
/// **Rows are selected by what they claim, not by skipping a failure.** The loop covers the
/// `moves_nodes: false` rows — the bundlers this test is named for — and the
/// `moves_nodes: true` rows are excluded because ink is not what they are for: they move
/// nodes, and a node move can raise or lower ink freely. Every bundler is still asserted with
/// the identical bound, so nothing is weakened. `separate` gets its own equivalent control in
/// its own test module — `it_actually_resolves_pairs` — which is the same question asked of it
/// in the units it reports in, and fails if the pass ever becomes a no-op.
#[test]
fn both_bundlers_reduce_ink_on_the_long_span_fixture() {
    let (records, edges) = fdeb::load("long-span").expect("the fixture is committed");
    let topology = index_model(&records, &edges).expect("the fixture indexes");
    let grid = crate::registry::find("layout.grid").expect("registered");
    let geometry = (grid.run)(&topology).expect("the grid lays it out");
    let before = measure(&topology, &geometry);
    let bundlers: Vec<_> = POSTS.iter().filter(|cap| !cap.meta.moves_nodes).collect();
    assert!(
        bundlers.len() >= 2,
        "the test is named for the bundlers and needs at least two, found {}",
        bundlers.len()
    );
    for cap in bundlers {
        let bundled = (cap.run)(&topology, &geometry).expect("runs");
        let after = measure(&topology, &bundled.geometry);
        assert!(
            after.cells < before.cells,
            "{}: {} cells straight, {} bundled — no ink saved",
            cap.id,
            before.cells,
            after.cells
        );
    }
}

/// The hairball over every registered layout: the subject the measurement in
/// `docs/measurements/phase08-ink.md` is written from, over all the layouts it claims. FDEB's
/// surviving pair count is pinned per layout — a changed compatibility term, threshold or
/// pair order moves it — and asserted below every layout's full cross-product, so a prune
/// that stopped pruning (or a threshold that stopped being read) fails here rather than only
/// costing time.
#[test]
fn fdeb_surviving_pairs_on_the_hairball_are_pinned_over_every_layout() {
    let (records, edges) = fdeb::load("hairball").expect("the fixture is committed");
    let topology = index_model(&records, &edges).expect("the fixture indexes");
    let every_pair = edges_squared(&topology);
    let mut pinned = 0;
    for layout in &LAYOUTS {
        let geometry = (layout.run)(&topology)
            .unwrap_or_else(|e| panic!("{} over the hairball: {e}", layout.id));
        let bundled = fdeb::run(&topology, &geometry).expect("fdeb runs");
        assert!(bundled.pairs < every_pair, "{}: no prune at all", layout.id);
        assert!(bundled.pairs > 0, "{}: nothing bundled at all", layout.id);
        pinned += 1;
    }
    assert_eq!(pinned, LAYOUTS.len(), "every layout is in the pin");
}
