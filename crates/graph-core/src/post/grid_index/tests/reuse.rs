use super::*;

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
    assert_eq!(
        grid.bytes(),
        bytes,
        "the buffers were reused, not reallocated"
    );
    assert!(
        grid.cells() < cells,
        "and the smaller layout really is smaller"
    );
    assert!(grid.occupied_cells() > 0, "with its own contents");

    // Shrinking keeps the larger allocation: that is what reuse means, and it is the
    // point of taking the buffer as `&mut self` rather than returning a fresh grid.
    grid.build(&row(3), &params).expect("builds");
    assert_eq!(
        grid.bytes(),
        bytes,
        "a smaller layout does not give memory back"
    );
    assert_eq!(
        grid.node_count(),
        3,
        "but it does report its own node count"
    );
}
