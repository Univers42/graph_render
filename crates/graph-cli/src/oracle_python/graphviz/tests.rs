//! The Graphviz differential's metric: the rescale, the gap, and the two readers.
//!
//! **These are the tests that make the ceiling mean something.** A tolerance metric that
//! cannot see a real disagreement is a number, not a check, so each of these perturbs one
//! thing and demands the metric move: a pure translation and a pure scale must *not* move
//! it (those two say nothing about the tiling), and a moved node, a stretched drawing and a
//! permuted node order each must.

use super::super::graphviz_arm::{bbox_of, gap, read_fixture, read_oracle, rescale};
use serde_json::json;

/// A two-by-two grid of unit squares — the shape a squarified treemap draws for four equal
/// nodes, and the smallest shape a permutation is visible on.
fn grid() -> Vec<(f64, f64)> {
    vec![(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
}

#[test]
fn the_bounding_box_is_the_corner_and_the_span() {
    let ((low_x, low_y), (width, height)) = bbox_of(&grid());
    assert_eq!((low_x, low_y, width, height), (0.0, 0.0, 1.0, 1.0));
}

#[test]
fn the_bounding_box_of_one_node_has_no_span() {
    let ((low_x, low_y), (width, height)) = bbox_of(&[(4.0, -2.0)]);
    assert_eq!((low_x, low_y, width, height), (4.0, -2.0, 0.0, 0.0));
}

/// Translation and scale are what the rescale exists to remove, and a drawing that differs
/// only by those two is the same tiling. If this fails, the metric is grading the frame.
#[test]
fn a_translation_and_a_scale_alone_move_nothing() {
    let theirs = grid();
    let mine: Vec<(f64, f64)> = theirs
        .iter()
        .map(|&(x, y)| (x * 7.5 + 100.0, y * 7.5 - 40.0))
        .collect();
    assert_eq!(gap(&mine, &theirs).expect("comparable"), 0.0);
}

/// A single node moved by 1e-6 points — the perturbation `docs/decisions/graphviz-oracle.md`
/// records as the non-vacuity check on the oracle's `cmp`, applied here to the metric.
///
/// Asserted at the *order* of the move rather than exactly: the node moved sits on the
/// bounding box, so moving it also moves the box, and the rescale's uniform scale is
/// therefore part of what the gap sees — it arrives a half-ulp under the move, not exactly on
/// it. What this pins is that the metric reports a difference at all where a blind one would
/// report `0.0`, which is the whole claim.
#[test]
fn one_coordinate_moved_by_a_printed_digit_is_a_gap() {
    let theirs = grid();
    let mut mine = theirs.clone();
    mine[2].1 += 1e-6;
    let got = gap(&mine, &theirs).expect("comparable");
    assert!(
        (1e-7..=1e-6).contains(&got),
        "a 1e-6 move registered as {got}"
    );
}

/// The point of one uniform scale rather than one per axis: a stretched drawing is a *larger*
/// gap, not a gap the metric normalises away.
#[test]
fn a_stretched_drawing_is_a_larger_gap_than_a_shifted_one() {
    let theirs = grid();
    let shifted: Vec<(f64, f64)> = theirs.iter().map(|&(x, y)| (x, y + 0.25)).collect();
    let stretched: Vec<(f64, f64)> = theirs.iter().map(|&(x, y)| (x * 2.0, y)).collect();
    assert!(gap(&stretched, &theirs).expect("comparable") > gap(&shifted, &theirs).expect("ok"));
}

/// A squarified treemap is a *packing*: getting node `n3`'s square is what makes the drawing
/// read. Swapping two nodes' coordinates leaves every tile's shape intact and the field
/// still full, and must still be a gap — this is the failure a bounding-box-only metric
/// would report as exact.
#[test]
fn a_permuted_node_order_is_a_gap() {
    let theirs = grid();
    let mut mine = theirs.clone();
    mine.swap(1, 2);
    assert!(gap(&mine, &theirs).expect("comparable") > 1e-3);
}

/// The rescale maps onto the *target's* box, so both arms land in the same frame and the gap
/// is a shape difference and nothing else. Asserted directly rather than through `gap` so a
/// change to the metric cannot hide it.
#[test]
fn the_rescale_lands_both_arms_on_the_targets_own_box() {
    let theirs = grid();
    let mine: Vec<(f64, f64)> = theirs.iter().map(|&(x, y)| (x * 3.0, y * 3.0)).collect();
    let target = bbox_of(&theirs);
    let scaled = rescale(&mine, target);
    assert_eq!(scaled, theirs, "one uniform scale onto the target's box");
}

/// A degenerate cloud — one node, or a treemap row that is a straight line — has no span to
/// scale by, and must not divide by zero to grade it.
///
/// **It is graded in the same frame as every other drawing, not a special one of its own.**
/// That is the claim: the degenerate branch translates our lower-left corner onto the origin,
/// exactly as the scaling branch's translation does, so a single-node drawing placed anywhere
/// in the plane is the same drawing as any other single-node drawing. Subtracting the
/// *target's* corner instead would put the two arms in two different frames and grade the
/// distance between them as a shape difference.
#[test]
fn a_degenerate_cloud_is_translated_rather_than_divided() {
    let target = bbox_of(&[(3.0, 4.0)]);
    assert_eq!(rescale(&[(3.0, 4.0)], target), vec![(0.0, 0.0)]);
    assert_eq!(rescale(&[(10.0, 20.0)], target), vec![(0.0, 0.0)]);
    assert_eq!(
        gap(&[(10.0, 20.0)], &[(3.0, 4.0)]).expect("comparable"),
        0.0
    );
    let line = vec![(0.0, 5.0), (0.0, 1.0)];
    assert!(
        rescale(&line, bbox_of(&line))
            .iter()
            .all(|&(x, y)| x.is_finite() && y.is_finite())
    );
}

#[test]
fn the_two_arms_must_hold_the_same_node_count() {
    assert!(gap(&grid(), &grid()[..3]).is_err());
    let theirs = grid();
    assert_eq!(gap(&theirs, &theirs).expect("comparable"), 0.0);
}

/// Our arm is read from the `{x, y}` columns the emit wrote under the engine's key. A
/// fixture whose x and y columns disagree in length is refused rather than zipped short.
#[test]
fn the_ours_arm_is_read_in_dense_index_order_and_refuses_a_ragged_line() {
    let line = json!({"patchwork": {"x": [1.0, 2.0], "y": [3.0]}});
    let err = read_fixture(&line, "patchwork").expect_err("ragged");
    assert!(err.contains("x against"), "{err}");
    let line = json!({"patchwork": {"x": [1.0, 2.0], "y": [3.0, 4.0]}});
    assert_eq!(
        read_fixture(&line, "patchwork").expect("reads"),
        vec![(1.0, 3.0), (2.0, 4.0)]
    );
}

/// Graphviz's arm is keyed by node *name*, not by position: `-Tplain`'s node order is the DOT
/// declaration order, which the harness happens to write dense, but a plain-format
/// reordering must not be graded as a layout difference.
#[test]
fn the_graphviz_arm_is_keyed_by_name_and_not_by_position() {
    let record = json!({"nodes": {"n1": [2.0, 4.0], "n0": [1.0, 3.0]}});
    assert_eq!(
        read_oracle(&record, "nodes").expect("reads"),
        vec![(1.0, 3.0), (2.0, 4.0)]
    );
}

/// An id that is not a dense node id, or one past the count, is refused: a silent drop would
/// compare the wrong nodes and report a gap nobody can explain.
#[test]
fn the_graphviz_arm_refuses_an_id_outside_the_dense_range() {
    for record in [
        json!({"nodes": {"n0": [1.0, 3.0], "x": [2.0, 4.0]}}),
        json!({"nodes": {"n0": [1.0, 3.0], "n2": [2.0, 4.0]}}),
        json!({"nodes": {"n0": [1.0, 3.0], "n1": [2.0]}}),
        json!({"nodes": {"n0": [1.0, 3.0], "n1": "somewhere"}}),
    ] {
        assert!(read_oracle(&record, "nodes").is_err(), "{record}");
    }
}
