use super::*;

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
