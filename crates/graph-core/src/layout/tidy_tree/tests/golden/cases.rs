//! The tests for [`witnesses`] and [`ties`], and the suite's negative control.

use super::{
    CHAIN8, EXECUTE_SHIFTS_WITNESS, FAN10, FINISH_SECOND_BRANCH, LEFT_TIE, M_SHIFT_WITNESS,
    RIGHT_TIE, Row, SIGN_FLIP_WITNESS, TREE_BALANCED, TREE_DEGENERATE, UNBALANCED, assert_golden,
    assert_indexed, build, dense_ids,
};
use crate::layout::hierarchy::fixture::load;

#[test]
fn the_execute_shifts_witness_matches_d3_hierarchy_bit_for_bit() {
    assert_indexed(
        &[(0, 1), (1, 2), (0, 3), (0, 4), (1, 5), (4, 6)],
        &EXECUTE_SHIFTS_WITNESS,
        "execute-shifts witness",
    );
}

#[test]
fn the_modifier_shift_witness_matches_d3_hierarchy_bit_for_bit() {
    assert_indexed(
        &[
            (0, 1),
            (0, 2),
            (0, 3),
            (0, 4),
            (2, 5),
            (0, 6),
            (2, 7),
            (2, 8),
            (7, 9),
            (6, 10),
            (5, 11),
        ],
        &M_SHIFT_WITNESS,
        "modifier-shift witness",
    );
}

/// The edges of [`SIGN_FLIP_WITNESS`], as (parent, child) dense-index pairs, admission
/// order = id order. Split out so the test below stays under the house line limit.
const SIGN_FLIP_PAIRS: [(usize, usize); 47] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (1, 4),
    (1, 5),
    (0, 6),
    (2, 7),
    (5, 8),
    (7, 9),
    (8, 10),
    (10, 11),
    (10, 12),
    (6, 13),
    (12, 14),
    (7, 15),
    (10, 16),
    (14, 17),
    (8, 18),
    (10, 19),
    (11, 20),
    (1, 21),
    (14, 22),
    (0, 23),
    (9, 24),
    (5, 25),
    (12, 26),
    (16, 27),
    (15, 28),
    (17, 29),
    (20, 30),
    (19, 31),
    (11, 32),
    (1, 33),
    (26, 34),
    (5, 35),
    (4, 36),
    (35, 37),
    (18, 38),
    (32, 39),
    (29, 40),
    (26, 41),
    (24, 42),
    (6, 43),
    (15, 44),
    (38, 45),
    (26, 46),
    (43, 47),
];

#[test]
fn the_modifier_sign_flip_witness_matches_d3_hierarchy_bit_for_bit() {
    assert_indexed(
        &SIGN_FLIP_PAIRS,
        &SIGN_FLIP_WITNESS,
        "modifier sign-flip witness",
    );
}

#[test]
fn the_second_finish_contour_branch_matches_d3_hierarchy_bit_for_bit() {
    assert_indexed(
        &[(0, 1), (0, 2), (0, 3), (0, 4), (4, 5)],
        &FINISH_SECOND_BRANCH,
        "finish_contour second branch",
    );
}

#[test]
fn the_left_extreme_tie_break_matches_d3_hierarchy_bit_for_bit() {
    assert_indexed(&[(0, 1), (1, 2), (0, 3)], &LEFT_TIE, "left tie");
}

#[test]
fn the_right_extreme_tie_break_matches_d3_hierarchy_bit_for_bit() {
    assert_indexed(
        &[(0, 1), (1, 2), (1, 3), (1, 4), (0, 5)],
        &RIGHT_TIE,
        "right tie",
    );
}

#[test]
fn the_tree_balanced_fixture_matches_d3_hierarchy_bit_for_bit() {
    let (nodes, edges) = load("tree-balanced");
    let dense = dense_ids(&nodes);
    assert_golden(
        &build(&nodes, &edges),
        &dense,
        &TREE_BALANCED,
        "tree-balanced",
    );
}

#[test]
fn the_tree_degenerate_fixture_matches_d3_hierarchy_bit_for_bit() {
    let (nodes, edges) = load("tree-degenerate");
    let dense = dense_ids(&nodes);
    assert_golden(
        &build(&nodes, &edges),
        &dense,
        &TREE_DEGENERATE,
        "tree-degenerate",
    );
}

/// The negative control for the goldens above: if every shape pinned the same thing —
/// the normalisation and nothing else — these would all match each other. Asserting they
/// differ is what makes the unbalanced and caterpillar goldens meaningful rather than
/// decorative.
#[test]
fn the_golden_shapes_are_not_all_the_same_layout() {
    let column = |rows: &[Row]| rows.iter().all(|r| r.1 == rows[0].1);
    assert!(column(&CHAIN8), "the chain is one column by construction");
    assert!(!column(&FAN10), "a fan is not a column");
    assert!(!column(&UNBALANCED), "an unbalanced tree is not a column");
    assert!(
        UNBALANCED.windows(2).any(|w| w[0].1 != w[1].1),
        "the unbalanced tree must have contour-sensitive x to pin"
    );
    assert_ne!(
        LEFT_TIE.iter().map(|r| r.1).collect::<Vec<_>>(),
        RIGHT_TIE.iter().map(|r| r.1).collect::<Vec<_>>(),
        "the two tie-break shapes must not agree, or one of them is vacuous"
    );
}
