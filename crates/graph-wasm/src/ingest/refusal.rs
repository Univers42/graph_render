//! The ingest refusal: what [`super::read_records`] and [`super::index`] return instead of
//! records, and the wire code each is published under.

use graph_contract::canonical_json::JsonError;

use crate::errors::Code;

/// Why an ingest buffer was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestError {
    /// Not UTF-8.
    Utf8,
    /// Not JSON at all.
    Json(JsonError),
    /// JSON, but not this shape: dotted path and what was wrong.
    Shape(String),
    /// A duplicate node or edge id (C12: refused, not silently first-wins).
    DuplicateId {
        /// The list the id repeats in: `node` or `edge`.
        what: &'static str,
        /// The repeated id.
        id: String,
    },
    /// An edge naming a node id that is not in `nodes` (C12: refused, not dropped).
    DanglingEndpoint {
        /// The edge's id.
        edge: String,
        /// The end that dangles: `source` or `target`.
        end: &'static str,
        /// The node id it names.
        id: String,
    },
    /// Too many nodes or edges to index (`u32` capacity). Reachable only on a 64-bit host: on
    /// wasm32 `usize` is `u32` (F-79). A ceiling on the document's bytes is
    /// [`IngestError::TooLarge`], which is the one that bites on wasm32.
    Capacity,
    /// The buffer is longer than the ceiling, [`MAX_INGEST_BYTES`](super::MAX_INGEST_BYTES).
    TooLarge {
        /// The buffer's length in bytes.
        bytes: usize,
        /// The ceiling it was held to.
        limit: usize,
    },
}

impl IngestError {
    /// The wire code this refusal is published under (C4): every ingest refusal is
    /// [`Code::IngestInvalid`] but the one the host can do something about — an oversized
    /// document is not malformed, and telling a caller the two are the same would send it
    /// looking for a bad member in a document it must instead split.
    pub fn code(&self) -> Code {
        match self {
            Self::TooLarge { .. } => Code::IngestTooLarge,
            _ => Code::IngestInvalid,
        }
    }
}
