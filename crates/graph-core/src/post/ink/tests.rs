use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};
use graph_contract::geometry::{EdgeGeometry, Paths};

#[test]
fn a_box_too_small_to_divide_is_one_cell_and_walks_in_bounded_steps() {
    // R17. No registry oracle measures the raster; the expected value is this module's own
    // degenerate-axis rule (`Raster::over`): an axis f32 cannot divide into
    // INK_RESOLUTION cells is one cell. 128 / 1e-37 overflows f32 to +inf, the half-cell
    // step was then 0, and the walk 1e-37 / 0 saturated to u32::MAX steps per segment.
    let raster = Raster::over(&[0.0, 1e-37], &[0.0, 0.0]);
    let steps = raster.steps(1e-37, 0.0);
    assert!(
        steps <= 2 * INK_RESOLUTION,
        "{steps} steps for one box width"
    );
    let topology =
        index_model(&[node("a", ""), node("b", "")], &[edge("e", "a", "b")]).expect("fits");
    let geometry = Geometry::planar(
        NodeGeometry::Point {
            x: vec![0.0, 1e-37],
            y: vec![0.0, 0.0],
        },
        EdgeGeometry::Line,
        Vec::new(),
    );
    assert_eq!(ink(&topology, &geometry).cells, 1, "one cell per axis");
}

#[test]
fn a_segment_entirely_outside_the_box_marks_nothing_and_is_still_measured() {
    // PB-8. The raster's box is the unit square, and the segment sits past its far corner.
    // Every cell index `cell` hands out is clamped into the grid, so the walk used to land
    // in the corner cell for the whole of the segment and report one cell of ink for a
    // stroke that crosses none of the box. It also spent 2.56e9 half-cell steps getting
    // there: the step of a one-wide box is 1/256, and nothing but the box bounded it.
    //
    // What the walk cost, spelled out on this input: `steps` is `max(|dx|, |dy|)` at half a
    // cell and the cell of a 1-wide box is 1/128, so three units of travel were
    // `3 * 2 * INK_RESOLUTION` half-cell steps of a loop that marked one and the same
    // clamped cell every time.
    let mut raster = Raster::over(&[0.0, 1.0, 0.5], &[0.0, 1.0, 0.25]);
    assert_eq!(raster.steps(3.0, 3.0), 3 * 2 * INK_RESOLUTION);
    let length = raster.mark(2.0, 2.0, 3.0, 3.0);
    assert_eq!(raster.occupied(), 0, "no cell of the box is crossed");
    // The length is the whole segment's, whether or not a cell of it is inside the box:
    // the two numbers describe the same stroke, and the cell count is the one that is
    // clipped.
    assert_eq!(length, f64::from(libm::hypotf(1.0, 1.0)));
}

#[test]
fn an_edge_point_far_outside_a_one_by_one_box_finishes_and_marks_a_bounded_raster() {
    // PB-8. The finding's own figure: a box of 1 with an edge point at 1e7 is
    // 1e7 / (0.5 / 128) = 2.56e9 half-cell steps per segment, twice over this edge — it ran
    // for over 60 seconds before the clip. Clipped to the box the walk is two diagonals of
    // it, at most `2 * INK_RESOLUTION` steps each, covering the same cells.
    let topology = index_model(
        &[node("a", ""), node("b", ""), node("c", "")],
        &[edge("e", "a", "b")],
    )
    .expect("fits");
    let geometry = Geometry::planar(
        NodeGeometry::Point {
            x: vec![0.0, 1.0, 0.5],
            y: vec![0.0, 1.0, 0.25],
        },
        EdgeGeometry::Polyline(Paths {
            offsets: vec![0, 1],
            pts: vec![1e7, 1e7],
        }),
        Vec::new(),
    );
    let measured = ink(&topology, &geometry);
    assert!(
        measured.cells <= 2 * INK_RESOLUTION,
        "{} cells for one diagonal of a 1x1 box",
        measured.cells
    );
    // 2 * hypot(1e7, 1e7) = 2.83e7, against the 2.83 the clipped diagonal would have
    // reported: the length is never the clipped one.
    assert!(
        measured.length > 2.8e7,
        "{} is not the whole segment's length",
        measured.length
    );
}

#[test]
fn a_segment_inside_the_box_marks_exactly_the_cells_the_unclipped_walk_marked() {
    // The clip must not move a cell that was already marked: this walks the same segment
    // both ways — once through `mark`, once through the whole-segment walk `mark` used
    // before it — and compares the marked cells index for index.
    let (ax, ay, bx, by) = (0.1, 0.2, 0.9, 0.8);
    let mut clipped = Raster::over(&[0.0, 1.0, 0.5], &[0.0, 1.0, 0.25]);
    clipped.mark(ax, ay, bx, by);
    let mut whole = Raster::over(&[0.0, 1.0, 0.5], &[0.0, 1.0, 0.25]);
    unclipped(&mut whole, ax, ay, bx, by);
    assert_eq!(marked(&clipped), marked(&whole), "cell for cell");
    assert!(!marked(&clipped).is_empty(), "the walk marked something");
}

/// The cell indices `raster` has marked, in index order, so two walks compare cell for cell.
fn marked(raster: &Raster) -> Vec<u32> {
    raster
        .cells
        .iter()
        .enumerate()
        .filter(|(_, on)| **on)
        .map(|(index, _)| index as u32)
        .collect()
}

/// The walk [`Raster::mark`] used before it clipped: every sample of the whole segment,
/// marked through the same `mark_point`, cells clamped exactly as [`cell`] clamps them.
fn unclipped(raster: &mut Raster, ax: f32, ay: f32, bx: f32, by: f32) {
    let steps = raster.steps(bx - ax, by - ay);
    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        raster.mark_point(ax + (bx - ax) * t, ay + (by - ay) * t);
    }
}
