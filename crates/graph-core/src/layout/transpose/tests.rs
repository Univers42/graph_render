use super::*;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry, Paths};

/// One column, as the bits the wire carries: two `f32` that compare equal can still be
/// different numbers (`-0.0` and `0.0`, or a NaN's many spellings), and a transpose is
/// exact only if it moves the value itself.
fn bits(column: &[f32]) -> Vec<u32> {
    column.iter().map(|value| value.to_bits()).collect()
}

#[test]
fn a_transpose_swaps_centres_sizes_and_every_path_point() {
    let geometry = Geometry::planar(
        NodeGeometry::Box {
            x: vec![1.0, 2.0],
            y: vec![3.0, 4.0],
            w: vec![5.0, 6.0],
            h: vec![7.0, 8.0],
        },
        EdgeGeometry::Polyline(Paths {
            offsets: vec![0, 1, 3],
            pts: vec![10.0, 11.0, 12.0, 13.0, 14.0, 15.0],
        }),
        Vec::new(),
    );
    let turned = geometry.transposed();
    let NodeGeometry::Box { x, y, w, h } = turned.nodes else {
        panic!("a box stays a box");
    };
    assert_eq!(bits(&x), bits(&[3.0, 4.0]), "x takes y");
    assert_eq!(bits(&y), bits(&[1.0, 2.0]), "y takes x");
    assert_eq!(bits(&w), bits(&[7.0, 8.0]), "width takes height");
    assert_eq!(bits(&h), bits(&[5.0, 6.0]), "height takes width");
    let EdgeGeometry::Polyline(paths) = turned.edges else {
        panic!("a polyline stays a polyline");
    };
    assert_eq!(paths.offsets, [0, 1, 3], "offsets are the CSR shape, not coordinates");
    assert_eq!(bits(&paths.pts), bits(&[11.0, 10.0, 13.0, 12.0, 15.0, 14.0]));
}

#[test]
fn two_transposes_give_back_the_same_geometry() {
    let geometry = Geometry::planar(
        NodeGeometry::Circle {
            x: vec![1.0, -2.0],
            y: vec![3.0, 4.0],
            r: vec![0.5, 0.25],
        },
        EdgeGeometry::Curve {
            degree: 2,
            paths: Paths {
                offsets: vec![0, 2],
                pts: vec![1.0, 2.0, 3.0, 4.0],
            },
        },
        Vec::new(),
    );
    assert_eq!(geometry.clone().transposed().transposed(), geometry);
}
