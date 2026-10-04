//! The analytically determined cases, pinned node by one.
//!
//! Every expectation here is derived from `lib/circogen` — the block decomposition, the
//! radius `N * (min_dist + largest_node) / 2*PI` of `blockpath.c:594`, and the placement
//! arithmetic of `circpos.c` — and then **checked against `circo -Tplain`** in the pinned
//! oracle image, whose printed node lines are reproduced in each test's doc comment. They are
//! byte-compared a second time, on both arms, by `harness/oracle-graphviz.py`; a tolerance
//! would be weaker than the truth these cases carry.
//!
//! A layout that is within a thousandth of Graphviz on a star but wrong on a 3-path has a
//! bug a ceiling would round away, which is why the path, the cycle and the star are all here
//! and not just one of them.

use super::run;
use crate::layout::coords::probe::{assert_close, graph, points};

mod crossings;
mod skeleton;

/// `min_dist + largest_node` = `1.0 + 0.75`, the pair every circle's radius is `N` times over
/// `2*PI`. In points: `72 * 1.75 = 126`, one node's slot on the circle.
const SLOT: f32 = 126.0;

/// The 4-cycle's radius: `4 * 1.75 / 2*PI` inches, promoted to points.
const RING: f32 = 504.0 / (2.0 * std::f64::consts::PI as f32);

/// A one-node graph never becomes a block at all: `circular.c:70-74` puts it at the origin
/// and returns. `-Tplain` prints `node n0 0.375 0.25 0.75 0.5` — the origin plus half a node.
#[test]
fn one_node_sits_at_the_origin() {
    let got = points(&run(&graph(1, &[])).expect("lays out"));
    assert_close(&got, &[(0.0, 0.0)], 1e-4);
}

/// Two nodes are two one-node blocks. The reference coalesces the child into its parent
/// (`circpos.c:381-384`), so the pair sits one `min_dist` apart on the parent's circle:
/// `1.75` inches, which is `126` points. `-Tplain` prints `0.375 0.25` and `2.125 0.25`.
#[test]
fn two_nodes_are_one_slot_apart_on_a_coalesced_circle() {
    let got = points(&run(&graph(2, &[(0, 1)])).expect("lays out"));
    assert_close(&got, &[(-SLOT / 2.0, 0.0), (SLOT / 2.0, 0.0)], 1e-3);
}

/// A 3-path is a chain of three one-node blocks, and each coalescing step doubles the reach:
/// `-1.75, 0, 1.75` inches, or `-126, 0, 126` points. `-Tplain` prints `0.375`, `2.125` and
/// `3.875` on one line, 1.75 inches apart.
#[test]
fn a_three_path_coalesces_outwards_from_its_centre() {
    let got = points(&run(&graph(3, &[(0, 1), (1, 2)])).expect("lays out"));
    assert_close(&got, &[(-SLOT, 0.0), (0.0, 0.0), (SLOT, 0.0)], 1e-3);
}

/// A 4-cycle is one biconnected block, so it is a circle of `N * 1.75 / 2*PI` inches with no
/// coalescing at all — the *only* case here where `circpos.c`'s child arithmetic never runs.
/// The skeleton pass gives the order `n3, n2, n1, n0` (the long path of the spanning tree of
/// the thinned cycle), so `n0` lands last, at `3 * 2*PI / 4`. `-Tplain` prints the four nodes
/// at `1.4891 0.25`, `0.375 1.3641`, `1.4891 2.4782`, `2.6032 1.3641`.
#[test]
fn a_four_cycle_is_one_circle_of_four_slots() {
    let got = points(&run(&graph(4, &[(0, 1), (1, 2), (2, 3), (3, 0)])).expect("lays out"));
    assert_close(
        &got,
        &[(0.0, -RING), (-RING, 0.0), (0.0, RING), (RING, 0.0)],
        1e-2,
    );
}

/// A 5-star is one node with four one-node children, so `positionChildren`'s `length == 1`
/// branch runs with `childCount == 4`: each child claims `incidentAngle + mindistAngle`, and
/// the quarter turn happens to be `2 * incidentAngle + mindistAngle`. The leaves land on the
/// axes. `-Tplain` prints `2.125 2`, `3.875 2`, `2.125 3.75`, `0.375 2` and `2.125 0.25`.
#[test]
fn a_five_star_hangs_its_leaves_on_the_axes() {
    let got = points(&run(&graph(5, &[(0, 1), (0, 2), (0, 3), (0, 4)])).expect("lays out"));
    assert_close(
        &got,
        &[
            (0.0, 0.0),
            (SLOT, 0.0),
            (0.0, SLOT),
            (-SLOT, 0.0),
            (0.0, -SLOT),
        ],
        1e-2,
    );
}

/// A triangle is one block of three, so the circle order is the whole story: the long path of
/// the thinned triangle's spanning tree runs `n2, n1, n0`, `n0` being both the tree's root
/// and the deepest branch node, and `n0` ends up at `2 * 2*PI / 3`. `-Tplain` prints
/// `0.375 0.25`, `0.375 1.6972` and `1.6283 0.97362`.
#[test]
fn a_triangle_is_one_circle_with_the_root_last() {
    let got = points(&run(&graph(3, &[(0, 1), (1, 2), (2, 0)])).expect("lays out"));
    assert_close(
        &got,
        &[slot(3.0, 2.0), slot(3.0, 1.0), slot(3.0, 0.0)],
        1e-2,
    );
}

/// A 5-cycle is one block of five, and it is the case that pins the circle's *starting* node:
/// the long path comes back as `n4, n3, n2, n1, n0`, so `n4` takes slot 0 and the drawing is
/// rotated a fifth of a turn from what the node order alone would suggest. `-Tplain` prints
/// `1.932 0.25`, `0.375 0.75589`, `0.375 2.393`, `1.932 2.8989` and `2.8942 1.5744`.
#[test]
fn a_five_cycle_starts_at_its_own_root() {
    let edges = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 0)];
    let got = points(&run(&graph(5, &edges)).expect("lays out"));
    assert_close(
        &got,
        &[
            slot(5.0, 4.0),
            slot(5.0, 3.0),
            slot(5.0, 2.0),
            slot(5.0, 1.0),
            slot(5.0, 0.0),
        ],
        1e-2,
    );
}

/// A 5-path is the longest coalescing chain, and each step's radius grows by half a slot, so
/// the nodes end up evenly spaced at `-2, -1, 0, 1, 2` slots. `-Tplain` prints `0.375`,
/// `2.125`, `3.875`, `5.625` and `7.375` on one line: 1.75 inches, or 126 points, apart.
#[test]
fn a_five_path_is_evenly_spaced() {
    let edges = [(0, 1), (1, 2), (2, 3), (3, 4)];
    let got = points(&run(&graph(5, &edges)).expect("lays out"));
    assert_close(
        &got,
        &[
            (-2.0 * SLOT, 0.0),
            (-SLOT, 0.0),
            (0.0, 0.0),
            (SLOT, 0.0),
            (2.0 * SLOT, 0.0),
        ],
        1e-2,
    );
}

/// Two one-node stars glued at `n1`, so the block tree is a one-node block with two children
/// under a two-node block under one one-node block — the only closed case that runs
/// `getRotation`'s `coalesced` and two-node branches and a `length == 1` sweep with more than
/// one child. `-Tplain` prints `4.75 2`, `8.25 2`, `2.125 2`, `8.25 0.25`, `8.25 3.75` and
/// `0.375 2`; read back into this port's own frame (`n0` at the origin, 126 points a slot,
/// 189 for the `1 + 7/16` reach the two-star draw gives) that is the row below.
#[test]
fn two_stars_glued_at_a_leaf_keep_their_own_circles_apart() {
    let edges = [(0, 1), (0, 2), (1, 3), (1, 4), (2, 5)];
    let got = points(&run(&graph(6, &edges)).expect("lays out"));
    assert_close(
        &got,
        &[
            (0.0, 0.0),
            (2.0 * SLOT, 0.0),
            (-1.5 * SLOT, 0.0),
            (2.0 * SLOT, -SLOT),
            (2.0 * SLOT, SLOT),
            (-2.5 * SLOT, 0.0),
        ],
        1e-2,
    );
}

/// Two triangles glued at `n2`: two blocks and a cut point, so the block tree has a coalesced
/// parent over two children and the coalesced turn in `getRotation` runs. `-Tplain` prints
/// `0.375 0.97362`, `2.125 0.97362`, `3.9178 0.25`, `3.9178 1.6972` and `5.1711 0.97362`;
/// [`TIP`] is where this port puts the bowtie's left tip, the oracle putting it at the origin —
/// the two frames differ by a translation, which is exactly what the differential's rescale
/// removes.
#[test]
fn two_triangles_glued_at_a_cut_point_coalesce_into_one_line() {
    let edges = [(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 2)];
    let got = points(&run(&graph(5, &edges)).expect("lays out"));
    assert_close(
        &got,
        &[
            (TIP, 0.0),
            (TIP + SLOT, 0.0),
            (TIP + 255.0816, -52.1006),
            (TIP + 255.0816, 52.1006),
            (TIP + 345.32, 0.0),
        ],
        1e-2,
    );
}

/// This port's own frame for the bowtie above: the left tip sits here, where the oracle's frame
/// puts it at the origin.
const TIP: f32 = -159.1606;

/// Point `at` of a `count`-node circle in points, the one place a circle is built in these
/// tests — so a wrong radius cannot hide behind six hand-written trigonometries.
fn slot(count: f64, at: f64) -> (f32, f32) {
    let ring = count * f64::from(SLOT) / (2.0 * std::f64::consts::PI);
    let theta = at * 2.0 * std::f64::consts::PI / count;
    (
        (ring * libm::cos(theta)) as f32,
        (ring * libm::sin(theta)) as f32,
    )
}

#[test]
fn a_disconnected_graph_lays_each_component_out_around_the_origin() {
    let edges = [(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 3)];
    let got = points(&run(&graph(6, &edges)).expect("lays out"));
    let half = |k: usize| (got[k], got[k + 3]);
    assert!(
        half(0).0 == half(0).1,
        "two identical components coincide: {got:?}"
    );
    assert!(got.iter().all(|p| p.0.is_finite() && p.1.is_finite()));
}

/// A component that does not hold node `0`. Every array in the walk is
/// component-sized and indexed by the component's own `slot`, but `PARENT(n)` is a **node**,
/// so it has to come back out of that array as one. Read as a local index instead, the first
/// node's parent reads as node `0`, which belongs to another component: on
/// `fixtures/force/disconnected.json` (node 0 in the triangle, the star and the path behind
/// it) that assertion fired, and under `panic = "abort"` it reached the browser as a wasm
/// trap — `gm_build` never ran again for the rest of the fixture sweep. A triangle-only
/// disconnected graph misses it: one block per component means `assemble` has nothing to
/// hang, so the two triangles above never reach the lookup at all.
///
/// The two connected nodes coalesce exactly as the two-node case above; node `0` is alone,
/// so the reference's own short circuit (`circular.c:70-74`) leaves it at the origin.
#[test]
fn a_component_that_does_not_hold_node_zero_still_finds_its_parent_block() {
    let got = points(&run(&graph(3, &[(1, 2)])).expect("lays out"));
    assert_close(
        &got,
        &[(0.0, 0.0), (-SLOT / 2.0, 0.0), (SLOT / 2.0, 0.0)],
        1e-3,
    );
}
