//! Split from `quadtree.rs` for the house line cap (`hierarchy.rs`/`hierarchy/tests.rs`
//! is the existing precedent for this split; reported as a deviation from the branch's
//! file list, which named `quadtree.rs` alone).

use super::*;

fn built(points: &[(f64, f64)]) -> (Quadtree, Vec<f64>, Vec<f64>) {
    let xs: Vec<f64> = points.iter().map(|p| p.0).collect();
    let ys: Vec<f64> = points.iter().map(|p| p.1).collect();
    let mut tree = Quadtree::default();
    tree.build(&xs, &ys);
    (tree, xs, ys)
}

/// Collects every point reached by an unpruned `visit`, in whatever order it arrives.
fn visited_points(tree: &mut Quadtree) -> Vec<u32> {
    let mut seen = Vec::new();
    tree.visit(|t, node, _| {
        if t.children(node).is_none() {
            seen.extend(t.leaf_points(node));
        }
        false
    });
    seen
}

#[test]
fn an_empty_or_all_nan_point_set_builds_an_empty_tree() {
    let (mut empty, ..) = built(&[]);
    assert!(empty.root.is_none());
    assert_eq!(visited_points(&mut empty), Vec::<u32>::new());
    let (nan_only, ..) = built(&[(f64::NAN, 1.0), (2.0, f64::NAN)]);
    assert!(nan_only.root.is_none(), "every point had a NaN coordinate");
}

#[test]
fn a_single_point_is_the_root_leaf() {
    let (mut tree, ..) = built(&[(3.0, 4.0)]);
    assert_eq!(visited_points(&mut tree), [0]);
    assert_eq!(tree.len(), 1);
}

#[test]
fn exactly_coincident_points_chain_on_one_leaf_others_split_apart() {
    let (mut tree, ..) = built(&[(1.0, 1.0), (1.0, 1.0), (9.0, 9.0), (1.0, 1.0)]);
    let mut seen = visited_points(&mut tree);
    seen.sort_unstable();
    assert_eq!(seen, [0, 1, 2, 3], "every point reachable exactly once");
    // The three coincident points (0, 1, 3) share one leaf's chain.
    let chain_leaf =
        (0..tree.len()).find(|&n| tree.children(n).is_none() && tree.leaf_points(n).count() == 3);
    assert!(chain_leaf.is_some(), "no leaf holds the 3-point chain");
}

#[test]
fn a_nan_point_is_ignored_and_never_reached_by_visit() {
    let (mut tree, ..) = built(&[(0.0, 0.0), (f64::NAN, 2.0), (5.0, 5.0)]);
    let mut seen = visited_points(&mut tree);
    seen.sort_unstable();
    assert_eq!(seen, [0, 2]);
}

#[test]
fn postorder_lists_every_child_before_its_parent() {
    let pts: Vec<(f64, f64)> = (0..40).map(|i| (i as f64, (i * 7 % 11) as f64)).collect();
    let (mut tree, ..) = built(&pts);
    let mut order = Vec::new();
    tree.postorder_into(&mut order);
    assert_eq!(
        order.len(),
        tree.len() as usize,
        "every arena node listed once"
    );
    let position: Vec<u32> = {
        let mut p = vec![0u32; order.len()];
        for (rank, &node) in order.iter().enumerate() {
            p[node as usize] = rank as u32;
        }
        p
    };
    for &node in &order {
        if let Some(children) = tree.children(node) {
            for child in children.into_iter().flatten() {
                assert!(
                    position[child as usize] < position[node as usize],
                    "child after parent"
                );
            }
        }
    }
}

#[test]
fn building_the_same_points_twice_gives_the_same_structure_run_to_run() {
    let pts: Vec<(f64, f64)> = (0..25)
        .map(|i| ((i * 3) as f64, (i * i % 13) as f64))
        .collect();
    let (mut a, xs, ys) = built(&pts);
    let mut b = Quadtree::default();
    b.build(&xs, &ys);
    let mut oa = Vec::new();
    let mut ob = Vec::new();
    a.postorder_into(&mut oa);
    b.postorder_into(&mut ob);
    assert_eq!(oa, ob, "same input, same arena layout, both runs");
    assert_eq!(a.len(), b.len());
}

#[test]
fn cover_grows_a_square_that_contains_every_point() {
    let (tree, ..) = built(&[(-5.5, 100.2), (300.0, -20.0), (0.0, 0.0)]);
    let b = tree.root_bounds;
    assert!(b.x1 - b.x0 == b.y1 - b.y0, "a square");
    for &(x, y) in &[(-5.5, 100.2), (300.0, -20.0), (0.0, 0.0)] {
        assert!(b.x0 <= x && x < b.x1, "x {x} in [{}, {})", b.x0, b.x1);
        assert!(b.y0 <= y && y < b.y1, "y {y} in [{}, {})", b.y0, b.y1);
    }
}

#[test]
fn visit_can_prune_a_whole_quadrant() {
    let (mut tree, ..) = built(&[(0.0, 0.0), (50.0, 50.0)]);
    let mut visits = 0u32;
    tree.visit(|_, _, _| {
        visits += 1;
        true // prune everything past the root
    });
    assert_eq!(visits, 1, "only the root was visited");
}
