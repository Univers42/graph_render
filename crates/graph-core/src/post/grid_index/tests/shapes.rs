use super::*;

#[test]
fn a_circle_node_occupies_every_cell_its_disc_covers() {
    let nodes = NodeGeometry::Circle {
        x: vec![4.0, 0.0, 8.0],
        y: vec![4.0, 0.0, 8.0],
        r: vec![1.5, 0.0, 0.0],
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
    grid.build(
        &nodes,
        &GridParams {
            resolution: 8,
            margin: 0,
            clearance: 0.0,
        },
    )
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
    assert_eq!(
        grid.occupied_cells(),
        6 + 2,
        "the box's 6, plus two corners"
    );
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
    assert_eq!(
        (nx, ny),
        (6, 6),
        "2 cells of span at cell 0.5, plus 2 a side"
    );
    let occupied = grid.occupied_cells();
    assert_eq!(occupied, 2, "the two nodes' cells only, one each");
    // The outermost ring is free on every side.
    for x in 0..nx {
        assert!(!grid.is_occupied(x), "bottom row x={x} is clear");
        assert!(
            !grid.is_occupied((ny - 1) * nx + x),
            "top row x={x} is clear"
        );
    }
    for y in 0..ny {
        assert!(!grid.is_occupied(y * nx), "left column y={y} is clear");
        assert!(
            !grid.is_occupied(y * nx + nx - 1),
            "right column y={y} is clear"
        );
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
