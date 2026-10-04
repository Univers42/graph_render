use super::super::*;
use super::*;

#[test]
fn an_empty_or_all_nan_point_set_builds_an_empty_tree() {
    let (empty, ..) = built(&[]);
    assert!(empty.root.is_none());
    assert_eq!(visited_points(&empty), Vec::<u32>::new());
    let (nan_only, ..) = built(&[
        (f64::NAN, 1.0, 2.0),
        (2.0, f64::NAN, 3.0),
        (1.0, 2.0, f64::NAN),
    ]);
    assert!(nan_only.root.is_none(), "every point had a NaN coordinate");
}

#[test]
fn a_single_point_is_the_root_leaf() {
    let (tree, ..) = built(&[(3.0, 4.0, 5.0)]);
    assert_eq!(visited_points(&tree), [0]);
    assert_eq!(tree.len(), 1);
}

#[test]
fn exactly_coincident_points_chain_on_one_leaf_others_split_apart() {
    let (tree, ..) = built(&[
        (1.0, 1.0, 1.0),
        (1.0, 1.0, 1.0),
        (9.0, 9.0, 9.0),
        (1.0, 1.0, 1.0),
    ]);
    assert_eq!(sorted(visited_points(&tree)), [0, 1, 2, 3]);
    // The three coincident points (0, 1, 3) share one leaf's chain.
    let chain_leaf = (0..tree.len()).find(|&k| {
        let cell = tree.cells()[k as usize];
        is_leaf(&tree, k) && cell.end - cell.start == 3
    });
    assert!(chain_leaf.is_some(), "no leaf holds the 3-point chain");
}

#[test]
fn a_nan_point_is_ignored_and_never_reached_by_visit() {
    let (tree, ..) = built(&[(0.0, 0.0, 0.0), (f64::NAN, 2.0, 1.0), (5.0, 5.0, 5.0)]);
    assert_eq!(sorted(visited_points(&tree)), [0, 2]);
    assert_eq!(
        tree.order(),
        [0, 2, 1],
        "the NaN point trails, so order is a permutation"
    );
}

/// Every non-finite point set the build must refuse and return on, each with the index of
/// its one finite point, or [`NONE_FINITE`] when it has none.
fn non_finite_cases() -> Vec<(&'static str, Points3<'static>, usize)> {
    vec![
        (
            "+inf x",
            Points3 {
                xs: &[f64::INFINITY, 0.0],
                ys: &[0.0, 1.0],
                zs: &[0.0, 2.0],
            },
            1,
        ),
        (
            "-inf z",
            Points3 {
                xs: &[0.0, 1.0],
                ys: &[1.0, 2.0],
                zs: &[2.0, f64::NEG_INFINITY],
            },
            0,
        ),
        (
            "all non-finite",
            Points3 {
                xs: &[f64::INFINITY],
                ys: &[f64::NEG_INFINITY],
                zs: &[f64::NAN],
            },
            NONE_FINITE,
        ),
    ]
}

/// `±inf` is the case the quadtree's review named: `bounds_of` filtered only NaN, so
/// `cover` grew until the bounds went NaN and `insert_leaf` split for ever.
const NONE_FINITE: usize = usize::MAX;

/// What one refused case must come back with: a finite cube (when a point is finite at all),
/// `order` still a permutation, the refused point in no leaf and trailing the leaves.
fn assert_refused(name: &str, pts: Points3<'_>, finite: usize) {
    let mut tree = Octree::default();
    tree.build(pts);
    assert_eq!(tree.order().len(), pts.len(), "{name}: a permutation");
    if finite == NONE_FINITE {
        assert!(tree.root.is_none(), "{name}: no point is finite");
        assert!(visited_points(&tree).is_empty(), "{name}: no leaf at all");
        return;
    }
    assert!(tree.root_bounds.x0.is_finite(), "{name}: finite bounds");
    assert_eq!(
        visited_points(&tree),
        [finite as u32],
        "{name}: one leaf, one point"
    );
    let refused = (0..pts.len() as u32)
        .find(|&i| i as usize != finite)
        .expect("one refused");
    assert_eq!(tree.order()[1], refused, "{name}: the refused point trails");
}

/// The `±inf` case the quadtree's capped-child probe guards, asserted inline here.
#[test]
fn an_infinite_coordinate_is_refused_and_the_build_returns() {
    for (name, pts, finite) in non_finite_cases() {
        assert_refused(name, pts, finite);
    }
}

#[test]
fn every_subtree_is_one_run_of_cells_and_one_run_of_points() {
    let pts: Vec<(f64, f64, f64)> = (0..40)
        .map(|i| (i as f64, (i * 7 % 11) as f64, (i * 13 % 5) as f64))
        .collect();
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
    let pts: Vec<(f64, f64, f64)> = (0..25)
        .map(|i| ((i * 3) as f64, (i * i % 13) as f64, (i % 7) as f64))
        .collect();
    let (a, xs, ys, zs) = built(&pts);
    let mut b = Octree::default();
    b.build(Points3 {
        xs: &xs,
        ys: &ys,
        zs: &zs,
    });
    b.build(Points3 {
        xs: &xs,
        ys: &ys,
        zs: &zs,
    });
    let layout = |t: &Octree| {
        let cells: Vec<_> = t.cells().iter().map(|c| (c.skip, c.start, c.end)).collect();
        let keys: Vec<_> = (0..t.len()).map(|k| t.node_id(k)).collect();
        (cells, keys, t.order().to_vec())
    };
    assert_eq!(
        layout(&a),
        layout(&b),
        "same input, same arena, every rebuild"
    );
}

#[test]
fn cover_grows_a_cube_that_contains_every_point() {
    for points in cover_cases() {
        let (tree, ..) = built(&points);
        let b = tree.root_bounds;
        let edge = b.x1 - b.x0;
        assert_eq!(
            (edge, b.y1 - b.y0, b.z1 - b.z0),
            (edge, edge, edge),
            "a cube for {points:?}"
        );
        assert!(
            b.x0.is_finite() && b.x1.is_finite(),
            "finite for {points:?}"
        );
        for &(x, y, z) in &points {
            assert!(
                b.x0 <= x && x < b.x1 && b.y0 <= y && y < b.y1 && b.z0 <= z && z < b.z1,
                "({x}, {y}, {z}) in {b:?}"
            );
        }
        assert_eq!(
            sorted(visited_points(&tree)),
            (0..points.len() as u32).collect::<Vec<_>>()
        );
    }
}

#[test]
fn build_refuses_a_length_mismatch_instead_of_indexing_past_the_columns() {
    let mut tree = Octree::default();
    tree.build(Points3 {
        xs: &[0.0, 1.0],
        ys: &[0.0],
        zs: &[0.0, 1.0],
    });
    assert!(tree.root.is_none(), "a refused build leaves no root");
    assert!(tree.order().is_empty(), "and no leaf");
    let (good, xs, ys, zs) = built(&[(0.0, 0.0, 0.0), (9.0, 9.0, 9.0)]);
    assert!(good.len() > 0);
    tree.build(Points3 {
        xs: &xs[..1],
        ys: &ys,
        zs: &zs,
    });
    assert_eq!(tree.cells().len(), 0, "the old arena is cleared");
}
