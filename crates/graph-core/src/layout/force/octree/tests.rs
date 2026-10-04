//! The octree's own suite, next to `quadtree/tests.rs` and written as its twin: every
//! test below has a row in that file, the same assertion with one more axis. The two
//! additions are `the_eight_children_tile_the_parent_in_slot_order` and
//! `a_planar_point_set_gives_the_octree_the_quadtrees_arena` — the second is the strongest
//! statement the module can make, that where the two trees can be compared at all they
//! agree point for point, cell for cell and id for id.
//!
//! Unlike the quadtree's suite there is no capped-child probe for a non-finite coordinate:
//! this build carries the same three guards (`bounds_of` excludes non-finite points,
//! `insert` skips them, `cover` bails on a non-finite edge), so the `±inf` cases are
//! asserted inline below rather than run in a second process. That is a stated departure,
//! not an omission — the probe exists to stop a *hang*, and a hang is what would show up
//! here as a failed test with a timeout.

mod forces;

use super::*;
use crate::layout::force::quadtree::Quadtree;

fn built(points: &[(f64, f64, f64)]) -> (Octree, Vec<f64>, Vec<f64>, Vec<f64>) {
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

fn is_leaf(tree: &Octree, k: u32) -> bool {
    tree.cells()[k as usize].skip == k + 1
}

/// Every point some leaf holds, leaf by leaf in preorder.
fn visited_points(tree: &Octree) -> Vec<u32> {
    let mut seen = Vec::new();
    for (k, cell) in tree.cells().iter().enumerate() {
        if is_leaf(tree, k as u32) {
            seen.extend(&tree.order()[cell.start as usize..cell.end as usize]);
        }
    }
    seen
}

fn sorted(mut v: Vec<u32>) -> Vec<u32> {
    v.sort_unstable();
    v
}

#[test]
fn an_empty_or_all_nan_point_set_builds_an_empty_tree() {
    let (empty, ..) = built(&[]);
    assert!(empty.root.is_none());
    assert_eq!(visited_points(&empty), Vec::<u32>::new());
    let (nan_only, ..) = built(&[(f64::NAN, 1.0, 2.0), (2.0, f64::NAN, 3.0), (1.0, 2.0, f64::NAN)]);
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

/// The `±inf` case the quadtree's capped-child probe guards, asserted inline here. Each
/// case names which of its two points is finite, so the expectation is stated and not
/// inferred from the run.
#[test]
fn an_infinite_coordinate_is_refused_and_the_build_returns() {
    let cases: Vec<(&str, Points3<'_>, usize)> = vec![
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
            usize::MAX,
        ),
    ];
    for (name, pts, finite) in cases {
        let mut tree = Octree::default();
        tree.build(pts);
        assert_eq!(tree.order().len(), pts.len(), "{name}: order is a permutation");
        if finite == usize::MAX {
            assert!(tree.root.is_none(), "{name}: no point is finite");
            assert!(visited_points(&tree).is_empty(), "{name}: no leaf at all");
            continue;
        }
        assert!(tree.root_bounds.x0.is_finite(), "{name}: finite bounds");
        assert_eq!(visited_points(&tree), [finite as u32], "{name}: one leaf, one point");
        let refused = (0..pts.len() as u32).find(|&i| i as usize != finite).expect("one refused");
        assert_eq!(tree.order()[1], refused, "{name}: the refused point trails");
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
            assert_eq!(child.start, at, "child {c} of {k} starts where its sibling ended");
            assert!(child.skip <= cell.skip, "child {c} of {k} nests inside it");
            (c, at) = (child.skip, child.end);
        }
        assert_eq!((c, at), (cell.skip, cell.end), "the children cover cell {k}");
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
    assert_eq!(layout(&a), layout(&b), "same input, same arena, every rebuild");
}

/// The point sets whose root cube must contain every point, at magnitudes far enough apart
/// to exercise the growth loop.
fn cover_cases() -> Vec<Vec<(f64, f64, f64)>> {
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

#[test]
fn cover_grows_a_cube_that_contains_every_point() {
    for points in cover_cases() {
        let (tree, ..) = built(&points);
        let b = tree.root_bounds;
        let edge = b.x1 - b.x0;
        assert_eq!((edge, b.y1 - b.y0, b.z1 - b.z0), (edge, edge, edge), "a cube for {points:?}");
        assert!(b.x0.is_finite() && b.x1.is_finite(), "finite for {points:?}");
        for &(x, y, z) in &points {
            assert!(b.x0 <= x && x < b.x1 && b.y0 <= y && y < b.y1 && b.z0 <= z && z < b.z1,
                "({x}, {y}, {z}) in {b:?}");
        }
        assert_eq!(sorted(visited_points(&tree)), (0..points.len() as u32).collect::<Vec<_>>());
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

#[test]
fn a_cell_s_node_id_is_a_shape_slot_and_never_a_point_index() {
    let (tree, ..) = built(&[
        (0.0, 0.0, 0.0),
        (1.0, 1.0, 1.0),
        (2.0, 2.0, 2.0),
        (3.0, 3.0, 3.0),
    ]);
    let ids: Vec<u32> = (0..tree.len()).map(|k| tree.node_id(k)).collect();
    assert_eq!(ids.len(), tree.cells().len(), "one per cell");
    assert!(
        ids.iter().all(|&id| (id as usize) < tree.shape.len()),
        "every node id is a shape slot: {ids:?} of {} slots",
        tree.shape.len()
    );
    let points = tree.order().len() as u32;
    assert!(
        ids.iter().any(|&id| id >= points),
        "a node id is not a point index: {ids:?} against {points} points"
    );
    let heads: Vec<u32> = ids
        .iter()
        .filter(|&&id| matches!(tree.shape[id as usize], Shape::Leaf(_)))
        .copied()
        .collect();
    assert_eq!(heads.len(), 4, "four leaves");
    for id in heads {
        let Shape::Leaf(head) = tree.shape[id as usize] else {
            unreachable!()
        };
        assert!(tree.order().contains(&head), "leaf slot {id} holds point {head}");
    }
}

#[test]
fn every_internal_cell_has_a_child_so_the_centre_never_divides_by_zero() {
    for points in cover_cases() {
        let (tree, ..) = built(&points);
        for k in 0..tree.len() {
            let cell = tree.cells()[k as usize];
            assert!(cell.end > cell.start, "cell {k} holds a point");
            if is_leaf(&tree, k) {
                continue;
            }
            assert!(cell.skip > k + 1, "cell {k} has at least one child");
        }
    }
}

#[test]
fn a_cell_s_bounds_are_its_parent_s_octant() {
    let (tree, ..) = built(&[(0.0, 0.0, 0.0), (50.0, 50.0, 50.0)]);
    let cells = tree.cells();
    assert_eq!(cells[0].bounds, tree.root_bounds);
    assert!(!is_leaf(&tree, 0) && is_leaf(&tree, 1));
    assert_eq!(cells[1].bounds, tree.root_bounds.octant(0), "(0,0,0) is slot 0");
    assert_eq!(cells[0].skip, tree.len(), "the root's run is the whole arena");
}

/// The octant's own claim: eight well-separated points occupy all eight slots, in slot
/// order, and each child's bounds are its octant.
#[test]
fn the_eight_children_tile_the_parent_in_slot_order() {
    let pts: Vec<(f64, f64, f64)> = (0..8)
        .map(|i| ((i & 1) as f64, ((i >> 1) & 1) as f64, ((i >> 2) & 1) as f64))
        .collect();
    let (tree, ..) = built(&pts);
    let cells = tree.cells();
    assert_eq!(tree.len(), 9, "one internal cell and eight leaves");
    let root = &cells[0];
    assert!(!is_leaf(&tree, 0), "the cube splits once and no further");
    for slot in 0..OCTANTS {
        let child = &cells[slot + 1];
        assert!(is_leaf(&tree, slot as u32 + 1), "slot {slot} is a leaf");
        assert_eq!(child.bounds, root.bounds.octant(slot), "slot {slot}'s octant");
        assert_eq!(child.end - child.start, 1, "slot {slot} holds one point");
    }
}

/// A point exactly on an axis' midpoint takes the upper half, `>=`, as the quadtree does.
#[test]
fn a_point_on_the_midplane_takes_the_upper_half_on_every_axis() {
    let (tree, ..) = built(&[(0.0, 0.0, 0.0), (0.5, 0.5, 0.5)]);
    let root = tree.root_bounds;
    let mut one = root;
    assert_eq!(one.narrow(0.5, 0.5, 0.5), OCTANTS - 1, "all three bits set");
    let mut zero = root;
    assert_eq!(zero.narrow(0.0, 0.0, 0.0), 0, "no bit set");
    assert_eq!(tree.cells()[1].bounds, root.octant(0), "(0,0,0) landed in slot 0");
    assert_eq!(tree.cells()[2].bounds, root.octant(OCTANTS - 1), "(0.5)^3 landed in slot 7");
}

/// Where the two trees can be compared at all, they agree exactly.
///
/// A planar point set never splits on `z`: every point is at or above each midpoint, so
/// the `z` interval only ever narrows upward while `x` and `y` halve exactly as they do in
/// the quadtree. Every internal cell therefore has one child and the two arenas must be
/// identical — cell for cell, id for id, point for point. This is the test that would fail
/// first if the octree's build order, its coincidence chaining or its preorder flatten ever
/// drifted from the quadtree's.
#[test]
fn a_planar_point_set_gives_the_octree_the_quadtrees_arena() {
    let pts: Vec<(f64, f64)> = (0..30).map(|i| ((i * 5) as f64, (i * i % 17) as f64)).collect();
    let (xs, ys): (Vec<f64>, Vec<f64>) = pts.iter().map(|p| (p.0, p.1)).unzip();
    let zs = vec![7.0; xs.len()];
    let mut oct = Octree::default();
    oct.build(Points3 {
        xs: &xs,
        ys: &ys,
        zs: &zs,
    });
    let mut quad = Quadtree::default();
    quad.build(&xs, &ys);
    let runs = |c: Vec<(u32, u32, u32)>| c;
    let oct_runs = runs(oct.cells().iter().map(|c| (c.skip, c.start, c.end)).collect());
    let quad_runs = runs(quad.cells().iter().map(|c| (c.skip, c.start, c.end)).collect());
    assert_eq!(oct_runs, quad_runs, "the same cells, in the same order");
    let oct_ids: Vec<u32> = (0..oct.len()).map(|k| oct.node_id(k)).collect();
    let quad_ids: Vec<u32> = (0..quad.len()).map(|k| quad.node_id(k)).collect();
    assert_eq!(oct_ids, quad_ids, "the same shape slots in the same order");
    assert_eq!(oct.order(), quad.order(), "the same point order");
}

/// Two points one `f64` step apart in `z` and equal in `x` and `y` split for 33 levels
/// before the `z` span stops shrinking, so `insert_leaf` bails after pushing internal
/// nodes.
#[test]
fn a_bail_after_a_split_keeps_both_points_in_one_leaf() {
    let (tree, ..) = built(&[(1e6, 1e6, 0.0), (1e6, 1e6, 1e-300)]);
    assert_eq!(sorted(visited_points(&tree)), [0, 1], "every point is in a leaf");
    for (k, cell) in tree.cells().iter().enumerate() {
        assert!(cell.end > cell.start, "cell {k} holds a point");
    }
}

/// A rebuild at steady state allocates nothing: every buffer is cleared and refilled.
#[test]
fn a_rebuild_allocates_nothing_once_the_buffers_have_reached_capacity() {
    let pts: Vec<(f64, f64, f64)> = (0..40).map(|i| (i as f64, (i * 3) as f64, (i % 9) as f64)).collect();
    let (mut tree, xs, ys, zs) = built(&pts);
    let pts = Points3 {
        xs: &xs,
        ys: &ys,
        zs: &zs,
    };
    tree.build(pts);
    let before = tree.capacity();
    tree.build(pts);
    assert_eq!(tree.capacity(), before, "no buffer grew");
}