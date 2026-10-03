//! Row correctness tests.

use super::fixtures::*;
use graph_core::post::{fdeb, mingle, routed};

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
        &["a", "b", "c", "d"],
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
        assert!(
            id == "post.style.straight" || id == super::fixtures::NODE_MOVER,
            "only straight and the node mover store no row: {id} stores none and is neither"
        );
    }
}

#[test]
fn routing_reports_its_straight_fallbacks_as_unbundled_and_routes_a_clear_field() {
    let (t, g) = pair();
    let routed = run_at(&t, &g, "post.route.grid");
    let direct =
        routed::route(&g.nodes, t.edges(), &GridParams::default()).expect("the grid builds");
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
    let direct =
        routed::route(&g.nodes, t.edges(), &GridParams::default()).expect("the grid builds");
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
        fdeb::run(&t, &g).expect("fdeb runs").geometry
    );
    assert_eq!(
        run_at(&t, &g, "post.bundle.mingle").geometry,
        mingle::run(&t, &g).expect("mingle runs").geometry
    );
}
