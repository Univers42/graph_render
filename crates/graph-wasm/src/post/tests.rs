//! The ABI's POST registry, its adapters, and every branch they can take. Natively
//! testable because `super` is: the table is plain data and every entry point is a
//! `PostRun` over graph-core's own types, so the wasm32-only export is a three-line
//! delegation (C21) and what it delegates to is pinned here.

use super::*;
use crate::ingest;
use graph_contract::geometry::{EdgeGeometry, EdgeGeometryKind, NodeGeometry, Paths};
use graph_contract::notes::{Note, NoteCode};
use graph_core::Topology;
use graph_core::index_model;
use graph_core::post::grid_index::GridParams;
use graph_core::post::styles::Style;

/// The ids this table must answer with, in order. Written out as literals rather than
/// read back from the table: a registry that answered for whatever it happened to hold
/// would pass a test that only checks it against itself.
const IDS: [&str; 7] = [
    "post.bundle.fdeb",
    "post.bundle.mingle",
    "post.route.grid",
    "post.style.straight",
    "post.style.orthogonal",
    "post.style.quadratic",
    "post.style.bezier",
];

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
fn topology(node_ids: &[&str], edges: &[(&str, &str, &str)]) -> Topology {
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
fn pair() -> (Topology, Geometry) {
    let t = topology(&["a", "b"], &[("e", "a", "b")]);
    (t, points(&[0.0, 10.0], &[0.0, 0.0], &[]))
}

fn points(x: &[f32], y: &[f32], notes: &[Note]) -> Geometry {
    Geometry {
        nodes: NodeGeometry::Point {
            x: x.to_vec(),
            y: y.to_vec(),
        },
        edges: EdgeGeometry::Line,
        notes: notes.to_vec(),
    }
}

fn paths(edges: &EdgeGeometry) -> &Paths {
    match edges {
        EdgeGeometry::Polyline(p) | EdgeGeometry::Curve { paths: p, .. } => p,
        EdgeGeometry::Line => panic!("a Line edge kind stores no path"),
    }
}

/// `None` for a `Line`, which stores no row at all — the one capability that does, so a
/// caller must not ask its columns for points.
fn row_points(edges: &EdgeGeometry) -> Option<Vec<f32>> {
    match edges {
        EdgeGeometry::Line => None,
        _ => Some(paths(edges).pts.clone()),
    }
}

fn run_at(t: &Topology, g: &Geometry, id: &str) -> Bundled {
    let i = index_of(id);
    (CAPABILITIES[i].run)(t, g).unwrap_or_else(|e| panic!("{id} over the pair fixture: {e}"))
}

fn index_of(id: &str) -> usize {
    IDS.iter()
        .position(|&candidate| candidate == id)
        .unwrap_or_else(|| panic!("{id} is not one of the pinned ids"))
}

#[test]
fn the_registry_is_the_two_bundlers_then_routing_then_the_four_styles() {
    assert_eq!(count(), 7);
    for (i, id) in IDS.iter().enumerate() {
        assert_eq!(id_at(u32::try_from(i).expect("small")), Some(*id));
    }
    assert_eq!(id_at(7), None, "one past the end is refused, not a panic");
    assert_eq!(id_at(u32::MAX), None);
}

#[test]
fn running_past_the_end_is_refused_rather_than_answered_with_another_row() {
    let (t, g) = pair();
    assert!(run(7, &t, &g).is_none(), "index 7 is one past the last row");
    assert!(run(u32::MAX, &t, &g).is_none());
    assert!(
        run(0, &t, &g).expect("row 0 exists").is_ok(),
        "row 0 itself runs"
    );
}

#[test]
fn every_capability_replaces_the_edges_and_leaves_the_nodes_and_the_notes_alone() {
    let note = Note {
        code: NoteCode::EdgeReversed,
        index: 0,
    };
    let (t, _) = pair();
    let input = points(&[0.0, 10.0], &[0.0, 0.0], &[note]);
    for id in IDS {
        let bundled = run_at(&t, &input, id);
        assert_eq!(bundled.geometry.nodes, input.nodes, "{id}: nodes moved");
        assert_eq!(bundled.geometry.notes, input.notes, "{id}: notes lost");
        bundled
            .geometry
            .edges
            .check(t.edge_count())
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        assert!(
            bundled.unbundled <= t.edge_count(),
            "{id}: more unbundled edges than edges"
        );
    }
}

#[test]
fn straight_stores_nothing_and_the_three_row_storing_styles_write_their_rows() {
    let (t, g) = pair();
    let kinds: Vec<_> = IDS
        .iter()
        .map(|id| {
            let bundled = run_at(&t, &g, id);
            (
                bundled.geometry.edges.kind(),
                row_points(&bundled.geometry.edges),
            )
        })
        .collect();
    assert_eq!(kinds[0].0, EdgeGeometryKind::Polyline, "fdeb");
    assert_eq!(kinds[1].0, EdgeGeometryKind::Polyline, "mingle");
    assert_eq!(kinds[2].0, EdgeGeometryKind::Polyline, "route");
    assert_eq!(kinds[3].0, EdgeGeometryKind::Line, "straight");
    assert_eq!(kinds[4].0, EdgeGeometryKind::Polyline, "orthogonal");
    assert_eq!(kinds[5].0, EdgeGeometryKind::Curve, "quadratic");
    assert_eq!(kinds[6].0, EdgeGeometryKind::Curve, "bezier");
    assert_eq!(kinds[3].1, None, "straight stores no row at all");
    // The chord is (0,0) -> (10,0) and the group holds one edge, so the fan offset is
    // zero and every coordinate below is the generator's own arithmetic: the bulge goes
    // to the side `p0.x + p0.y > p1.x + p1.y` does not pick, i.e. down in `x`, and the
    // clockwise perpendicular of a chord along +x is (0, -|d|).
    assert_eq!(
        kinds[4].1,
        Some(vec![5.0, 0.0, 5.0, 0.0]),
        "Z: two level bends at the chord's midpoint"
    );
    assert_eq!(
        kinds[5].1,
        Some(vec![5.0, 1.5]),
        "one control point, half a bulge off the chord"
    );
    assert_eq!(
        kinds[6].1,
        Some(vec![2.5, 0.75, 7.5, 0.75]),
        "two, at a quarter and three quarters"
    );
}

/// The curve degree a style stamps is part of its contract, and a style that stamped the
/// other one's degree would draw a different curve from the same control points.
#[test]
fn each_curve_style_stamps_its_own_degree() {
    let (t, g) = pair();
    for (id, want) in [("post.style.quadratic", 2u32), ("post.style.bezier", 3u32)] {
        let EdgeGeometry::Curve { degree, .. } = run_at(&t, &g, id).geometry.edges else {
            panic!("{id} emits a Curve");
        };
        assert_eq!(degree, want, "{id}");
    }
}

/// The CSR every row-storing capability writes has to be well formed at every boundary
/// — `m + 1` offsets from 0, non-decreasing, and `2 x` the last offset scalars — or
/// `gm_column_ptr`/`gm_column_len` hand a caller a row that reads another edge's points.
#[test]
fn every_row_storing_capability_writes_a_well_formed_csr() {
    let t = topology(
        &["a", "b", "c", "z"],
        &[
            ("e0", "a", "b"),
            ("e1", "b", "c"),
            ("e2", "c", "a"),
            ("loop", "a", "a"),
        ],
    );
    let g = points(&[0.0, 10.0, 5.0, 5.0], &[0.0, 0.0, 8.0, 0.0], &[]);
    let m = t.edge_count() as usize;
    for id in IDS {
        let edges = run_at(&t, &g, id).geometry.edges;
        let EdgeGeometry::Line = edges else {
            let p = paths(&edges);
            assert_eq!(p.offsets.len(), m + 1, "{id}: offsets length");
            assert_eq!(p.offsets[0], 0, "{id}: the first offset");
            assert_eq!(
                p.pts.len() / 2,
                *p.offsets.last().expect("at least one offset") as usize,
                "{id}: 2 x the last offset"
            );
            for (e, pair) in p.offsets.windows(2).enumerate() {
                assert!(
                    pair[0] <= pair[1],
                    "{id}: offsets never decrease at edge {e}"
                );
            }
            assert!(
                p.pts.iter().all(|v| v.is_finite()),
                "{id}: D9, no non-finite coordinate reaches the wire"
            );
            continue;
        };
        assert_eq!(id, "post.style.straight", "only straight stores no row");
    }
}

/// Routing draws the straight segment when no route exists, and reports it: the count
/// the caller acts on is `Bundled::unbundled`, not a log line a downstream program
/// cannot read. Two nodes with a clear field between them route, so it reads 0 here.
#[test]
fn routing_reports_its_straight_fallbacks_as_unbundled_and_routes_a_clear_field() {
    let (t, g) = pair();
    let routed = run_at(&t, &g, "post.route.grid");
    let direct = graph_core::post::routed::route(&g.nodes, t.edges(), &GridParams::default())
        .expect("the grid builds");
    assert_eq!(routed.unbundled, direct.fallbacks);
    assert_eq!(routed.unbundled, 0, "nothing is enclosed here");
    assert_eq!(
        routed.geometry.edges,
        EdgeGeometry::Polyline(direct.paths()),
        "the adapter must hand back the route it computed, unaltered"
    );
    let row = paths(&routed.geometry.edges);
    assert_eq!(row.offsets, vec![0, direct.routes[0].points.len() as u32]);
    assert_eq!(row.pts[0], 0.0, "a route starts at its source node");
    assert_eq!(row.pts[1], 0.0);
    assert_eq!(row.pts[row.pts.len() - 2], 10.0, "and ends at its target");
    assert_eq!(row.pts[row.pts.len() - 1], 0.0);
}

/// The fallback count is only pinned if some fixture actually falls back, **and only
/// distinguishes a count from a clamp if more than one edge does**. Two nodes a layout
/// put on one spot share a cell, there is nothing to route between them, and the straight
/// segment is used and reported — the case `routed.rs` names as a fallback. This fixture
/// has three such edges, so a pass that reported "at most one fallback" instead of the
/// count would read `1` here and be caught; a two-edge fixture would not.
#[test]
fn a_graph_whose_every_edge_shares_a_cell_reports_one_fallback_per_edge() {
    let t = topology(
        &["a", "b", "c"],
        &[("e0", "a", "b"), ("e1", "b", "c"), ("e2", "a", "c")],
    );
    let g = points(&[4.0, 4.0, 4.0], &[4.0, 4.0, 4.0], &[]);
    let direct = graph_core::post::routed::route(&g.nodes, t.edges(), &GridParams::default())
        .expect("the grid builds");
    assert_eq!(
        direct.fallbacks, 3,
        "every edge shares a cell: nothing to route"
    );
    let bundled = run_at(&t, &g, "post.route.grid");
    assert_eq!(bundled.unbundled, 3, "one per edge, not a clamped count");
    assert_eq!(
        bundled.geometry.edges,
        EdgeGeometry::Polyline(direct.paths())
    );
    let row = paths(&bundled.geometry.edges);
    assert_eq!(
        row.pts,
        vec![4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0],
        "each row is its own straight segment"
    );
}

/// The two bundlers are graph-core's own entry points, reached through this table: a row
/// that pointed at another capability's `run` would still compose, and still emit the
/// right geometry kind, and would be wrong.
#[test]
fn the_two_bundler_rows_run_graph_cores_own_entry_points() {
    let (t, g) = pair();
    assert_eq!(
        run_at(&t, &g, "post.bundle.fdeb").geometry,
        graph_core::post::fdeb::run(&t, &g)
            .expect("fdeb runs")
            .geometry
    );
    assert_eq!(
        run_at(&t, &g, "post.bundle.mingle").geometry,
        graph_core::post::mingle::run(&t, &g)
            .expect("mingle runs")
            .geometry
    );
}

/// A capability has to work over whatever a layout emitted, not only over a hand-written
/// `Point` drawing: this is the composability claim, restated over the ABI's own table.
#[test]
fn every_capability_composes_with_every_registered_layout() {
    use graph_core::registry::LAYOUTS;
    let t = topology(
        &["a", "b", "c", "d"],
        &[
            ("e0", "a", "b"),
            ("e1", "b", "c"),
            ("e2", "c", "a"),
            ("e3", "a", "d"),
        ],
    );
    for layout in LAYOUTS {
        let geometry = (layout.run)(&t).unwrap_or_else(|e| panic!("{}: {e}", layout.id));
        for id in IDS {
            let bundled = run_at(&t, &geometry, id);
            assert_eq!(
                bundled.geometry.nodes, geometry.nodes,
                "{id} over {}",
                layout.id
            );
            bundled
                .geometry
                .edges
                .check(t.edge_count())
                .unwrap_or_else(|e| panic!("{id} over {}: {e}", layout.id));
        }
    }
}

/// A `Circle` layout's radius column must survive a post pass untouched, or a caller
/// reading `r` after bundling would draw the wrong circles.
#[test]
fn a_circle_layouts_radius_column_survives_a_post_pass() {
    let t = topology(&["a", "b"], &[("e", "a", "b")]);
    let g = (graph_core::registry::find("layout.packing.circle")
        .expect("registered")
        .run)(&t)
    .expect("packs");
    assert!(matches!(g.nodes, NodeGeometry::Circle { .. }));
    for id in IDS {
        let bundled = run_at(&t, &g, id);
        assert_eq!(bundled.geometry.nodes, g.nodes, "{id}");
    }
}

/// The style ids come from `Style::ALL` in graph-core, in that order: a fourth style
/// added there must appear here, not be silently unroutable.
#[test]
fn the_four_style_rows_are_the_four_style_variants_at_their_own_defaults() {
    let styles: Vec<Style> = Style::ALL.into_iter().collect();
    assert_eq!(styles.len(), 4);
    for (row, style) in styles.iter().enumerate() {
        let i = index_of(style.id());
        assert_eq!(i, 3 + row, "row {row} is {}", style.id());
    }
}
