use super::*;

#[test]
fn a_point_exactly_on_a_boundary_takes_the_cell_that_boundary_opens() {
    // cell = 1.0, origin = 0.0, so 1.0 is exactly the boundary between cells 0 and 1.
    assert_eq!(axis_cell(1.0, 0.0, 1.0, 4), 1, "closed-low, open-high");
    assert_eq!(axis_cell(0.0, 0.0, 1.0, 4), 0);
    assert_eq!(axis_cell(3.0, 0.0, 1.0, 4), 3);
    // Clear of the snap radius on either side, so the rule is the arithmetic and not the
    // tolerance: 1e-6 is a million times BOUNDARY_TOL.
    assert_eq!(
        axis_cell(1.0 - 1e-6, 0.0, 1.0, 4),
        0,
        "just below the boundary"
    );
    assert_eq!(axis_cell(1.0 + 1e-6, 0.0, 1.0, 4), 1, "just above it");
}

#[test]
fn a_boundary_point_snaps_up_even_when_the_division_rounds_down() {
    // (0.3 - 0.1) / 0.1 is 1.9999999999999998 in f64 — the division genuinely rounds
    // *down*, so a bare `floor` would put a point that is exactly on the boundary into the
    // cell below. The snap is what makes the rule hold instead of following the rounding.
    let (origin, cell) = (0.1_f64, 0.1_f64);
    let q = (0.3_f64 - origin) / cell;
    assert!(q < 2.0, "the quotient itself rounds down: {q}");
    assert_eq!(libm::floor(q) as u32, 1, "a bare floor would answer 1");
    assert_eq!(
        axis_cell(0.3, origin, cell, 4),
        2,
        "snapped to the boundary instead"
    );
    assert_eq!(
        BOUNDARY_TOL, 1e-9,
        "a named constant, so the snap is auditable"
    );
}

#[test]
fn a_coordinate_outside_the_grid_clamps_rather_than_wrapping() {
    assert_eq!(axis_cell(-100.0, 0.0, 1.0, 4), 0);
    assert_eq!(axis_cell(100.0, 0.0, 1.0, 4), 3);
}

#[test]
fn the_cell_size_is_the_longer_span_over_the_resolution_and_cells_stay_square() {
    let mut grid = GridIndex::new();
    // 8 units wide, 4 tall: the span is 8, so cell = 1 and the grid is 8 x 4.
    let nodes = NodeGeometry::Point {
        x: vec![0.0, 4.0, 8.0],
        y: vec![0.0, 2.0, 4.0],
    };
    let params = GridParams {
        resolution: 8,
        margin: 0,
        clearance: 0.0,
    };
    grid.build(&nodes, &params).expect("builds");
    assert_eq!(grid.cell_size(), 1.0, "cubic: the wider axis sets the cell");
    assert_eq!(grid.shape(), (8, 4), "x is the longer axis");
    assert_eq!(grid.cells(), 32);
}

#[test]
fn a_single_point_is_gridded_over_the_references_floor_span() {
    // `routed.py:55` floors each span at 1e-9, so a point with no extent is still divided
    // into `resolution` cells per axis, as the reference divides it.
    let mut grid = GridIndex::new();
    let nodes = NodeGeometry::Point {
        x: vec![3.0],
        y: vec![3.0],
    };
    grid.build(&nodes, &small()).expect("builds");
    assert_eq!(grid.shape(), (8, 8));
    assert_eq!(grid.cells(), 64);
    assert_eq!(grid.node_cell(0), 0);
    assert!(grid.is_occupied(0));
}

#[test]
fn an_empty_layout_gives_an_empty_grid_and_every_query_answers_zero() {
    let mut grid = GridIndex::new();
    grid.build(
        &NodeGeometry::Point {
            x: Vec::new(),
            y: Vec::new(),
        },
        &small(),
    )
    .expect("builds");
    assert_eq!(
        (grid.cells(), grid.node_count(), grid.occupied_cells()),
        (0, 0, 0)
    );
    assert_eq!(grid.cell_of(1.0, 2.0), 0, "no cell to name, so 0");
    assert_eq!(grid.centre(0).0, 0.0, "and no centre to report");
    assert!(!grid.is_occupied(0));
    assert_eq!(
        grid.node_cell(7),
        0,
        "a node past the layout is 0, never a panic"
    );
}

#[test]
fn a_non_finite_coordinate_is_refused_rather_than_producing_a_nan_cell() {
    let mut grid = GridIndex::new();
    let nodes = NodeGeometry::Point {
        x: vec![0.0, f32::NAN],
        y: vec![0.0, 1.0],
    };
    let err = grid.build(&nodes, &small()).expect_err("NaN");
    assert_eq!(err, StageError::NonFinite { column: "node.x" });
}

#[test]
fn a_zero_resolution_is_refused_rather_than_dividing_by_zero() {
    let mut grid = GridIndex::new();
    let err = grid
        .build(
            &row(4),
            &GridParams {
                resolution: 0,
                ..small()
            },
        )
        .expect_err("resolution 0");
    assert_eq!(
        err,
        StageError::Param {
            name: "resolution",
            rule: "at least 1"
        }
    );
}

#[test]
fn a_node_index_past_the_layout_is_zero_rather_than_a_panic() {
    let mut grid = GridIndex::new();
    grid.build(&row(3), &small()).expect("builds");
    assert!(grid.node_cell(2) < grid.cells());
    assert_eq!(grid.node_cell(3), 0);
    assert!(
        !grid.is_occupied(grid.cells()),
        "a cell past the grid is unoccupied"
    );
    assert!(!grid.is_occupied(u32::MAX));
}
