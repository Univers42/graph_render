//! What a route is never cut short by, and what `route_over` refuses rather than panics on
//! or passes through (review findings R9, M15).

use super::*;

/// A 256 × 256 grid of unit cells, margin 0, crossed by 127 one-cell walls at columns
/// 1, 3, …, 253, each open at alternate ends: the even walls cover rows 0..=254, the odd
/// ones rows 1..=255. Node 0 sits at cell (0, 0) and node 1 at cell (255, 0), so the only
/// route snakes through every gap.
fn serpentine() -> NodeGeometry {
    let mut nodes = (vec![0.0, 256.0], vec![0.0, 0.0], vec![0.0; 2], vec![0.0; 2]);
    for wall in 0..127_u16 {
        let column = f32::from(2 * wall + 1);
        let (y, h) = if wall % 2 == 0 {
            (127.5, 254.5)
        } else {
            (128.625, 254.75)
        };
        nodes.0.push(column + 0.5);
        nodes.1.push(y);
        nodes.2.push(0.5);
        nodes.3.push(h);
    }
    let (x, y, w, h) = nodes;
    NodeGeometry::Box { x, y, w, h }
}

#[test]
fn a_route_longer_than_any_fixed_step_cap_is_still_routed() {
    let nodes = serpentine();
    let params = GridParams {
        resolution: 256,
        margin: 0,
        clearance: 0.0,
    };
    let mut grid = GridIndex::new();
    grid.build(&nodes, &params).expect("builds");
    assert_eq!(grid.shape(), (256, 256));
    let edges = columns(129, &[(0, 1)]);
    let routed = route_over(&mut grid, &nodes, &edges).expect("routes");
    let route = routed.route(0);
    assert!(!route.straight_fallback, "a route exists through the gaps");
    assert!(route.cells.len() > 16_384, "{} cells", route.cells.len());
    let inner = &route.cells[1..route.cells.len() - 1];
    assert!(
        inner.iter().all(|c| !grid.is_occupied(*c)),
        "crosses a wall"
    );
}

#[test]
fn route_over_refuses_a_non_finite_node_rather_than_emitting_it() {
    let mut grid = grid_over(&SPAN);
    let nodes = NodeGeometry::Point {
        x: vec![0.0, f32::NAN],
        y: vec![0.0, 8.0],
    };
    let err = route_over(&mut grid, &nodes, &columns(2, &[(0, 1)])).expect_err("refused");
    assert_eq!(err, StageError::NonFinite { column: "node.x" });
}

#[test]
fn route_over_refuses_an_endpoint_past_the_nodes_rather_than_panicking() {
    let (nodes, mut grid) = bare(&SPAN, 8);
    let err = route_over(&mut grid, &nodes, &columns(3, &[(0, 2)])).expect_err("refused");
    assert!(
        matches!(err, StageError::Param { name: "edges", .. }),
        "{err:?}"
    );
}

#[test]
fn route_over_refuses_a_grid_built_over_other_nodes() {
    let mut grid = grid_over(&spanned(&[(4.0, 4.0)]));
    let nodes = NodeGeometry::Point {
        x: SPAN.iter().map(|p| p.0).collect(),
        y: SPAN.iter().map(|p| p.1).collect(),
    };
    let err = route_over(&mut grid, &nodes, &columns(2, &[(0, 1)])).expect_err("refused");
    assert!(
        matches!(err, StageError::Param { name: "grid", .. }),
        "{err:?}"
    );
}
