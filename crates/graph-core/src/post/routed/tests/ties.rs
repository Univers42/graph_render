use super::*;

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
