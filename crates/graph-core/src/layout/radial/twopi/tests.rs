//! The analytically determined cases, read off Graphviz 16.1.0's `twopi` itself
//! (`twopi -Tplain`) and re-derived by hand from `lib/twopigen/circle.c`.
//!
//! Every expectation here is the **closed answer**, not a number copied out of the oracle:
//! the centre is the node furthest from any leaf, ring `r` sits at `r` inches, and a
//! node's angle is its share of `2*PI` in proportion to the leaves below it. The oracle's
//! plain output is quoted beside each case only as the independent check that the closed
//! answer and Graphviz agree — the oracle cannot be the oracle.

use super::run;
use crate::layout::coords::probe::{assert_close, graph, points};

/// 72 points per inch (`POINTS_PER_INCH`), so ring `r` has radius `72 * r`.
const PT: f32 = 72.0;

/// `sqrt(2) / 2 * 72`: the star's leaf offset.
const HALF_DIAGONAL: f32 = 50.911_69;

#[test]
fn an_empty_graph_has_no_points_and_one_node_is_the_centre() {
    assert_eq!(points(&run(&graph(0, &[])).unwrap()), vec![]);
    assert_eq!(points(&run(&graph(1, &[])).unwrap()), vec![(0.0, 0.0)]);
}

/// One node: Graphviz returns before any of the six phases, leaving the origin
/// (`circle.c:314-319`). `twopi -Tplain` on `graph g { n0; }` prints `node n0 0.375 0.25`,
/// which is the origin once the drawing is translated onto its bounding box.
#[test]
fn a_single_node_sits_at_the_origin() {
    assert_eq!(points(&run(&graph(1, &[])).unwrap()), [(0.0, 0.0)]);
}

/// A 3-path: both ends are leaves at one step, so `n1` is the node furthest from any
/// leaf and the centre. Its two subtrees hold one leaf each, so each takes half of `2*PI`
/// and its leaf sits on a radius of one inch.
///
/// Graphviz: `n0 0.375 0.25`, `n1 0.375 1.25`, `n2 0.375 2.25` — the middle node between
/// two ends a full inch apart on the vertical, `n0` *below* `n1`, which is the sibling
/// order of `n1`'s out-edge (`n1 -- n2`) before its in-edge (`n0 -- n1`).
#[test]
fn a_three_node_path_hangs_its_ends_a_rank_apart() {
    let want = [(0.0, -PT), (0.0, 0.0), (0.0, PT)];
    assert_close(
        &points(&run(&graph(3, &[(0, 1), (1, 2)])).unwrap()),
        &want,
        1e-3,
    );
}

/// A 4-cycle has no leaf at all, so every `nStepsToLeaf` stays at its `n^2` sentinel and
/// the centre is the first node, `n0` (`findCenterNode`, `circle.c:107-113`: strictly
/// greater keeps the first maximum). `n1` and `n3` are one step out and `n2` is two, so
/// `n2` lands on the outer ring above `n1` and the whole drawing is a vertical line.
#[test]
fn a_four_cycle_has_no_leaf_and_takes_the_first_node_as_its_centre() {
    let got = points(&run(&graph(4, &[(0, 1), (1, 2), (2, 3), (3, 0)])).unwrap());
    let want = [(0.0, 0.0), (0.0, PT), (0.0, 2.0 * PT), (0.0, -PT)];
    assert_close(&got, &want, 1e-3);
}

/// A 5-star: the hub is one step from four leaves, every leaf one step from the nearest
/// leaf through the hub, so the hub's `nStepsToLeaf` is 1 against the leaves' 0 and it
/// wins. Each leaf's subtree holds one of the four leaves, so each takes a quadrant and
/// the ring's radius is one inch.
#[test]
fn a_five_node_star_spreads_its_four_leaves_across_the_quadrants() {
    let edges = [(0, 1), (0, 2), (0, 3), (0, 4)];
    let want = [
        (0.0, 0.0),
        (HALF_DIAGONAL, HALF_DIAGONAL),
        (-HALF_DIAGONAL, HALF_DIAGONAL),
        (-HALF_DIAGONAL, -HALF_DIAGONAL),
        (HALF_DIAGONAL, -HALF_DIAGONAL),
    ];
    assert_close(&points(&run(&graph(5, &edges)).unwrap()), &want, 1e-3);
}

/// Two nodes: both are leaves, and a leaf's own `nStepsToLeaf` is never raised — the
/// relaxation writes only on strictly smaller, and `0` is already minimal — so both stay
/// at 0 and the tie goes to the lower index, `n0`.
///
/// `n0` has one child, so `setSubtreeSize` skips it (`circle.c:175-177`) and its subtree is
/// one leaf, the same as `n1`'s: the single child's span is therefore the whole `2*PI`, not
/// half of it, and `n1` lands at `theta = PI`. Graphviz draws the pair one inch apart,
/// which is all `-Tplain` can show — the translation onto the bounding box hides which of
/// the two is the centre.
#[test]
fn two_nodes_tie_on_the_leaf_test_and_the_first_takes_the_centre() {
    let got = points(&run(&graph(2, &[(0, 1)])).unwrap());
    assert_close(&got, &[(0.0, 0.0), (-PT, 0.0)], 1e-3);
}

/// The whole sweep on a branch, which is where the leaf counts and the sibling order both
/// bite: `n2` is two steps from the nearest leaf against `n0`'s and `n4`'s one, so it is
/// the root, its subtree holds three leaves, `n0`'s two and `n4`'s one, and the two
/// subtrees take `4*PI/3` and `2*PI/3` of the circle — with `n4` first, because `n2`'s
/// out-edge (`n2 -- n4`) precedes its in-edge (`n0 -- n2`).
///
/// Every number below is derived from `circle.c`, and each is confirmed by
/// `twopi -Tplain` on the same DOT, which prints `n0 1.875 1.116`, `n1 0.375 1.9821`,
/// `n2 2.375 1.9821`, `n3 3.375 0.25`, `n4 2.875 2.8481`, `n5 3.375 3.7141` — the drawing
/// shifted by the translation onto its bounding box.
#[test]
fn a_branch_divides_the_circle_by_the_leaves_below_each_subtree() {
    let edges = [(0, 1), (0, 2), (0, 3), (2, 4), (4, 5)];
    let want = [
        (-0.5 * PT, -0.866_025_4 * PT), // n0, one ring out at 240 degrees
        (-2.0 * PT, 0.0),               // n1, two rings out at 180
        (0.0, 0.0),                     // n2, the root
        (PT, -0.866_025_4 * PT * 2.0),  // n3, two rings out at 300
        (0.5 * PT, 0.866_025_4 * PT),   // n4, one ring out at 60
        (PT, 0.866_025_4 * PT * 2.0),   // n5, two rings out at 60
    ];
    let got = points(&run(&graph(6, &edges)).unwrap());
    assert_close(&got, &want, 1e-2);
}

/// A parallel edge is one neighbour, not two: `isLeaf` (`circle.c:55-72`) counts
/// *distinct* neighbours and skips the loop, so a doubled edge neither makes the node a
/// non-leaf nor doubles its subtree. The subtree guard `SPAN(next) != 0` (`circle.c:199`)
/// is what stops the second edge assigning the child's span twice.
#[test]
fn a_parallel_edge_is_one_neighbour_and_assigns_the_span_once() {
    let once = points(&run(&graph(2, &[(0, 1)])).unwrap());
    let twice = points(&run(&graph(2, &[(0, 1), (0, 1)])).unwrap());
    assert_close(&twice, &once, 1e-4);
}

/// An isolated node is a leaf (`isLeaf` finds no neighbour, so `neighp` stays null), so
/// an edgeless graph is all leaves at zero steps and each node is its own component's
/// first maximum — hence every node at the origin. This is the disconnected Ponytail, and
/// the reason it is a *test* and not a note: the reference refuses the whole drawing
/// (`twopi: use of weight=0 creates disconnected component`) and the harness records
/// nothing, so this position is ours alone.
#[test]
fn an_isolated_node_is_a_leaf_and_is_its_own_centre() {
    assert_eq!(points(&run(&graph(3, &[])).unwrap()), [(0.0, 0.0); 3]);
}

/// Two components are laid out one after another and **both around the origin**, so they
/// overlap: the reference would have packed them apart with `packSubgraphs`, which this
/// port does not translate. Every node still lands on its own ring, which is the direction
/// the module doc records as cosmetic.
#[test]
fn two_components_are_both_drawn_around_the_origin_and_overlap() {
    let got = points(&run(&graph(4, &[(0, 1), (2, 3)])).unwrap());
    let each = [(0.0, 0.0), (-PT, 0.0)];
    assert_close(&got, &[each[0], each[1], each[0], each[1]], 1e-3);
}

/// The centre is the node **furthest from the nearest leaf**, so a graph whose diameter
/// end is not node 0 still centres on the middle. A 5-path (`n0-n1-n2-n3-n4`) has leaves
/// `n0` and `n4`; `n2` is two steps from each and wins.
#[test]
fn the_centre_is_the_node_furthest_from_the_nearest_leaf() {
    let edges = [(0, 1), (1, 2), (2, 3), (3, 4)];
    let got = points(&run(&graph(5, &edges)).unwrap());
    assert_close(&[(got[2].0, got[2].1)], &[(0.0, 0.0)], 1e-3);
    let want = [
        (0.0, -2.0 * PT),
        (0.0, -PT),
        (0.0, 0.0),
        (0.0, PT),
        (0.0, 2.0 * PT),
    ];
    assert_close(&got, &want, 1e-3);
}

/// The radius is the BFS depth, not the position in the drawing, and the centre is the
/// first *maximum*, which here is not node 0: `n0`, `n3` and `n4` are leaves at zero
/// steps, `n1` is one step from `n0` and `n2` is one step from `n3`, so `n1` and `n2` tie
/// at 1 and `n1` — the lower index — wins (`circle.c:108`).
///
/// Graphviz `twopi -Tplain` on `n0--n1, n1--n2, n2--n3, n1--n4` puts `n1` at the centre of
/// the drawing and the four others 1, 1, 1 and 2 inches from it.
#[test]
fn the_radius_is_the_bfs_depth_and_the_centre_is_the_first_maximum() {
    let edges = [(0, 1), (1, 2), (2, 3), (1, 4)];
    let got = points(&run(&graph(5, &edges)).unwrap());
    assert_close(&[(got[1].0, got[1].1)], &[(0.0, 0.0)], 1e-3);
    for (node, want) in [(0u32, PT), (2, PT), (3, 2.0 * PT), (4, PT)] {
        assert_radius(got[node as usize], want, node);
    }
}

/// `got` is `want` points from the origin, checked as a radius so the angle is not pinned
/// twice by the same numbers.
#[track_caller]
fn assert_radius(got: (f32, f32), want: f32, node: u32) {
    let (x, y) = (f64::from(got.0), f64::from(got.1));
    let radius = f64::sqrt(x * x + y * y);
    assert!(
        (radius - f64::from(want)).abs() < 1e-2,
        "n{node} sits {radius} points from the centre, not {want}"
    );
}
