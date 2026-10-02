use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};
use graph_contract::geometry::EdgeGeometry;

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
