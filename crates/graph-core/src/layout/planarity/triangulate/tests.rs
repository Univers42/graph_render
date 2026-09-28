//! The triangulator's growable rotation, one operation at a time: the copy out of an
//! [`Embedding`], a fresh half-edge, the two splices, the row lookup, the component
//! representatives and the CSR read-back.
//!
//! The integration tests reach this code only through [`triangulate_embedding`]'s
//! finished result, where a chord inserted at the wrong place in a row still produces a
//! valid triangulation of the same graph — same node count, same face set, different
//! rotation. So what is pinned here is the *intermediate* rotation, half-edge id for
//! half-edge id, where a wrong id or a missed pointer is visible.
//!
//! The golden values were read off the module after the fix, by printing the tables;
//! each sits on top of the behavioural tests in `tests::triangulate` (every face but the
//! largest is a triangle) and `tests::sweep`.

use super::*;
use crate::layout::planarity::planar_embedding;

/// `K_4`. Every row has three half-edges, which is what separates `cw` from `ccw` — on a
/// row of two they are the same array.
fn k4() -> Embedding {
    planar_embedding(4, &[(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]).expect("K4 is planar")
}

/// `from_embedding` is the boundary between the two representations: the fixed-slot CSR
/// the LR test produced becomes a growable cycle. Each row is closed on itself, anchored
/// on its first half-edge, with `visited` clear and `to` a straight copy.
#[test]
fn the_growable_rotation_is_exact_for_k4_and_for_a_four_cycle() {
    let square = planar_embedding(4, &[(0, 1), (1, 2), (2, 3), (0, 3)]).expect("a 4-cycle");

    let b = Builder::from_embedding(&k4());
    assert_eq!(b.to, [1, 3, 2, 0, 2, 3, 0, 3, 1, 0, 1, 2]);
    assert_eq!(
        b.cw,
        [1, 2, 0, 4, 5, 3, 7, 8, 6, 10, 11, 9],
        "row 0: 0-1-3-2-0"
    );
    assert_eq!(b.ccw, [2, 0, 1, 5, 3, 4, 8, 6, 7, 11, 9, 10], "its mirror");
    assert_eq!(
        b.any,
        [Some(0), Some(3), Some(6), Some(9)],
        "first of each row"
    );
    assert_eq!(b.visited, [false; 12], "nothing traced yet");

    let s = Builder::from_embedding(&square);
    assert_eq!(
        s.cw,
        [1, 0, 3, 2, 5, 4, 7, 6],
        "every row is a 2-cycle here"
    );
    assert_eq!(s.any, [Some(0), Some(2), Some(4), Some(6)]);
}

/// An isolated node has no half-edge at all, so its row has no anchor — which is what
/// `find` returns `None` for and what `connect_pair` and `into_embedding` both branch on.
#[test]
fn an_isolated_node_anchors_nothing_and_answers_no_lookup() {
    let embedding = planar_embedding(3, &[(0, 1)]).expect("an edge is planar");
    let b = Builder::from_embedding(&embedding);
    assert_eq!(b.any, [Some(0), Some(1), None]);
    assert_eq!(b.find(2, 0), None, "no row to walk");
    assert_eq!(b.find(0, 1), Some(0));
    assert_eq!(b.find(0, 0), None, "and no self-loop half-edge either");
}

/// `new_half_edge` grows four tables together and hands back the new id, leaving its
/// `cw`/`ccw` pointing at itself so the row is still a valid cycle for the caller to
/// splice into. A table that grew without the others would index past its end.
#[test]
fn a_fresh_half_edge_grows_every_table_and_stands_alone() {
    let mut b = Builder::from_embedding(&k4());
    let id = b.new_half_edge(0);
    assert_eq!(id, 12, "one past the last half-edge");
    assert_eq!(b.to, [1, 3, 2, 0, 2, 3, 0, 3, 1, 0, 1, 2, 0]);
    assert_eq!(b.cw[12], 12);
    assert_eq!(b.ccw[12], 12);
    assert_eq!(b.visited.len(), 13, "visited grows with the rest");
}

/// The two splices again, this time on the growable side: same four pointers, and each
/// returns the id it allocated so `add_chord` can wire the far end to the same chord.
#[test]
fn a_splice_on_the_growable_side_rewires_four_pointers_and_returns_its_id() {
    let mut after = Builder::from_embedding(&k4());
    assert_eq!(after.splice_after(0, 3), 12);
    assert_eq!((after.cw[0], after.cw[12]), (12, 1), "0 - 12 - 1");
    assert_eq!((after.ccw[12], after.ccw[1]), (0, 12), "and back");
    assert_eq!(
        after.to[12], 3,
        "the new half-edge targets what it was told"
    );

    let mut before = Builder::from_embedding(&k4());
    assert_eq!(before.splice_before(0, 3), 12);
    assert_eq!((before.cw[12], before.cw[0]), (0, 1), "12 - 0 - 1");
    assert_eq!((before.ccw[0], before.ccw[1]), (12, 0), "and back");
}

/// `component_reps` is what makes the component join deterministic: the lowest dense
/// index of each component, ascending. Anything else — the DFS that reached it last, a
/// hash order — would put the new edge somewhere different in the rotation.
#[test]
fn the_component_representatives_are_the_lowest_index_of_each_component_in_order() {
    let joined = planar_embedding(5, &[(0, 1), (1, 2), (3, 4)]).expect("planar");
    assert_eq!(Builder::from_embedding(&joined).component_reps(), [0, 3]);
    let one = planar_embedding(6, &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)]).expect("planar");
    assert_eq!(Builder::from_embedding(&one).component_reps(), [0]);
    let scattered = planar_embedding(6, &[(4, 5), (0, 1), (1, 2), (0, 2)]).expect("planar");
    assert_eq!(
        Builder::from_embedding(&scattered).component_reps(),
        [0, 3, 4],
        "an isolated node is a component too, and a higher one still follows a lower \
         one whatever the edge order was"
    );
}

/// `connect_pair` is deliberately asymmetric between its two ends: each is spliced after
/// *its own* row's anchor, so a node that is the join target and a node that is the join
/// source end up with different rotations even though the edge is the same.
#[test]
fn connecting_two_components_splices_each_end_after_its_own_anchor() {
    let mut b = Builder::from_embedding(&k4());
    b.connect_pair(0, 1);
    assert_eq!(b.to[12], 1, "row 0 gains a half-edge to 1");
    assert_eq!(b.to[13], 0, "row 1 gains one back");
    assert_eq!((b.cw[0], b.cw[12]), (12, 1), "right after row 0's anchor");
    assert_eq!(
        (b.cw[3], b.cw[13]),
        (13, 4),
        "right after row 1's own anchor"
    );
    assert_eq!(
        b.any,
        [Some(0), Some(3), Some(6), Some(9)],
        "anchors do not move"
    );
}

/// The other arm: a component with no half-edge at all gets the bare edge as its anchor,
/// which is the only way an isolated node ever enters the rotation.
#[test]
fn connecting_to_an_isolated_node_gives_it_a_bare_anchor() {
    let embedding = planar_embedding(3, &[(0, 1)]).expect("an edge is planar");
    let mut b = Builder::from_embedding(&embedding);
    b.connect_pair(0, 2);
    assert_eq!(
        b.any,
        [Some(0), Some(1), Some(3)],
        "node 2 now anchors on the new edge"
    );
    assert_eq!(
        (b.cw[3], b.ccw[3]),
        (3, 3),
        "which is its own one-slot cycle"
    );
    let joined = b.into_embedding();
    assert_eq!(joined.offsets(), [0, 2, 3, 4]);
    assert_eq!(joined.neighbours(), [1, 2, 0, 0], "0-1, 0-2, and now 2-0");
    assert_eq!(joined.edge_count(), 2, "one new edge, both ways");
}

/// `into_embedding` reads each row out from its anchor rather than from its lowest id —
/// the two differ as soon as a row has been spliced into — so a graph is not a fixed
/// point of this function and cannot be.
#[test]
fn the_read_out_walks_each_row_from_its_anchor_not_from_its_lowest_id() {
    let b = Builder::from_embedding(&k4());
    let read = b.into_embedding();
    assert_eq!(read.offsets(), [0, 3, 6, 9, 12]);
    assert_eq!(read.neighbours(), [1, 3, 2, 0, 2, 3, 0, 3, 1, 0, 1, 2]);
    assert_eq!(read.node_count(), 4);
    assert_eq!(read.edge_count(), 6);
    assert_eq!(
        Builder::from_embedding(&k4()).into_embedding(),
        read,
        "deterministic"
    );
}
