//! The wire order of the node columns, and each column's own rule.
//!
//! Split out of `geometry/tests.rs` by the house's 300-line limit, and because the z column
//! is the one thing here with a rule of its own: it sits after `y`, it is a coordinate and
//! not a size, and a wrong length in it is refused under its own name.

use crate::geometry::NodeGeometry;
use crate::snapshot::SnapshotError;

fn point(n: usize) -> NodeGeometry {
    NodeGeometry::Point {
        x: vec![0.5; n],
        y: vec![-0.5; n],
    }
}

/// The z column is spliced in right after `y`, so `x, y, z` stays contiguous and a size
/// shifts one word along. A z is a coordinate, so it may be negative and NaN is refused
/// like any other column.
#[test]
fn a_z_column_sits_after_y_and_is_checked_like_any_other_column() {
    let names = |g: &NodeGeometry, z: Option<&[f32]>| {
        g.columns_dim(z).iter().map(|c| c.0).collect::<Vec<_>>()
    };
    let z = [0.0, 1.0];
    let circle = NodeGeometry::Circle {
        x: vec![0.0; 2],
        y: vec![0.0; 2],
        r: vec![1.0; 2],
    };
    let boxes = NodeGeometry::Box {
        x: vec![0.0; 2],
        y: vec![0.0; 2],
        w: vec![1.0; 2],
        h: vec![1.0; 2],
    };
    assert_eq!(names(&circle, Some(&z)), ["x", "y", "z", "r"]);
    assert_eq!(names(&boxes, Some(&z)), ["x", "y", "z", "w", "h"]);
    assert_eq!(names(&point(2), Some(&z)), ["x", "y", "z"]);
    assert_eq!(names(&circle, None), ["x", "y", "r"], "2D is the old order");

    assert_eq!(circle.check(2, Some(&z)), Ok(()));
    assert_eq!(
        circle.check(2, Some(&[-1.0, -2.0])),
        Ok(()),
        "a negative z is a coordinate, not a size"
    );
    let length = |expected, found| SnapshotError::Length {
        column: "node.z",
        expected,
        found,
    };
    assert_eq!(circle.check(2, Some(&[0.0])), Err(length(2, 1)));
    assert_eq!(
        circle.check(2, Some(&[0.0, 1.0, 2.0])),
        Err(length(2, 3)),
        "and a z of the wrong length is refused on its own account"
    );
    assert_eq!(
        circle.check(2, Some(&[0.0, f32::INFINITY])),
        Err(SnapshotError::NonFinite {
            column: "node.z",
            index: 1
        }),
        "and the column is named as its own, not as the x or y it sits between"
    );
}

/// The z column shifts a size along, so a reader that assumed a fixed position for `r`
/// would read a z as a radius. This is the assertion that says the order is computed, not
/// assumed: the Circle's `r` is not at the byte offset a 2D snapshot puts it at.
#[test]
fn the_sizes_move_along_when_a_z_column_is_present() {
    let circle = NodeGeometry::Circle {
        x: vec![1.0],
        y: vec![2.0],
        r: vec![3.0],
    };
    let flat: Vec<f32> = circle
        .columns_dim(None)
        .into_iter()
        .flat_map(|(_, c)| c.iter().copied())
        .collect();
    let deep: Vec<f32> = circle
        .columns_dim(Some(&[9.0]))
        .into_iter()
        .flat_map(|(_, c)| c.iter().copied())
        .collect();
    assert_eq!(flat, [1.0, 2.0, 3.0]);
    assert_eq!(deep, [1.0, 2.0, 9.0, 3.0], "z is third, r is fourth");
    assert_eq!(
        deep[3], flat[2],
        "r keeps its value, only its position moves"
    );
}
