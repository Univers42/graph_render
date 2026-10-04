//! The octree's own suite, next to `quadtree/tests.rs` and written as its twin: every test
//! has a row in that file, the same assertion with one more axis.
//!
//! The two additions are in [`tests::arena`] (`the_eight_children_tile_the_parent_in_slot_order`
//! and `a_planar_point_set_gives_the_octree_the_quadtrees_arena`) — the second is the
//! strongest statement this module can make, that where the two trees can be compared at all
//! they agree point for point, cell for cell and id for id.
//!
//! Unlike the quadtree's suite there is no capped-child probe for a non-finite coordinate:
//! this build carries the same three guards (`bounds_of` excludes non-finite points,
//! `insert` skips them, `cover` bails on a non-finite edge), so the `±inf` cases are
//! asserted inline in [`tests::tree`] rather than run in a second process. That is a stated
//! departure, not an omission — the probe exists to stop a *hang*, and a hang is what would
//! show up here as a failed test with a timeout.

mod arena;
mod forces;
mod tree;

use super::*;

pub(super) fn built(points: &[(f64, f64, f64)]) -> (Octree, Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut zs = Vec::new();
    for &(x, y, z) in points {
        xs.push(x);
        ys.push(y);
        zs.push(z);
    }
    let mut tree = Octree::default();
    tree.build(Points3 {
        xs: &xs,
        ys: &ys,
        zs: &zs,
    });
    (tree, xs, ys, zs)
}

pub(super) fn is_leaf(tree: &Octree, k: u32) -> bool {
    tree.cells()[k as usize].skip == k + 1
}

/// Every point some leaf holds, leaf by leaf in preorder.
pub(super) fn visited_points(tree: &Octree) -> Vec<u32> {
    let mut seen = Vec::new();
    for (k, cell) in tree.cells().iter().enumerate() {
        if is_leaf(tree, k as u32) {
            seen.extend(&tree.order()[cell.start as usize..cell.end as usize]);
        }
    }
    seen
}

pub(super) fn sorted(mut v: Vec<u32>) -> Vec<u32> {
    v.sort_unstable();
    v
}

/// The point sets whose root cube must contain every point, at magnitudes far enough apart
/// to exercise the growth loop.
pub(super) fn cover_cases() -> Vec<Vec<(f64, f64, f64)>> {
    vec![
        vec![(-5.5, 100.2, 0.25), (300.0, -20.0, 7.0), (0.0, 0.0, -3.0)],
        vec![(-1e17, 1e17, 1e17), (1e17, -1e17, -1e17), (0.0, 0.0, 0.0)],
        vec![(1e300, -1e300, 1e300), (-1e300, 1e300, -1e300)],
        vec![
            (0.0, 0.0, 0.0),
            (1e17, 1e17, 1e17),
            (1e300, 1e300, 1e300),
            (-1.0, -2.0, -3.0),
        ],
    ]
}
