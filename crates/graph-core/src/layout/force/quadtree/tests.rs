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

fn is_leaf(tree: &Quadtree, k: u32) -> bool {
    tree.cells()[k as usize].skip == k + 1
}

/// Every point some leaf holds, leaf by leaf in preorder.
fn visited_points(tree: &Quadtree) -> Vec<u32> {
    let mut seen = Vec::new();
    for (k, cell) in tree.cells().iter().enumerate() {
        if is_leaf(tree, k as u32) {
            seen.extend(&tree.order()[cell.start as usize..cell.end as usize]);
        }
    }
    seen
}

#[test]
fn an_empty_or_all_nan_point_set_builds_an_empty_tree() {
    let (empty, ..) = built(&[]);
    assert!(empty.root.is_none());
    assert_eq!(visited_points(&empty), Vec::<u32>::new());
    let (nan_only, ..) = built(&[(f64::NAN, 1.0), (2.0, f64::NAN)]);
    assert!(nan_only.root.is_none(), "every point had a NaN coordinate");
}

#[test]
fn a_single_point_is_the_root_leaf() {
    let (tree, ..) = built(&[(3.0, 4.0)]);
    assert_eq!(visited_points(&tree), [0]);
    assert_eq!(tree.len(), 1);
}

#[test]
fn exactly_coincident_points_chain_on_one_leaf_others_split_apart() {
    let (tree, ..) = built(&[(1.0, 1.0), (1.0, 1.0), (9.0, 9.0), (1.0, 1.0)]);
    let mut seen = visited_points(&tree);
    seen.sort_unstable();
    assert_eq!(seen, [0, 1, 2, 3], "every point reachable exactly once");
    // The three coincident points (0, 1, 3) share one leaf's chain.
    let chain_leaf = (0..tree.len()).find(|&k| {
        let cell = tree.cells()[k as usize];
        is_leaf(&tree, k) && cell.end - cell.start == 3
    });
    assert!(chain_leaf.is_some(), "no leaf holds the 3-point chain");
}

#[test]
fn a_nan_point_is_ignored_and_never_reached_by_visit() {
    let (tree, ..) = built(&[(0.0, 0.0), (f64::NAN, 2.0), (5.0, 5.0)]);
    let mut seen = visited_points(&tree);
    seen.sort_unstable();
    assert_eq!(seen, [0, 2]);
    assert_eq!(
        tree.order(),
        [0, 2, 1],
        "the NaN point trails, so order is a permutation"
    );
}

#[test]
fn every_subtree_is_one_run_of_cells_and_one_run_of_points() {
    let pts: Vec<(f64, f64)> = (0..40).map(|i| (i as f64, (i * 7 % 11) as f64)).collect();
    let (tree, ..) = built(&pts);
    let cells = tree.cells();
    assert_eq!(tree.order().len(), 40, "every point listed once");
    for (k, cell) in cells.iter().enumerate() {
        let k = k as u32;
        assert!(cell.skip > k && cell.skip as usize <= cells.len());
        assert!(cell.start < cell.end, "every cell holds a point");
        if is_leaf(&tree, k) {
            continue;
        }
        // The children tile the parent's cells and points, in slot order.
        let (mut c, mut at) = (k + 1, cell.start);
        while c < cell.skip {
            let child = cells[c as usize];
            assert_eq!(
                child.start, at,
                "child {c} of {k} starts where its sibling ended"
            );
            assert!(child.skip <= cell.skip, "child {c} of {k} nests inside it");
            (c, at) = (child.skip, child.end);
        }
        assert_eq!(
            (c, at),
            (cell.skip, cell.end),
            "the children cover cell {k}"
        );
    }
}

#[test]
fn building_the_same_points_twice_gives_the_same_structure_run_to_run() {
    let pts: Vec<(f64, f64)> = (0..25)
        .map(|i| ((i * 3) as f64, (i * i % 13) as f64))
        .collect();
    let (a, xs, ys) = built(&pts);
    let mut b = Quadtree::default();
    b.build(&xs, &ys);
    b.build(&xs, &ys);
    let layout = |t: &Quadtree| {
        let cells: Vec<_> = t.cells().iter().map(|c| (c.skip, c.start, c.end)).collect();
        let keys: Vec<_> = (0..t.len()).map(|k| t.key(k)).collect();
        (cells, keys, t.order().to_vec())
    };
    assert_eq!(
        layout(&a),
        layout(&b),
        "same input, same arena, every rebuild"
    );
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
fn a_cell_s_bounds_are_its_parent_s_quadrant() {
    let (tree, ..) = built(&[(0.0, 0.0), (50.0, 50.0)]);
    let cells = tree.cells();
    assert_eq!(cells[0].bounds, tree.root_bounds);
    assert!(!is_leaf(&tree, 0) && is_leaf(&tree, 1));
    assert_eq!(
        cells[1].bounds,
        tree.root_bounds.quadrant(0),
        "(0, 0) is slot 0"
    );
    assert_eq!(
        cells[0].skip,
        tree.len(),
        "the root's run is the whole arena"
    );
}
