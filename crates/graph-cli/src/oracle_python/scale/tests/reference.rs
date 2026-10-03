//! The reference arithmetic, re-run natively so a matrix change is caught here and not only
//! in the container. numpy is not a graph-cli dependency, so the one function the camera
//! test needs is written out longhand: it is `lod.py:35-47` with `w = 1`.

use crate::oracle_python::scale::{cases::square, ortho};

/// The camera is the rectangle: its corners land on the corners of the NDC box, and a
/// node's disc crosses the edge exactly when the motor's own rectangle test says it does.
/// Without this the cull comparison could pass on a matrix that culls everything or
/// nothing.
#[test]
fn the_camera_is_the_rectangle_and_its_radius_crosses_the_edge() {
    let view = square(0.0, 8.0, 0.5);
    let matrix = flat(&ortho(view));
    // The reference reads `radii` in clip space: `clip_radii`, not the world radius.
    let r = view.radius * 2.0 / (view.x1 - view.x0);
    let at = |x: f64, y: f64| ref_kept(&matrix, x, y, r);
    assert!(at(4.0, 4.0), "the middle is inside");
    assert!(at(7.75, 4.0), "a disc straddling the right edge is kept");
    assert!(!at(8.75, 4.0), "a disc past the right edge is culled");
    assert!(at(4.0, -0.25), "a disc straddling the left edge is kept");
    assert!(!at(4.0, -0.75), "a disc past the left edge is culled");
    assert!(at(4.0, 8.25), "a disc straddling the top edge is kept");
    assert!(!at(4.0, 8.75), "a disc past the top edge is culled");
    assert!(ref_kept(&matrix, 0.0, 0.0, 0.0) && ref_kept(&matrix, 8.0, 8.0, 0.0));
}

/// `ortho(view)` as a flat row-major array of `f64`.
fn flat(matrix: &[Vec<f64>]) -> Vec<f64> {
    matrix.iter().flat_map(|row| row.iter().copied()).collect()
}

/// `lod.frustum_cull_spheres` for one node against [`flat`]'s matrix, `z = 0`: each axis
/// keeps when `clip/aw ± r/aw` brackets the NDC box, and the near plane keeps the rest.
fn ref_kept(matrix: &[f64], x: f64, y: f64, radius: f64) -> bool {
    let clip = |row: usize, p: [f64; 3]| {
        matrix[row * 4] * p[0]
            + matrix[row * 4 + 1] * p[1]
            + matrix[row * 4 + 2] * p[2]
            + matrix[row * 4 + 3]
    };
    let point = [x, y, 0.0];
    let aw = 1.0 + 1e-9;
    let keep = (0..3).all(|axis| {
        let ndc = clip(axis, point) / aw;
        (ndc + radius / aw >= -1.0) && (ndc - radius / aw <= 1.0)
    });
    keep && clip(2, point) + radius >= -aw
}
