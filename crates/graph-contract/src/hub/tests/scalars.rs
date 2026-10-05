//! The cells that are carried rather than read: `scalar` numbers and the record
//! size cap.
//!
//! Split out of `batch.rs` by the house's 300-line limit. The two properties here are about
//! *bytes a client sent* rather than about a role's shape: the two number spellings that
//! must survive a round trip unchanged, and the cap on what the store will hold.

use super::super::Limits;
use super::batch::{manifest, one};
use crate::ingest::record_piece;

/// The two number spellings that must survive a round trip byte for byte: `-0` is not
/// `0` on the wire, and `2^53 − 1` is the largest integer a JSON consumer holds exactly.
/// Both are carried in a `scalar` cell, where the motor reads nothing, so the writer is
/// the only thing that can lose them.
#[test]
fn minus_zero_and_the_last_exact_integer_survive_the_record_piece_byte_identically() {
    // The written text, not a quoted string: both are numbers, and `-0` is the point —
    // it is a distinct `f64` from `0` and a client that sent it must see it come back.
    for (cell, text) in [("-0", "-0"), ("9007199254740991", "9007199254740991")] {
        let batch = one(&format!(r#""note":{cell}"#));
        batch
            .check("tracker", &manifest(), &Limits::DEFAULT)
            .expect("a scalar number is accepted");
        let piece = record_piece(&batch.upserts[0].record("tracker"));
        assert!(
            piece.contains(&format!(r#""note":{text}"#)),
            "{cell} must write as {text}: {piece}"
        );
    }
}
