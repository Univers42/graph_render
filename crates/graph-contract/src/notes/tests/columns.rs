//! The two columns of a [`Notes`] on their own: how they are read, how a note orders, and
//! what `Notes::of` is and is not allowed to do to the order it is handed.

use super::*;

/// The two columns are read from each other nowhere. `len` counts the codes and
/// `is_empty` needs *both* columns empty, so a hand-built pair of different lengths
/// reports itself as what it is and is refused — and since no `Snapshot` can hold one,
/// these are the only inputs these three ever see.
#[test]
fn the_columns_are_never_derived_from_each_other() {
    let empty = Notes::default();
    assert_eq!((empty.len(), empty.is_empty()), (0, true));
    for (code, index) in [
        (vec![], vec![1]),
        (vec![1], vec![]),
        (vec![1, 2], vec![1]),
        (vec![1, 2, 3], vec![3, 2]),
    ] {
        let notes = columns(&code, &index);
        let expected = code.len() as u32;
        assert_eq!(
            (notes.len(), notes.is_empty()),
            (expected, false),
            "{code:?} / {index:?}"
        );
        let refused = Snapshot::new(parts(CURRENT_VERSION, notes)).expect_err("unequal");
        assert_eq!(
            refused,
            E::Length {
                column: "note.index",
                expected: expected.into(),
                found: u64::from(index.len() as u32)
            },
            "{code:?} / {index:?}"
        );
    }
}

/// A note orders by code, then by index; `Notes::of` keeps the order it is given, because
/// sorting is the producer's job and a producer that sorted twice would hide its own bug
/// (the module doc's "the constructor never sorts").
#[test]
fn a_note_orders_by_code_then_index_and_of_keeps_the_order_it_is_given() {
    let first = Note {
        code: NoteCode::CycleEdgeDropped,
        index: 1,
    };
    let later = Note {
        code: NoteCode::CycleEdgeDropped,
        index: 2,
    };
    let other = Note {
        code: NoteCode::ExtraParentDropped,
        index: 0,
    };
    assert!(first < later && later < other, "code first, then index");
    assert!(
        NoteCode::ALL.windows(2).all(|pair| pair[0] < pair[1]),
        "codes ascend"
    );
    let given = Notes::of(&[other, later, first]);
    assert_eq!(given.len(), 3);
    assert_eq!(
        (&given.code, &given.index),
        (&vec![2, 1, 1], &vec![0, 2, 1]),
        "of() copies, it does not sort"
    );
}
