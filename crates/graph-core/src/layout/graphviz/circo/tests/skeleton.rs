//! The blocks the skeleton pass has edges to delete in: the complete graphs, the 6-cycle and
//! the chorded 5-cycle, each pinned against `circo -Tplain` like the cases in the parent.

use super::super::run;
use super::slot;
use crate::layout::coords::probe::{assert_close, graph, points};

/// `K4`: one block of four where every node is a neighbour of every other, so the skeleton
/// pass has a pair edge to delete and the tree splits — the smallest graph that exercises
/// `place_residual_nodes`. The circle starts at `n1`, not at `n0`. `-Tplain` prints `1.4891
/// 0.25`, `2.6032 1.3641`, `1.4891 2.4782` and `0.375 1.3641`.
#[test]
fn a_complete_four_starts_at_its_first_leaf() {
    let edges = [(0, 1), (1, 2), (2, 3), (3, 0), (0, 2), (1, 3)];
    let got = points(&run(&graph(4, &edges)).expect("lays out"));
    assert_close(
        &got,
        &[
            slot(4.0, 3.0),
            slot(4.0, 0.0),
            slot(4.0, 1.0),
            slot(4.0, 2.0),
        ],
        1e-2,
    );
}

/// `K5`: the same at five nodes, with every pair an edge, so two skeleton passes run and the
/// residual pass fills three slots. `-Tplain` prints `1.932 0.25`, `2.8942 1.5744`,
/// `0.375 2.393`, `0.375 0.75589` and `1.932 2.8989`.
#[test]
fn a_complete_five_still_lands_on_one_circle() {
    let edges = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 4),
        (4, 0),
        (0, 2),
        (1, 4),
        (2, 4),
        (3, 0),
    ];
    let got = points(&run(&graph(5, &edges)).expect("lays out"));
    assert_close(
        &got,
        &[
            slot(5.0, 4.0),
            slot(5.0, 0.0),
            slot(5.0, 2.0),
            slot(5.0, 3.0),
            slot(5.0, 1.0),
        ],
        1e-2,
    );
}

/// A 6-cycle, where three skeleton passes run and the tree still spans the block. `-Tplain`
/// prints `2.8817 0.25`, `1.2106 0.25`, `0.375 1.6972`, `1.2106 3.1445`, `2.8817 3.1445` and
/// `3.7173 1.6972`.
#[test]
fn a_six_cycle_starts_at_its_last_node() {
    let edges = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 0)];
    let got = points(&run(&graph(6, &edges)).expect("lays out"));
    assert_close(
        &got,
        &[
            slot(6.0, 5.0),
            slot(6.0, 4.0),
            slot(6.0, 3.0),
            slot(6.0, 2.0),
            slot(6.0, 1.0),
            slot(6.0, 0.0),
        ],
        1e-2,
    );
}

/// A 5-cycle with one chord: the chord is the pair edge the skeleton pass deletes, and the
/// circle comes out the same as the plain 5-cycle's. `-Tplain` prints `1.932 0.25`,
/// `0.375 0.75589`, `0.375 2.393`, `1.932 2.8989` and `2.8942 1.5744`.
#[test]
fn a_chorded_five_cycle_loses_the_chord_and_keeps_the_circle() {
    let edges = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 0), (0, 2)];
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
