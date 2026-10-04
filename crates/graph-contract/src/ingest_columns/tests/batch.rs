//! `GMX1`, the extend batch: the same sections as `GMC1` with one rule changed. The
//! fixtures are `full()`'s bytes with the magic word swapped — the sections, the counts and
//! the offsets are identical, which is the claim the format makes and the reason the encoder
//! is shared.
//!
//! Every test here pins the *difference*: what `GMX1` reads that `GMC1` cannot, and the two
//! directions of "each reader refuses the other's bytes".

use super::*;

/// `full()` read as a batch. Its two endpoints are cells `0` and `1`, which are the entries
/// of `"n-0"` and `"n-1"` — so the same bytes are a legal batch *and* a legal document, and
/// only the magic tells them apart.
fn batch() -> Encoded {
    let mut encoded = full().encode();
    encoded.patch_header(0, layout::BATCH_MAGIC);
    encoded
}

/// The refusal `batch()` earns after `patch`, as `refusals::refused` for the batch reader.
fn refused(patch: impl FnOnce(&mut Encoded)) -> ColumnsError {
    let mut encoded = batch();
    patch(&mut encoded);
    decode_batch(&encoded.bytes).expect_err("this batch is refused")
}

#[test]
fn a_batch_reads_the_same_sections_as_a_document() {
    let bytes = batch().bytes;
    let doc = decode_batch(&bytes).expect("a batch is a batch");
    assert_eq!((doc.node_count(), doc.edge_count()), (2, 1));
    assert_eq!(doc.format(), Format::Batch);
    let node = doc.node(0).expect("row 0");
    assert_eq!(
        (node.id, node.kind, node.source),
        ("n-0", "record", "studio")
    );
    assert_eq!(node.weight.to_bits(), (-0.0f64).to_bits());
    assert_eq!(node.version.to_bits(), f64::from_bits(1).to_bits());
}

#[test]
fn a_batch_endpoint_is_a_string_entry_naming_a_node_id() {
    let bytes = batch().bytes;
    let doc = decode_batch(&bytes).expect("a batch is a batch");
    let edge = doc.edge(0).expect("row 0");
    assert_eq!((edge.source_row, edge.target_row), (0, 1));
    // The endpoint cells are entries, so what they *name* is the text behind them.
    assert_eq!(doc.text(edge.source_row), Some("n-0"));
    assert_eq!(doc.text(edge.target_row), Some("n-1"));
}

#[test]
fn each_reader_refuses_the_other_formats_magic() {
    assert_eq!(
        decode(&batch().bytes).expect_err("a batch is not a document"),
        ColumnsError::BadMagic {
            found: layout::BATCH_MAGIC
        }
    );
    assert_eq!(
        decode_batch(&full().encode().bytes).expect_err("a document is not a batch"),
        ColumnsError::BadMagic {
            found: layout::MAGIC
        }
    );
}

#[test]
fn a_document_endpoint_must_be_a_row_and_a_batch_endpoint_must_be_an_entry() {
    // One cell, two rules: entry 5 is past the document's two node rows, and a perfectly
    // ordinary string index in a batch. If the two arms of `endpoint` were one rule, one of
    // these two refusals below would be the wrong error.
    let mut as_document = full().encode();
    as_document.patch_u32(as_document.marks.edge_source, 0, 5);
    assert_eq!(
        decode(&as_document.bytes).expect_err("row 5 is past two nodes"),
        ColumnsError::EndpointRow {
            column: "edge source",
            row: 0
        }
    );
    let mut as_batch = batch();
    as_batch.patch_u32(as_batch.marks.edge_source, 0, 5);
    assert_eq!(
        decode_batch(&as_batch.bytes).expect("entry 5 names a string").format(),
        Format::Batch
    );
}

#[test]
fn a_batch_endpoint_names_no_string_and_is_refused() {
    let past = full().string_count();
    assert_eq!(
        refused(|e| e.patch_u32(e.marks.edge_target, 0, past)),
        ColumnsError::StringIndex {
            column: "edge target",
            row: 0
        }
    );
}

#[test]
fn a_batch_endpoint_is_required_and_refuses_the_absent_marker() {
    assert_eq!(
        refused(|e| e.patch_u32(e.marks.edge_source, 0, ABSENT)),
        ColumnsError::RequiredAbsent {
            column: "edge source",
            row: 0
        }
    );
}

/// The one rule a batch shares with a document: the rest of the header is refused the same
/// way, so the format is a magic word and not a second contract.
#[test]
fn a_batch_shares_the_documents_every_other_refusal() {
    assert_eq!(
        refused(|e| e.patch_header(1, 2)),
        ColumnsError::BadVersion { found: 2 }
    );
    assert_eq!(
        refused(|e| e.patch_header(6, 1)),
        ColumnsError::NonZeroReserved { word: 1 }
    );
    let mut encoded = batch();
    encoded.patch_u32(encoded.marks.node_has_note, 0, 2);
    assert_eq!(
        decode_batch(&encoded.bytes).expect_err("2 is not a boolean"),
        ColumnsError::NotBoolean {
            column: "node has_note",
            row: 0,
            found: 2
        }
    );
    let mut encoded = batch();
    encoded.patch_f64(encoded.marks.strength, 0, f64::NAN);
    assert_eq!(
        decode_batch(&encoded.bytes).expect_err("NaN is not finite"),
        ColumnsError::NotFinite {
            column: "edge strength",
            row: 0
        }
    );
}