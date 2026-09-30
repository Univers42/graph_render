use super::*;

#[test]
fn an_obstacle_between_the_endpoints_is_avoided_rather_than_crossed() {
    // Three nodes on a row at y = 4: x = 0, 4, 8. The middle one is an obstacle, so a
    // route from the first to the last must bow off the row to get past it.
    let points = spanned(&[(0.0, 4.0), (4.0, 4.0), (8.0, 4.0)]);
    let grid = grid_over(&points);
    let nodes = NodeGeometry::Point {
        x: points.iter().map(|p| p.0).collect(),
        y: points.iter().map(|p| p.1).collect(),
    };
    let edges = columns(points.len(), &[(2, 4)]);
    let routed = route_over(&mut grid.clone(), &nodes, &edges).expect("routes");
    let route = routed.route(0);
    assert!(
        !route.straight_fallback,
        "a route exists: the row is not sealed"
    );
    // The straight segment from (0, 4) to (8, 4) runs through the middle node's cells.
    // No crossed cell may be that node's.
    let middle = grid.node_cell(3);
    assert!(
        !route.cells.contains(&middle),
        "the obstacle's own cell is not crossed"
    );
    // And no crossed cell is occupied at all, other than the two endpoints' own.
    for cell in &route.cells {
        let owned = *cell == grid.node_cell(2) || *cell == grid.node_cell(4);
        assert!(
            !grid.is_occupied(*cell) || owned,
            "cell {cell} is an obstacle"
        );
    }
    // It really did detour: more than the 8 cells of the straight run.
    assert!(
        route.points.len() > 2,
        "bowed off the row: {:?}",
        route.points
    );
    assert_eq!(routed.fallbacks, 0);
}

#[test]
fn routing_twice_over_the_same_input_gives_the_same_bytes() {
    let points = spanned(&[(0.0, 4.0), (4.0, 4.0), (8.0, 4.0), (2.0, 1.0), (6.0, 7.0)]);
    let nodes = NodeGeometry::Point {
        x: points.iter().map(|p| p.0).collect(),
        y: points.iter().map(|p| p.1).collect(),
    };
    let edges = columns(points.len(), &[(2, 4), (2, 5), (5, 4), (3, 2), (4, 3)]);
    let first = route(&nodes, &edges, &unit()).expect("routes");
    let second = route(&nodes, &edges, &unit()).expect("routes");
    assert_eq!(first, second, "the whole output, not just the routes");
    // A third pass over a *reused* grid must agree too: the buffer is refilled, not
    // appended to, so a second build cannot inherit the first one's cells.
    let mut reused = GridIndex::new();
    reused.build(&nodes, &unit()).expect("builds");
    let a = route_over(&mut reused, &nodes, &edges).expect("routes");
    let b = route_over(&mut reused, &nodes, &edges).expect("routes");
    assert_eq!(a, b);
    assert_eq!(a, first, "and a reused buffer gives what a fresh one gives");
}

#[test]
fn an_enclosed_node_falls_back_to_the_straight_segment_and_sets_the_flag() {
    // A ring of eight nodes around (4, 4), with a ninth inside it. The inside node's
    // every neighbour cell is a ring node's, so no route leaves it.
    let mut points = spanned(&[(4.0, 4.0)]);
    for (dx, dy) in [
        (-1.0, -1.0),
        (0.0, -1.0),
        (1.0, -1.0),
        (-1.0, 0.0),
        (1.0, 0.0),
        (-1.0, 1.0),
        (0.0, 1.0),
        (1.0, 1.0),
    ] {
        points.push((4.0 + dx, 4.0 + dy));
    }
    let nodes = NodeGeometry::Point {
        x: points.iter().map(|p| p.0).collect(),
        y: points.iter().map(|p| p.1).collect(),
    };
    // Node 2 is the enclosed one; node 0 is the far corner.
    let edges = columns(points.len(), &[(2, 0)]);
    let mut grid = GridIndex::new();
    grid.build(&nodes, &unit()).expect("builds");
    let routed = route_over(&mut grid, &nodes, &edges).expect("routes");
    let route = routed.route(0);
    assert!(route.straight_fallback, "no route out of an enclosed node");
    assert_eq!(routed.fallbacks, 1, "and the count says so");
    assert!(route.cells.is_empty(), "a fallback claims no cells");
    // The straight segment: the two endpoints and nothing else, so the contract draws it
    // straight rather than routing it through the ring.
    assert_eq!(route.points, vec![(4.0, 4.0), (0.0, 0.0)]);
    assert!(route.is_straight());
}

#[test]
fn a_sealed_ring_around_an_edge_falls_back_and_never_claims_a_route() {
    // Same ring, but the *edge* runs from one ring node to the other across the middle.
    // The enclosed node is in the way and may not be crossed, so this must fall back too.
    let mut points = spanned(&[(4.0, 4.0)]);
    for (dx, dy) in [
        (-1.0, -1.0),
        (0.0, -1.0),
        (1.0, -1.0),
        (-1.0, 0.0),
        (1.0, 0.0),
        (-1.0, 1.0),
        (0.0, 1.0),
        (1.0, 1.0),
    ] {
        points.push((4.0 + dx, 4.0 + dy));
    }
    let nodes = NodeGeometry::Point {
        x: points.iter().map(|p| p.0).collect(),
        y: points.iter().map(|p| p.1).collect(),
    };
    // Nodes 3 (3, 4) and 5 (5, 4) are ring members either side of the enclosed node.
    let edges = columns(points.len(), &[(3, 5)]);
    let mut grid = GridIndex::new();
    grid.build(&nodes, &unit()).expect("builds");
    let routed = route_over(&mut grid, &nodes, &edges).expect("routes");
    let route = routed.route(0);
    // The ring is not sealed at the top and bottom, so a route may exist; what it must
    // never do is cross the enclosed node's cell.
    let enclosed = grid.node_cell(2);
    assert!(
        !route.cells.contains(&enclosed),
        "never through the enclosed node"
    );
}

#[test]
fn two_edges_whose_nodes_share_a_cell_report_a_straight_segment_honestly() {
    // Two nodes at the same position: same cell, so there is nothing to route around.
    let points = spanned(&[(4.0, 4.0), (4.0, 4.0)]);
    let nodes = NodeGeometry::Point {
        x: points.iter().map(|p| p.0).collect(),
        y: points.iter().map(|p| p.1).collect(),
    };
    let mut grid = GridIndex::new();
    grid.build(&nodes, &unit()).expect("builds");
    let edges = columns(points.len(), &[(2, 3)]);
    let routed = route_over(&mut grid, &nodes, &edges).expect("routes");
    let route = routed.route(0);
    assert!(
        route.straight_fallback,
        "flagged, so a caller is not misled"
    );
    assert!(route.cells.is_empty(), "and it claims no cells");
    assert_eq!(routed.fallbacks, 1);
}
