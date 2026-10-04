use super::super::*;
use super::*;
use crate::layout::force::quadtree::Quadtree;

/// The arena's own claims: the slot index is three bits wide and used end to end, every
/// internal cell holds a child, a cell's bounds are its parent's octant, and — the row the
/// whole module rests on — a planar point set gives the octree the quadtree's arena exactly.

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
        assert!(
            tree.order().contains(&head),
            "leaf slot {id} holds point {head}"
        );
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
    assert_eq!(
        cells[1].bounds,
        tree.root_bounds.octant(0),
        "(0,0,0) is slot 0"
    );
    assert_eq!(
        cells[0].skip,
        tree.len(),
        "the root's run is the whole arena"
    );
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
        assert_eq!(
            child.bounds,
            root.bounds.octant(slot),
            "slot {slot}'s octant"
        );
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
    assert_eq!(
        tree.cells()[1].bounds,
        root.octant(0),
        "(0,0,0) landed in slot 0"
    );
    assert_eq!(
        tree.cells()[2].bounds,
        root.octant(OCTANTS - 1),
        "(0.5)^3 landed in slot 7"
    );
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
    let pts: Vec<(f64, f64)> = (0..30)
        .map(|i| ((i * 5) as f64, (i * i % 17) as f64))
        .collect();
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
    let oct_runs = runs(
        oct.cells()
            .iter()
            .map(|c| (c.skip, c.start, c.end))
            .collect(),
    );
    let quad_runs = runs(
        quad.cells()
            .iter()
            .map(|c| (c.skip, c.start, c.end))
            .collect(),
    );
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
    assert_eq!(
        sorted(visited_points(&tree)),
        [0, 1],
        "every point is in a leaf"
    );
    for (k, cell) in tree.cells().iter().enumerate() {
        assert!(cell.end > cell.start, "cell {k} holds a point");
    }
}

/// A rebuild at steady state allocates nothing: every buffer is cleared and refilled.
#[test]
fn a_rebuild_allocates_nothing_once_the_buffers_have_reached_capacity() {
    let pts: Vec<(f64, f64, f64)> = (0..40)
        .map(|i| (i as f64, (i * 3) as f64, (i % 9) as f64))
        .collect();
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
