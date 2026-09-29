//! The embedding builder's own machinery, one link at a time: the phase-1 cycle, the two
//! splices, the leftmost hand-over, and the CSR read-out.
//!
//! The integration tests reach this code only through a *correct* embedding, and a
//! correct embedding has the same set of neighbours in every row however the rotation is
//! ordered — so a rotation assembled the wrong way round, or a row started at the wrong
//! half-edge, is invisible to all of them. What is pinned here is the rotation itself,
//! slot for slot.
//!
//! The golden values in this file were read off the module after the fix, by printing
//! the tables; every one of them sits on top of a behavioural test elsewhere in the
//! suite (`tests::positive` for "these graphs are planar", `tests::sweep` for the shape
//! of the face set, which is what a correct rotation is *for*).

use super::super::adjacency::Adjacency;
use super::super::lr;
use super::*;

/// Brings up `adjacency`, a real LR run over it, and a `Builder` over the two — a macro
/// rather than a function, because the builder borrows both and they have to outlive the
/// expression. Never a hand-written `Sides`: one that disagreed with the adjacency would
/// make a failure unreadable.
macro_rules! builder {
    ($b:ident, $n:expr, $edges:expr) => {
        let adjacency = Adjacency::simple($n, &$edges);
        let sides = lr::test(&adjacency).expect("expected planar");
        let mut $b = Builder::new(&adjacency, &sides);
    };
}

/// A triangle: three rows of two slots, with `0 - 1` at slot 0, `1 - 2` at slot 2 and
/// `0 - 2` at slot 4. Small enough that every pointer write below can be read by hand.
const TRIANGLE: [(u32, u32); 3] = [(0, 1), (1, 2), (0, 2)];

/// `link_cycle` is the modulo that makes a row circular: each slot points `cw` at the
/// next one and `ccw` at the previous one, and the ends wrap round to each other. Row
/// lengths 0, 1, 2 and 3 each take a different arm of that wrap, so all four are here —
/// including the empty row, which must touch nothing.
#[test]
fn linking_a_row_into_a_cycle_points_cw_forward_and_ccw_back() {
    builder!(b, 3, TRIANGLE);
    b.link_cycle(&[]);
    assert_eq!((b.cw[0], b.ccw[0]), (0, 0), "an empty row links nothing");
    b.link_cycle(&[2]);
    assert_eq!(
        (b.cw[2], b.ccw[2]),
        (2, 2),
        "a one-slot row is its own cycle"
    );
    b.link_cycle(&[2, 3]);
    assert_eq!((b.cw[2], b.cw[3]), (3, 2), "two slots point at each other");
    b.link_cycle(&[2, 3, 5]);
    assert_eq!(
        (b.cw[2], b.cw[3], b.cw[5]),
        (3, 5, 2),
        "cw wraps to the front"
    );
    assert_eq!(
        (b.ccw[2], b.ccw[3], b.ccw[5]),
        (5, 2, 3),
        "ccw wraps to the back"
    );
}

/// The two splices are the same operation mirrored, and between them they are every
/// write [`Builder::insert_first`] and [`Builder::insert_back_edge`] make. Each has to
/// rewire four pointers: the anchor's side, the new slot's side, and the displaced
/// neighbour's other side. A missed one is a broken cycle that still looks fine from
/// outside, because every neighbour is still there.
#[test]
fn a_splice_rewires_all_four_pointers_around_the_gap() {
    builder!(after, 3, TRIANGLE);
    after.link_cycle(&[2, 3]);
    after.splice_after(2, 1);
    assert_eq!(
        (after.cw[2], after.cw[1], after.cw[3]),
        (1, 3, 2),
        "2 - 1 - 3 - 2"
    );
    assert_eq!(
        (after.ccw[1], after.ccw[3], after.ccw[2]),
        (2, 1, 3),
        "and back"
    );

    builder!(before, 3, TRIANGLE);
    before.link_cycle(&[2, 3]);
    before.splice_before(2, 4);
    assert_eq!(
        (before.cw[4], before.cw[2], before.cw[3]),
        (2, 3, 4),
        "4 - 2 - 3 - 4"
    );
    assert_eq!(
        (before.ccw[2], before.ccw[3], before.ccw[4]),
        (4, 2, 3),
        "and back"
    );
}

/// `insert_first` has two arms, and only the second changes `leftmost`: a row that
/// already has an anchor keeps it (the inserted slot lands *before* the anchor), a row
/// with none starts a fresh one-slot cycle and the inserted slot becomes its anchor. The
/// hand-over is the whole reason a later back-edge insertion knows which neighbour to
/// treat as leftmost.
#[test]
fn insert_first_splices_before_the_current_leftmost_and_then_takes_over() {
    builder!(triangle, 3, TRIANGLE);
    assert_eq!(
        triangle.leftmost,
        [Some(0), Some(3), Some(4)],
        "phase 1 anchors each row on its first DG-out slot"
    );
    triangle.insert_first(1, 0);
    assert_eq!(
        triangle.leftmost,
        [Some(0), Some(2), Some(4)],
        "1 -> 0 takes over"
    );
    assert_eq!(
        (triangle.cw[2], triangle.cw[3]),
        (3, 2),
        "and lands immediately before the slot it displaced"
    );
    assert_eq!(triangle.ccw[3], 2, "which is told about it in turn");
}

/// The other arm: a row with no DG-out slot at all — a leaf of a star, which only ever
/// receives and never sends — has no leftmost, so its first insertion stands alone.
#[test]
fn insert_first_starts_a_row_that_has_no_anchor_yet() {
    builder!(star, 3, [(0, 1), (0, 2)]);
    assert_eq!(
        star.leftmost,
        [Some(0), None, None],
        "both leaves receive only"
    );
    star.insert_first(1, 0);
    assert_eq!(star.leftmost, [Some(0), Some(2), None]);
    assert_eq!((star.cw[2], star.ccw[2]), (2, 2), "so it is its own cycle");
}

/// One read-out case: node count, the graph, and the exact CSR (`offsets`, then
/// `neighbours`) its build has to produce.
type CsrCase = (u32, Vec<(u32, u32)>, Vec<u32>, Vec<u32>);

/// The finished CSR starts every row at its **lowest-numbered slot** and walks `cw` for
/// exactly `degree(v)` steps, so a row comes out as a rotation of the circle the builder
/// assembled and never as a different set of neighbours. The golden rows were read off
/// the module after the fix; "the same neighbours" is the behavioural half, and
/// `tests::checks::assert_valid_embedding` re-derives it independently.
#[test]
fn the_finished_embedding_is_exact_for_three_fixed_graphs() {
    let cases: [CsrCase; 3] = [
        (
            4,
            vec![(0, 1), (1, 2), (2, 3), (0, 3)],
            vec![0, 2, 4, 6, 8],
            vec![1, 3, 0, 2, 1, 3, 0, 2],
        ),
        (3, vec![(0, 1), (0, 2)], vec![0, 2, 3, 4], vec![1, 2, 0, 0]),
        (
            4,
            vec![(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)],
            vec![0, 3, 6, 9, 12],
            vec![1, 3, 2, 0, 2, 3, 0, 3, 1, 0, 1, 2],
        ),
    ];
    for (n, edges, offsets, neighbours) in cases {
        let adjacency = Adjacency::simple(n, &edges);
        let sides = lr::test(&adjacency).expect("expected planar");
        let built = build(&adjacency, &sides);
        assert_eq!(built.offsets(), offsets, "n={n} edges={edges:?}");
        assert_eq!(built.neighbours(), neighbours, "n={n} edges={edges:?}");
        assert_eq!(built.node_count(), n);
        assert_eq!(built.edge_count(), edges.len() as u32, "n={n}");
    }
}
