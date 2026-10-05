//! The batch's negative controls: what `GM_HUB_BREAK=lax-reader` turns off.
//!
//! Split out of `batch.rs` by the house's 300-line limit. Every other batch test asserts a
//! refusal; this one asserts what happens when the refusal is *deliberately* removed, so
//! the same `cargo test` run proves `negctl-lax-reader`'s break is wired to the reader and
//! not to something unrelated. If this file were absent the row would go red for the wrong
//! reason, or not at all.

use super::super::Limits;
use super::super::batch::read_batch;
use super::batch::QUALIFIED;

/// `GM_HUB_BREAK=lax-reader` turns off two things `batch.rs` pins — the collection-id
/// grammar check and the NUL walk — so `negctl-lax-reader` must go red. This test is what
/// "go red" means: under the break it asserts the *lax* behaviour and therefore passes,
/// while the refusals it relies on fail, so the run proves the break reaches the reader.
#[test]
fn the_lax_reader_break_relaxes_exactly_the_two_reader_refusals() {
    if !crate::hub::breaks::on("lax-reader") {
        return;
    }
    // No `check_collection_id`: a qualified collection is accepted, which is how a client
    // that echoes back a qualified id would be double-qualifying it into
    // `tracker.tracker.task`.
    let batch = read_batch(QUALIFIED, &Limits::DEFAULT).expect("a qualified collection now reads");
    assert_eq!(batch.upserts[0].collection, "tracker.task");
    // No NUL walk: a NUL in a key is accepted.
    let nuled = r#"{"upserts":[{"collection":"task","id":"r1","updatedAt":1,
        "values":{"na\u0000me":1}}],"deletes":[]}"#;
    assert!(read_batch(nuled, &Limits::DEFAULT).is_ok());
}
