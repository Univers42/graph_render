//! SCRATCH review harness - not part of the deliverable, deleted before return.

use super::quadtree::Quadtree;

#[test]
fn quadtree_hangs_on_a_single_point_past_2_pow_53() {
    let mut tree = Quadtree::default();
    println!("about to build a SINGLE point at x=1e17");
    tree.build(&[1e17], &[0.0]);
    println!("built, cells={}", tree.cells().len());
}

#[test]
fn quadtree_hangs_on_a_single_infinite_point() {
    let mut tree = Quadtree::default();
    println!("about to build a SINGLE point at x=inf");
    tree.build(&[f64::INFINITY], &[0.0]);
    println!("built, cells={}", tree.cells().len());
}