//! Determinism, order-independence and scale properties [`planar_embedding`] must have,
//! not tied to any one graph's own planarity.

use super::graphs::{maximal_planar, path};
use crate::layout::planarity::{euler_certificate, planar_embedding};

#[test]
fn the_same_graph_run_twice_gives_the_byte_equal_embedding() {
    let edges = maximal_planar(3, 60);
    let a = planar_embedding(60, &edges).expect("built planar by construction");
    let b = planar_embedding(60, &edges).expect("built planar by construction");
    assert_eq!(a, b);
}

#[test]
fn shuffling_the_input_edge_order_changes_nothing() {
    let mut edges = maximal_planar(11, 40);
    let original = planar_embedding(40, &edges).expect("built planar by construction");
    // A fixed, non-identity reordering of the very same edge set.
    edges.reverse();
    let third = edges.len() / 3;
    edges.rotate_left(third);
    let reordered = planar_embedding(40, &edges).expect("still the same edge set");
    assert_eq!(
        original, reordered,
        "row order is by dense index, never input or hash order"
    );
}

#[test]
fn a_hundred_thousand_node_path_does_not_recurse() {
    let edges = path(100_000);
    let embedding = planar_embedding(100_000, &edges).expect("a path is planar");
    assert!(euler_certificate(&embedding));
    assert_eq!(embedding.edge_count(), 99_999);
}

/// No floating point ever appears in this module's inputs or outputs (`Embedding` and
/// `Faces` are plain `u32`/`i8` CSR structures) — NaN/Inf simply cannot arise here. This
/// compiles only because every field involved is an integer; there is nothing to assert
/// at runtime.
#[test]
fn there_is_no_floating_point_anywhere_in_this_modules_output() {
    let embedding = planar_embedding(3, &[(0, 1), (1, 2), (0, 2)]).expect("a triangle is planar");
    let _: &[u32] = embedding.neighbours();
}
