use super::*;

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
    assert_eq!(
        without.node_cell(0),
        4 * 8 + 4,
        "the centre cell, as a number"
    );
    assert_eq!(without.occupied_cells(), 3, "three points, three cells");
    for iy in 3..=5u32 {
        for ix in 3..=5u32 {
            assert!(
                with.is_occupied(iy * 8 + ix),
                "the 3x3 block at ({ix}, {iy})"
            );
        }
    }
    assert!(
        !with.is_occupied(2 * 8 + 4),
        "two cells out, the ring has stopped"
    );
    // The ring is Chebyshev, so the centre node's own 3x3 is all of it: 9 cells. The
    // corner nodes have theirs clipped by the grid edge — (0, 0) reaches cells 0..1 on
    // each axis and (8, 8) only cell (7, 7) — which is why the total is not 3 + 3 * 9.
    assert_eq!(
        with.occupied_cells(),
        9 + 4 + 1,
        "9 + a clipped 4 + a clipped 1"
    );
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
    assert_eq!(
        bare.occupied_cells(),
        3,
        "no clearance: three cells, one per node"
    );
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
                assert!(
                    sealed.is_occupied(y * width + x),
                    "node {node}: ({dx}, {dy}) blocked"
                );
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
    assert_eq!(
        grid.cell_size(),
        1.0,
        "the longer span, y's 8, over resolution 8"
    );
    assert_eq!(
        grid.shape(),
        (2, 8),
        "x is covered at that same cell size, not its own"
    );
    // The consequence, in the only terms that matter downstream: a cell is as wide as it is
    // tall, so `centre` sits half a cell from each edge, and the cell's own lower corner is
    // inside it.
    let (x, y) = grid.centre(0);
    let half = 0.5 * grid.cell_size();
    assert_eq!(
        grid.cell_of(x - half, y - half),
        0,
        "the cell's lower corner is inside it"
    );
    // The *upper* corner is exactly on the next boundary, so by the module's rule it opens
    // the next cell — which is the boundary tie-break showing up where it matters.
    assert_eq!(
        grid.cell_of(x + half, y + half),
        3,
        "and it opens the next one"
    );
}
