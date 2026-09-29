use super::*;

#[test]
fn the_polyline_runs_from_the_endpoints_through_the_crossed_cell_centres() {
    let points = spanned(&[(0.0, 4.0), (8.0, 4.0)]);
    let nodes = NodeGeometry::Point {
        x: points.iter().map(|p| p.0).collect(),
        y: points.iter().map(|p| p.1).collect(),
    };
    let mut grid = GridIndex::new();
    grid.build(&nodes, &unit()).expect("builds");
    let edges = columns(points.len(), &[(2, 3)]);
    let routed = route_over(&mut grid, &nodes, &edges).expect("routes");
    let route = routed.route(0);
    assert_eq!(
        route.points.first(),
        Some(&(0.0, 4.0)),
        "starts at the source node"
    );
    assert_eq!(
        route.points.last(),
        Some(&(8.0, 4.0)),
        "ends at the target node"
    );
    // One interior point per crossed cell between the endpoints.
    assert_eq!(route.points.len(), route.cells.len());
    for (point, cell) in route.points[1..route.points.len() - 1]
        .iter()
        .zip(&route.cells[1..route.cells.len() - 1])
    {
        assert_eq!(*point, grid.centre(*cell), "the cell's own centre");
    }
}

#[test]
fn the_paths_column_set_carries_every_route_with_well_formed_offsets() {
    let points = spanned(&[(0.0, 4.0), (4.0, 4.0), (8.0, 4.0)]);
    let nodes = NodeGeometry::Point {
        x: points.iter().map(|p| p.0).collect(),
        y: points.iter().map(|p| p.1).collect(),
    };
    let edges = columns(points.len(), &[(2, 4), (2, 3)]);
    let routed = route(&nodes, &edges, &unit()).expect("routes");
    let paths = routed.paths();
    assert_eq!(paths.offsets.len(), edges.source.len() + 1, "m + 1 offsets");
    assert_eq!(paths.offsets[0], 0, "offsets start at 0");
    assert!(
        paths.offsets.windows(2).all(|w| w[1] >= w[0]),
        "never decreasing"
    );
    assert_eq!(
        paths.pts.len(),
        2 * paths.offsets[paths.offsets.len() - 1] as usize,
        "2 coordinates per point"
    );
    assert!(
        paths.check(edges.source.len() as u32).is_ok(),
        "the contract accepts it"
    );
}

#[test]
fn an_empty_layout_routes_nothing_rather_than_failing() {
    let nodes = NodeGeometry::Point {
        x: Vec::new(),
        y: Vec::new(),
    };
    let edges = index_model(&[] as &[NodeRecord], &[] as &[EdgeRecord])
        .map(|t| t.edges().clone())
        .expect("an empty model fits");
    let routed = route(&nodes, &edges, &unit()).expect("an empty graph routes");
    assert_eq!(routed.fallbacks, 0);
    assert!(routed.routes.is_empty());
}

#[test]
fn a_self_loop_routes_to_its_own_cell_and_reports_no_route_needed() {
    let points = spanned(&[(4.0, 4.0)]);
    let nodes = NodeGeometry::Point {
        x: points.iter().map(|p| p.0).collect(),
        y: points.iter().map(|p| p.1).collect(),
    };
    let mut grid = GridIndex::new();
    grid.build(&nodes, &unit()).expect("builds");
    let edges = columns(points.len(), &[(2, 2)]);
    let routed = route_over(&mut grid, &nodes, &edges).expect("routes");
    assert!(
        routed.route(0).straight_fallback,
        "a self-loop is a point, not a route"
    );
    assert_eq!(routed.route(0).points, vec![(4.0, 4.0), (4.0, 4.0)]);
}
