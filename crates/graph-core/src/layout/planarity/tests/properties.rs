//! Determinism, order-independence and scale properties [`planar_embedding`] must have,
//! not tied to any one graph's own planarity.

use super::graphs::{complete_graph, maximal_planar, path};
use crate::layout::planarity::{euler_certificate, faces, planar_embedding, triangulate_embedding};
use crate::synthetic::Mulberry32;

/// **Every graph on up to six nodes, exhaustively** (33 868 of them: every edge subset of
/// `K_0` .. `K_6`), plus 400 seeded samples above that, through the whole public path:
/// [`planar_embedding`], then [`faces`], then [`triangulate_embedding`].
///
/// This is the safety net the three `expect`s in this module and its children rest on.
/// They sit on a path whose documented promise is a **fail-safe `None`**, not a panic, and
/// nothing in the tree can show that for an arbitrary input: the invariants they name
/// (a reciprocal rotation, a back-edge owner that also owns a tree edge, a row's `cw`
/// cycle closing after exactly `degree(v)` steps) are established by `embed::build` and
/// nowhere asserted. A violation is therefore not a failing assertion on one fixture — it
/// is a crash in a caller that asked a yes/no question. Here it is a red test instead.
///
/// What it pins, beyond "nothing panicked": every embedding handed back is
/// Euler-certified, every traced face has at least two nodes (the two-node face is what
/// [`triangulate_embedding`] leaves to its own guard), and the outer boundary is empty for
/// no graph but the empty one.
#[test]
fn no_small_graph_reaches_an_expect_on_the_public_planarity_path() {
    let mut checked = 0usize;
    for n in 0..=6u32 {
        for edges in subsets(&complete_graph(n)) {
            checked += 1;
            check_one(n, &edges);
        }
    }
    for seed in 0..400u32 {
        let n = 7 + seed % 3;
        let mut rng = Mulberry32::new(seed);
        let edges: Vec<(u32, u32)> = complete_graph(n)
            .into_iter()
            .filter(|_| rng.pick(2) == 0)
            .collect();
        checked += 1;
        check_one(n, &edges);
    }
    assert!(checked > 30_000, "the sweep shrank: {checked} graphs");
}

/// Every edge subset of `pairs`, ascending in `(a, b)` — the whole graph space, in a
/// fixed order, with no randomness in the enumeration itself.
fn subsets(pairs: &[(u32, u32)]) -> Vec<Vec<(u32, u32)>> {
    let count = pairs.len();
    (0..(1u32 << count))
        .map(|mask| {
            pairs
                .iter()
                .enumerate()
                .filter(|(i, _)| mask >> i & 1 == 1)
                .map(|(_, &pair)| pair)
                .collect()
        })
        .collect()
}

/// The whole public path over one graph. The `unwrap_or`-free early return is the point:
/// a non-planar graph must stop at `planar_embedding`, never reach the rest.
fn check_one(n: u32, edges: &[(u32, u32)]) {
    let Some(embedding) = planar_embedding(n, edges) else {
        return;
    };
    assert!(euler_certificate(&embedding), "n={n} edges={edges:?}");
    let traced = faces(&embedding);
    for i in 0..traced.len() {
        assert!(
            traced.face(i).len() >= 2,
            "a face shorter than a digon: n={n} edges={edges:?}"
        );
    }
    let (tri, outer) = triangulate_embedding(&embedding);
    assert!(
        euler_certificate(&tri),
        "triangulated: n={n} edges={edges:?}"
    );
    assert_eq!(outer.is_empty(), n == 0, "n={n} edges={edges:?}");
}

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
