//! The canonical document as pieces, so a store can stream it without building the
//! [`Ingest`](crate::ingest::Ingest).
//!
//! `to_json` is the same concatenation written in one function; splitting it here means
//! the head, the middle, the tail and the separator count are *named*, so a hub that
//! serves a document as it stores it can compute its length before it has read any
//! record ([`frame_bytes`], spec §6's `doc_bytes`) and never hold the whole string.
//!
//! The pieces are the writer's own text, not a second format: `ingest/tests/pieces.rs`
//! pins the concatenation against `to_json` on every committed fixture, so the two
//! cannot drift.

use super::super::{Collection, Record, VERSION};
use super::{collection, quoted, record};

/// The bytes before the first collection.
pub const DOC_HEAD: &str = "{\"collections\":[";

/// The bytes between the last collection and the first record.
pub const DOC_MIDDLE: &str = "],\"records\":[";

/// The bytes between two collections or two records.
pub const DOC_SEPARATOR: &str = ",";

/// The bytes after the last record, trailing newline included.
pub fn doc_tail(source: &str) -> String {
    format!("],\"source\":{},\"version\":{VERSION}}}\n", quoted(source))
}

/// One collection's canonical text.
pub fn collection_piece(c: &Collection) -> String {
    collection(c)
}

/// One record's canonical text.
pub fn record_piece(r: &Record) -> String {
    record(r)
}

/// The document's bytes that are not a piece: head, middle, tail and separators.
///
/// `collections` and `records` are counts, so a caller that has not read the document
/// yet cannot use this; a caller that has holds the pieces' own lengths, and the two
/// sum to the document's exact length.
pub fn frame_bytes(source: &str, collections: u64, records: u64) -> u64 {
    let separators = collections.saturating_sub(1) + records.saturating_sub(1);
    (DOC_HEAD.len() + DOC_MIDDLE.len() + doc_tail(source).len()) as u64 + separators
}
