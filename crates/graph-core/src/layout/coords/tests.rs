use super::{point_geometry, rescale};
use graph_contract::geometry::NodeGeometry;

#[test]
fn rescale_centres_then_divides_by_the_largest_absolute_coordinate() {
    let (mut x, mut y) = (vec![0.0, 4.0], vec![1.0, 1.0]);
    rescale(&mut x, &mut y);
    assert_eq!((x, y), (vec![-1.0, 1.0], vec![0.0, 0.0]));
}

#[test]
fn rescale_leaves_a_collapsed_cloud_at_the_origin_and_an_empty_one_alone() {
    let (mut x, mut y) = (vec![3.0, 3.0], vec![-2.0, -2.0]);
    rescale(&mut x, &mut y);
    assert_eq!((x, y), (vec![0.0, 0.0], vec![0.0, 0.0]));
    rescale(&mut [], &mut []);
}

#[test]
fn point_geometry_narrows_to_f32_and_draws_lines() {
    let g = point_geometry(&[0.5], &[0.25]);
    assert_eq!(
        g.nodes,
        NodeGeometry::Point {
            x: vec![0.5],
            y: vec![0.25]
        }
    );
    assert!(g.notes.is_empty());
}
