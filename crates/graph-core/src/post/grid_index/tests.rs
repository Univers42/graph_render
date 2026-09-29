//! The grid index's own tests: the cell arithmetic, the boundary tie-break, node
//! occupancy, and the reused buffer.

use super::*;
use graph_contract::geometry::NodeGeometry;

/// A layout of `n` nodes on a row, one unit apart, centred on the origin.
fn row(n: u32) -> NodeGeometry {
    let half = f64::from(n - 1) / 2.0;
    NodeGeometry::Point {
        x: (0..n).map(|i| f64::from(i) - half).map(|v| v as f32).collect(),
        y: vec![0.0; n as usize],
    }
}

fn small() -> GridParams {
    GridParams {
        resolution: 8,
        margin: 0,
        clearance: 0.0,
    }
}

#[test]
fn a_point_exactly_on_a_boundary_takes_the_cell_that_boundary_opens() {
    // cell = 1.0, origin = 0.0, so 1.0 is exactly the boundary between cells 0 and 1.
    assert_eq!(axis_cell(1.0, 0.0, 1.0, 4), 1, "closed-low, open-high");
    assert_eq!(axis_cell(0.0, 0.0, 1.0, 4), 0);
    assert_eq!(axis_cell(3.0, 0.0, 1.0, 4), 3);
    // Clear of the snap radius on either side, so the rule is the arithmetic and not the
    // tolerance: 1e-6 is a million times BOUNDARY_TOL.
    assert_eq!(axis_cell(1.0 - 1e-6, 0.0, 1.0, 4), 0, "just below the boundary");
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
    assert_eq!(axis_cell(0.3, origin, cell, 4), 2, "snapped to the boundary instead");
    assert_eq!(BOUNDARY_TOL, 1e-9, "a named constant, so the snap is auditable");
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
fn a_degenerate_layout_still_produces_one_cell_per_axis() {
    let mut grid = GridIndex::new();
    let nodes = NodeGeometry::Point {
        x: vec![3.0],
        y: vec![3.0],
    };
    grid.build(&nodes, &small()).expect("builds");
    assert_eq!(grid.shape(), (1, 1), "a single point has no span to divide");
    assert_eq!(grid.cells(), 1);
    assert_eq!(grid.node_cell(0), 0);
    assert!(grid.is_occupied(0));
}

#[test]
fn a_point_node_occupies_its_own_cell_and_the_clearance_ring_around_it() {
    // The corners fix the span at 8, so at resolution 8 the cell is 1 and the centre node's
    // cell is (4, 4). Clearance 1 is the Chebyshev ring around it: 8 cells, 3x3 in total.
    let nodes = NodeGeometry::Point {
        x: vec![4.0, 0.0, 8.0],
        y: vec![4.0, 0.0, 8.0],
    };
    let bare = GridParams {
        resolution: 8,
        margin: 0,
        clearance: 0.0,
    };
    let mut without = GridIndex::new();
    without.build(&nodes, &bare).expect("builds");
    let mut with = GridIndex::new();
    with.build(
        &nodes,
        &GridParams {
            clearance: 1.0,
            ..bare
        },
    )
    .expect("builds");
    assert_eq!(without.node_cell(0), 4 * 8 + 4, "the centre cell, as a number");
    assert_eq!(without.occupied_cells(), 3, "three points, three cells");
    for iy in 3..=5u32 {
        for ix in 3..=5u32 {
            assert!(with.is_occupied(iy * 8 + ix), "the 3x3 block at ({ix}, {iy})");
        }
    }
    assert!(!with.is_occupied(2 * 8 + 4), "two cells out, the ring has stopped");
    // The ring is Chebyshev, so the centre node's own 3x3 is all of it: 9 cells. The
    // corner nodes have theirs clipped by the grid edge — (0, 0) reaches cells 0..1 on
    // each axis and (8, 8) only cell (7, 7) — which is why the total is not 3 + 3 * 9.
    assert_eq!(with.occupied_cells(), 9 + 4 + 1, "9 + a clipped 4 + a clipped 1");
}

/// The two corner nodes at (0, 0) and (8, 8) in the fixtures below are not decoration:
/// they fix the span at 8, so at resolution 8 the cell is exactly 1 and **a cell index is
/// the coordinate**. Every expected occupancy below is then read straight off the numbers.
#[test]
fn a_whole_cell_of_clearance_seals_every_node_which_is_why_the_default_is_zero() {
    // The measurement behind `GridParams::clearance`'s doc, as a test. A node's own cell
    // plus the eight around it is a solid 3 × 3 block, so with a whole cell of clearance
    // **every node is enclosed** and no route can ever leave one. That is not a subtle
    // degradation: it silently disables the capability, which is why the default is 0 and
    // not the obvious 1.
    let nodes = NodeGeometry::Point {
        x: vec![1.0, 4.0, 7.0],
        y: vec![1.0, 4.0, 7.0],
    };
    // `margin: 2` puts every node away from the grid edge, so a node's 3x3 block is whole
    // rather than clipped — the point of the test is the block, not the clipping.
    let params = |clearance| GridParams {
        resolution: 8,
        margin: 2,
        clearance,
    };
    let mut bare = GridIndex::new();
    bare.build(&nodes, &params(0.0)).expect("builds");
    let mut sealed = GridIndex::new();
    sealed.build(&nodes, &params(1.0)).expect("builds");
    assert_eq!(bare.occupied_cells(), 3, "no clearance: three cells, one per node");
    // All three are interior, so all three blocks are whole: 3 x 9.
    assert_eq!(sealed.occupied_cells(), 27, "three solid 3x3 blocks");
    let width = sealed.shape().0;
    // Every one of a node's eight neighbours is blocked.
    for node in 0..3u32 {
        let (cx, cy) = sealed.xy(sealed.node_cell(node));
        for dy in -1..=1i32 {
            for dx in -1..=1i32 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let x = u32::try_from(i64::from(cx) + i64::from(dx)).expect("interior");
                let y = u32::try_from(i64::from(cy) + i64::from(dy)).expect("interior");
                assert!(sealed.is_occupied(y * width + x), "node {node}: ({dx}, {dy}) blocked");
            }
        }
    }
    assert_eq!(
        GridParams::default().clearance,
        0.0,
        "and the default is zero, so the capability is not off"
    );
}

#[test]
fn a_fractional_clearance_widens_a_node_without_sealing_it() {
    // The knob is still useful below one cell: 0.4 of a cell widens each node without
    // closing the ring, so routes exist and keep further off.
    let nodes = NodeGeometry::Point {
        x: vec![1.0, 4.0, 7.0],
        y: vec![1.0, 4.0, 7.0],
    };
    let mut grid = GridIndex::new();
    grid.build(
        &nodes,
        &GridParams {
            resolution: 8,
            margin: 2,
            clearance: 0.4,
        },
    )
    .expect("builds");
    let bare = 3;
    assert!(
        grid.occupied_cells() > bare,
        "wider than {} cells: {}",
        bare,
        grid.occupied_cells()
    );
    for node in 0..3u32 {
        let (cx, cy) = grid.xy(grid.node_cell(node));
        // The ring is not closed: the cell two to the east is free.
        assert!(
            !grid.is_occupied(cy * grid.shape().0 + cx + 2),
            "node {node} can still breathe"
        );
    }
}

#[test]
fn an_unequal_layout_still_gets_square_cells_so_a_diagonal_costs_the_same_either_way() {
    // 2 wide and 8 tall. The cell must come from the **longer** span — here y's — so
    // cell = 8 / 8 = 1 and the grid is 2 x 8, square cells. A per-axis cell would give
    // cell = 0.25 on x and 1.0 on y, and a diagonal's cost would then depend on which
    // diagonal it was — the reference's stated reason for cubic cells (`routed.py::_grid`:
    // "Cells are cubic; with anisotropic ones a diagonal's cost depends on which
    // diagonal").
    let nodes = NodeGeometry::Point {
        x: vec![0.0, 1.0, 2.0],
        y: vec![0.0, 4.0, 8.0],
    };
    let mut grid = GridIndex::new();
    grid.build(
        &nodes,
        &GridParams {
            resolution: 8,
            margin: 0,
            clearance: 0.0,
        },
    )
    .expect("builds");
    assert_eq!(grid.cell_size(), 1.0, "the longer span, y's 8, over resolution 8");
    assert_eq!(grid.shape(), (2, 8), "x is covered at that same cell size, not its own");
    // The consequence, in the only terms that matter downstream: a cell is as wide as it is
    // tall, so `centre` sits half a cell from each edge, and the cell's own lower corner is
    // inside it.
    let (x, y) = grid.centre(0);
    let half = 0.5 * grid.cell_size();
    assert_eq!(grid.cell_of(x - half, y - half), 0, "the cell's lower corner is inside it");
    // The *upper* corner is exactly on the next boundary, so by the module's rule it opens
    // the next cell — which is the boundary tie-break showing up where it matters.
    assert_eq!(grid.cell_of(x + half, y + half), 3, "and it opens the next one");
}

#[test]
fn a_circle_node_occupies_every_cell_its_disc_covers() {
    let nodes = NodeGeometry::Circle {
        x: vec![4.0, 0.0, 8.0],
        y: vec![4.0, 0.0, 8.0],
        r: vec![1.5, 0.0, 0.0],
    };
    let mut grid = GridIndex::new();
    grid.build(&nodes, &GridParams { resolution: 8, margin: 0, clearance: 0.0 })
        .expect("builds");
    assert_eq!(grid.cell_size(), 1.0, "span 8 over resolution 8");
    assert_eq!(grid.shape(), (8, 8));
    // r = 1.5 from (4, 4) reaches x and y in [2.5, 5.5]: cells 2..=5 on each axis, 16 of
    // them. The two corner points add their own single cells.
    for iy in 0..8u32 {
        for ix in 0..8u32 {
            let cell = iy * 8 + ix;
            let in_disc = (2..=5).contains(&ix) && (2..=5).contains(&iy);
            assert_eq!(
                grid.is_occupied(cell),
                in_disc || cell == 0 || cell == 63,
                "cell ({ix}, {iy})"
            );
        }
    }
    assert_eq!(grid.occupied_cells(), 16 + 2);
}

#[test]
fn a_box_node_occupies_every_cell_its_rectangle_covers() {
    // Half-extents 1.0 x 0.5, so x in [3, 5] (cells 3..=5) and y in [3.5, 4.5]
    // (cells 3..=4): six cells, not the nine a square of the same width would take.
    let nodes = NodeGeometry::Box {
        x: vec![4.0, 0.0, 8.0],
        y: vec![4.0, 0.0, 8.0],
        w: vec![2.0, 0.0, 0.0],
        h: vec![1.0, 0.0, 0.0],
    };
    let mut grid = GridIndex::new();
    grid.build(&nodes, &GridParams { resolution: 8, margin: 0, clearance: 0.0 })
        .expect("builds");
    assert_eq!(grid.cell_size(), 1.0);
    for iy in 0..8u32 {
        for ix in 0..8u32 {
            let cell = iy * 8 + ix;
            let in_box = (3..=5).contains(&ix) && (3..=4).contains(&iy);
            assert_eq!(
                grid.is_occupied(cell),
                in_box || cell == 0 || cell == 63,
                "cell ({ix}, {iy})"
            );
        }
    }
    assert_eq!(grid.occupied_cells(), 6 + 2, "the box's 6, plus two corners");
}

#[test]
fn the_margin_puts_empty_cells_around_the_drawing_so_a_route_can_bow_outside_it() {
    let nodes = NodeGeometry::Point {
        x: vec![1.0, 2.0],
        y: vec![1.0, 2.0],
    };
    let mut grid = GridIndex::new();
    grid.build(
        &nodes,
        &GridParams {
            resolution: 2,
            margin: 2,
            clearance: 0.0,
        },
    )
    .expect("builds");
    let (nx, ny) = grid.shape();
    assert_eq!((nx, ny), (6, 6), "2 cells of span at cell 0.5, plus 2 a side");
    let occupied = grid.occupied_cells();
    assert_eq!(occupied, 2, "the two nodes' cells only, one each");
    // The outermost ring is free on every side.
    for x in 0..nx {
        assert!(!grid.is_occupied(x), "bottom row x={x} is clear");
        assert!(!grid.is_occupied((ny - 1) * nx + x), "top row x={x} is clear");
    }
    for y in 0..ny {
        assert!(!grid.is_occupied(y * nx), "left column y={y} is clear");
        assert!(!grid.is_occupied(y * nx + nx - 1), "right column y={y} is clear");
    }
}

#[test]
fn a_cell_centre_lands_back_in_its_own_cell() {
    let mut grid = GridIndex::new();
    grid.build(&row(9), &small()).expect("builds");
    for cell in 0..grid.cells() {
        let (x, y) = grid.centre(cell);
        assert_eq!(grid.cell_of(x, y), cell, "cell {cell} at ({x}, {y})");
    }
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
    assert_eq!((grid.cells(), grid.node_count(), grid.occupied_cells()), (0, 0, 0));
    assert_eq!(grid.cell_of(1.0, 2.0), 0, "no cell to name, so 0");
    assert_eq!(grid.centre(0).0, 0.0, "and no centre to report");
    assert!(!grid.is_occupied(0));
    assert_eq!(grid.node_cell(7), 0, "a node past the layout is 0, never a panic");
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
fn rebuilding_refills_the_same_allocation_rather_than_reallocating() {
    // A layout that needs many cells and many node entries, so the shrink below is visible.
    let wide = NodeGeometry::Point {
        x: (0..33).map(|i| i as f32).collect(),
        y: (0..33).map(|i| (i % 5) as f32).collect(),
    };
    let params = GridParams {
        resolution: 64,
        margin: 2,
        clearance: 0.0,
    };
    let mut grid = GridIndex::new();
    grid.build(&wide, &params).expect("builds");
    let (bytes, cells) = (grid.bytes(), grid.cells());
    assert!(cells > 100, "the wide layout really is wide: {cells} cells");

    // A different layout of the same node count: same buffers, different contents.
    let narrow = NodeGeometry::Point {
        x: (0..33).map(|i| i as f32 * 0.25).collect(),
        y: (0..33).map(|i| (i % 3) as f32 * 0.25).collect(),
    };
    grid.build(&narrow, &params).expect("rebuilds");
    assert_eq!(grid.bytes(), bytes, "the buffers were reused, not reallocated");
    assert!(grid.cells() < cells, "and the smaller layout really is smaller");
    assert!(grid.occupied_cells() > 0, "with its own contents");

    // Shrinking keeps the larger allocation: that is what reuse means, and it is the
    // point of taking the buffer as `&mut self` rather than returning a fresh grid.
    grid.build(&row(3), &params).expect("builds");
    assert_eq!(grid.bytes(), bytes, "a smaller layout does not give memory back");
    assert_eq!(grid.node_count(), 3, "but it does report its own node count");
}

#[test]
fn a_node_index_past_the_layout_is_zero_rather_than_a_panic() {
    let mut grid = GridIndex::new();
    grid.build(&row(3), &small()).expect("builds");
    assert!(grid.node_cell(2) < grid.cells());
    assert_eq!(grid.node_cell(3), 0);
    assert!(!grid.is_occupied(grid.cells()), "a cell past the grid is unoccupied");
    assert!(!grid.is_occupied(u32::MAX));
}
