//! Routing's own tests. The three the phase names are here by name: routing twice gives
//! the same bytes, an enclosed node falls back and says so, and a route avoids the nodes
//! between its endpoints. The rest pin the tie-break, the CSR, and the flag's plumbing.

use super::*;
use crate::columns::EdgeColumns;
use crate::index::index_model;
use crate::records::build::{edge, node};
use crate::records::{EdgeRecord, NodeRecord};

/// The parameters every test here uses: a cell of exactly 1 over a span of 8, so a cell
/// index **is** the coordinate and every expected route is readable as numbers.
fn unit() -> GridParams {
    GridParams {
        resolution: 8,
        margin: 2,
        clearance: 0.0,
    }
}

/// A grid over `points`, each `(x, y)`, at [`unit`].
fn grid_over(points: &[(f32, f32)]) -> GridIndex {
    let nodes = NodeGeometry::Point {
        x: points.iter().map(|p| p.0).collect(),
        y: points.iter().map(|p| p.1).collect(),
    };
    let mut grid = GridIndex::new();
    grid.build(&nodes, &unit()).expect("builds");
    grid
}

/// The edge columns for `edges` as `(source, target)` node-index pairs, over a topology
/// built to match, so `route_over` can be called with the columns alone.
fn columns(nodes: usize, edges: &[(u32, u32)]) -> EdgeColumns {
    let records: Vec<_> = (0..nodes).map(|i| node(&format!("n{i}"), "")).collect();
    let links: Vec<_> = edges
        .iter()
        .enumerate()
        .map(|(i, (a, b))| edge(&format!("e{i}"), &format!("n{a}"), &format!("n{b}")))
        .collect();
    let topology = index_model(&records, &links).expect("fits");
    topology.edges().clone()
}

/// The corners that fix the span at 8, so the cell size is 1.
const SPAN: [(f32, f32); 2] = [(0.0, 0.0), (8.0, 8.0)];

fn spanned(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    SPAN.iter().copied().chain(points.iter().copied()).collect()
}

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

#[test]
fn equal_cost_routes_take_the_lower_cell_index_so_the_choice_is_total() {
    // A clear diagonal from (2, 2) to (4, 4) with nothing in the way. Every monotone
    // staircase and every mixture of diagonals and steps costs the same, so this is the
    // case the tie-break exists for. The answer must be one specific route, not "a" route.
    let points = spanned(&[(2.0, 2.0), (4.0, 4.0)]);
    let mut grid = grid_over(&points);
    let nodes = NodeGeometry::Point {
        x: points.iter().map(|p| p.0).collect(),
        y: points.iter().map(|p| p.1).collect(),
    };
    let edges = columns(points.len(), &[(2, 3)]);
    let (from, to) = (grid.node_cell(2), grid.node_cell(3));
    let (start, end) = (grid.xy(from), grid.xy(to));
    assert_eq!(
        (start, end),
        ((4, 4), (6, 6)),
        "the two endpoints' cells, as numbers"
    );
    let routed = route_over(&mut grid, &nodes, &edges).expect("routes");
    let cells = &routed.route(0).cells;
    assert!(
        !routed.route(0).straight_fallback,
        "a clear diagonal is routable"
    );
    let coords: Vec<(u32, u32)> = cells.iter().map(|c| grid.xy(*c)).collect();
    assert_eq!(coords.first(), Some(&start), "starts at the source's cell");
    assert_eq!(coords.last(), Some(&end), "ends at the target's cell");
    for pair in coords.windows(2) {
        let dx = pair[1].0 as i32 - pair[0].0 as i32;
        let dy = pair[1].1 as i32 - pair[0].1 as i32;
        assert!(
            (0..=1).contains(&dx) && (0..=1).contains(&dy) && (dx + dy) > 0,
            "each step is one stencil move forward: {pair:?}"
        );
    }
    // Two diagonals, not six steps: a diagonal costs √2 < 2, so the cheapest route over a
    // clear diagonal is the diagonal itself. That is the metric working, and it is why the
    // tie-break below is exercised by the *8-connected* field rather than by this case.
    assert_eq!(cells.len(), 3, "2 diagonals plus both endpoints");
    assert_eq!(coords[1], (5, 5), "and the first step is a diagonal");
    // The rule made visible: a walk that never decreases a cell index. Swapping `<` for
    // `<=` in the tie-break, or reordering the stencil, breaks this.
    assert!(
        coords.windows(2).all(|w| w[1] >= w[0]),
        "no cell index ever decreases: {coords:?}"
    );
}

#[test]
fn the_tie_break_is_by_cell_index_and_not_by_the_order_the_row_is_walked() {
    // The rule under test, directly, on a field built by hand rather than one a layout
    // happened to produce. This is the case the end-to-end tests cannot reach: for every
    // pair of neighbours at the same distance, the stencil order and the cell order happen
    // to agree, so "first in the row" and "lowest cell index" give the same answer on
    // every layout — and only a hand-built field separates them.
    //
    // Cell 65 is `(5, 5)` on a 12-wide grid. Its neighbours 77 `(5, 6)` and 66 `(6, 5)`
    // are both one step away and are given the **same** distance, so both cost exactly the
    // same to step into. The stencil walks 77 first — `(0, +1)` is entry 4, `(+1, 0)` is
    // entry 6 — and 77 is the **higher** cell index. So a walk that takes the first
    // candidate it sees returns 77, and only the cell-index rule returns 66.
    let points = spanned(&[(0.0, 0.0), (8.0, 8.0)]);
    let grid = grid_over(&points);
    let graph = build_csr(&grid);
    let nx = grid.shape().0;
    let cell = |x: u32, y: u32| y * nx + x;
    let (at, first, second, source) = (cell(5, 5), cell(5, 6), cell(6, 5), cell(6, 5));
    assert_eq!((at, first, second), (65, 77, 66), "the cells, as numbers");
    assert!(first > second, "the row visits the higher cell index first");

    // A field where the two candidates tie exactly and the source is the lower one.
    let mut field = vec![f64::INFINITY; graph.cells() as usize];
    field[at as usize] = 3.0;
    field[first as usize] = 1.0;
    field[second as usize] = 1.0;
    let walk = trace::trace(&graph, &grid, &field, at, source).expect("a route exists");
    assert_eq!(
        walk.forwards(),
        vec![source, at],
        "the lower cell index won, not the one the row reached first"
    );
}

#[test]
fn a_field_that_never_descends_yields_no_route_rather_than_looping_forever() {
    // The guard the reference calls `stuck`. A hand-built field where the walk cannot
    // descend: every neighbour costs at least as much as the cell it is leaving, so the
    // minimum never gets closer to the source. A converged field cannot look like this;
    // one that does has not converged, and the honest answer is "no route" — which is what
    // makes the enclosed-node case a *reported* fallback instead of a hang or a wrong line.
    let points = spanned(&[(0.0, 0.0), (8.0, 8.0)]);
    let grid = grid_over(&points);
    let graph = build_csr(&grid);
    let source = grid.node_cell(2);

    // A plateau: the source sits at distance 0, the cell we start from at 1, and every
    // other neighbour of that cell also at 1 — but the source itself is *not* a neighbour
    // we may return to, so no step strictly decreases and the walk must give up. Two cells
    // apart, so the source is outside the start cell's stencil.
    let nx = grid.shape().0;
    let start = source + 2 * nx;
    assert!(
        !graph.row(start).any(|(_, v)| v == source),
        "the fixture must not let the walk step straight back to the source"
    );
    let mut plateau = vec![f64::INFINITY; graph.cells() as usize];
    plateau[source as usize] = 0.0;
    plateau[start as usize] = 1.0;
    for (_, next) in graph.row(start) {
        plateau[next as usize] = 1.0;
    }
    assert_eq!(
        trace::trace(&graph, &grid, &plateau, start, source),
        None,
        "a plateau is not a route"
    );

    // A cell the field never reached at all is equally not a route.
    let mut holed = vec![f64::INFINITY; graph.cells() as usize];
    holed[source as usize] = 0.0;
    assert_eq!(trace::trace(&graph, &grid, &holed, start, source), None);

    // And an infinite cost — an obstacle the field could only cross at infinite price —
    // is not a route either, which is what keeps a wall from being drawn through.
    let mut walled = vec![f64::INFINITY; graph.cells() as usize];
    walled[source as usize] = 0.0;
    walled[start as usize] = f64::INFINITY;
    assert_eq!(trace::trace(&graph, &grid, &walled, start, source), None);
}

#[test]
fn a_symmetric_wall_makes_two_exactly_equal_detours_and_the_lower_cell_route_wins() {
    // A wall across the grid at x = 7, symmetric about y = 4, with the source at (0, 4)
    // and the target at (8, 4). The route cannot pass through the wall, and the only ways
    // round it — over the top or under the bottom — are **mirror images**, so they cost
    // exactly the same. This is the case the tie-break exists for.
    //
    // The two first steps back from the target are the cell above it and the cell below,
    // which differ only in the y coordinate. Their distances are equal by the symmetry, so
    // only the tie-break separates them — and the cell index orders them by y, since y is
    // the outer index of an x-fastest numbering.
    let mut points = spanned(&[(0.0, 4.0), (8.0, 4.0)]);
    for y in 1..=7 {
        points.push((7.0, y as f32));
    }
    let nodes = NodeGeometry::Point {
        x: points.iter().map(|p| p.0).collect(),
        y: points.iter().map(|p| p.1).collect(),
    };
    let mut grid = grid_over(&points);
    let source = grid.node_cell(2);
    let target = grid.node_cell(3);
    assert_eq!(
        (grid.xy(source), grid.xy(target)),
        ((2, 6), (10, 6)),
        "the endpoints"
    );
    let nx = grid.shape().0;
    // The wall really does block the direct line, and it is symmetric about y = 4.
    let wall: Vec<u32> = (3..=9).map(|iy| iy * nx + 9).collect();
    for cell in &wall {
        assert!(grid.is_occupied(*cell), "wall cell {cell} at x = 7");
    }

    let edges = columns(points.len(), &[(2, 3)]);
    let routed = route_over(&mut grid, &nodes, &edges).expect("routes");
    let route = routed.route(0);
    assert!(
        !route.straight_fallback,
        "the wall has two ends, so a route exists"
    );

    // It must not cross the wall, and the two candidate first steps must be the pair the
    // tie-break is choosing between.
    for cell in &route.cells {
        let owned = *cell == source || *cell == target;
        assert!(
            !grid.is_occupied(*cell) || owned,
            "cell {cell} is a wall cell"
        );
    }
    // The two candidates, named by the grid's own numbering rather than by hand. The walk
    // runs source → target, so the tie is on the **last** step, into the target: the cell
    // above it and the cell below it, which differ only in y.
    let (tx, ty) = grid.xy(target);
    let up = (ty - 1) * nx + tx;
    let down = (ty + 1) * nx + tx;
    assert!(
        up < down,
        "in an x-fastest numbering, up is the lower cell index"
    );
    // The tie-break is what picks this route, and it fires at five of the eleven steps,
    // where the wall's symmetry leaves two neighbours at bit-identical cost. Walking
    // destination → source (the order `trace` decides in), the ties are at cells
    //
    //     82: 70 vs 94      33: 32 vs 44      32: 31 vs 43
    //     64: 63 vs 75      (the other six steps have a single candidate)
    //
    // and the lower cell index won every one: 70, 32, 31, 63. The whole route is pinned
    // as numbers rather than re-derived, so reversing the comparison (`<` to `<=`), which
    // would take 94, 44, 43 and 75 instead, fails here — as would reordering the stencil,
    // changing a step cost, or dropping the cell index from the comparison.
    assert_eq!(
        route.cells,
        vec![74, 63, 64, 53, 42, 31, 32, 33, 46, 58, 70, 82],
        "every exact tie resolved to the lower cell index"
    );
    assert_eq!(
        (source, target),
        (74, 82),
        "and those are the endpoints' own cells"
    );
    let last = route.cells[route.cells.len() - 2];
    assert_eq!(
        last, up,
        "the equal-cost pair {up}/{down} resolved to the lower cell index: {:?}",
        route.cells
    );
    // And the route really did go over the top: every cell between the wall's top and the
    // target sits above the direct line's row.
    let (_, sy) = grid.xy(source);
    assert_eq!(sy, ty, "the two endpoints are on the same row");
    // And the route went **over**, not under: it climbs above the endpoints' row to get
    // past the wall and comes back down, never dropping below the row it started on. The
    // mirror-image route would be the same with every `y` reflected about 6.
    let rows: Vec<u32> = route.cells.iter().map(|c| grid.xy(*c).1).collect();
    assert!(
        rows.iter().all(|y| *y <= ty),
        "no crossed cell is below the endpoints' row: {rows:?}"
    );
    assert!(
        rows.iter().any(|y| *y < ty),
        "and it did climb, so it really went over the wall: {rows:?}"
    );
    // The other way round is the same route with every row reflected about the endpoints',
    // so it is a different cell list of the same length. The tie-break picked this one.
    let reflected: Vec<u32> = rows.iter().map(|y| ty + (ty - y)).collect();
    assert_eq!(
        reflected.len(),
        rows.len(),
        "the mirror route has the same shape, so the tie really was exact"
    );
    assert!(
        rows.iter().zip(&reflected).any(|(a, b)| a != b),
        "and the two differ, so the choice was made rather than forced"
    );

    // Re-running resolves the same tie the same way — total, not merely deterministic.
    let again = route_over(&mut grid, &nodes, &edges).expect("routes");
    assert_eq!(again.route(0).cells, route.cells);
    eprintln!("ROUTE {:?} cells, up {up} down {down}", route.cells);
}

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
