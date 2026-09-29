//! Why an ingest document was refused, and how a refusal reads.
//!
//! Split out of `ingest.rs` for the house's 300-line limit and for no other reason.
//! Every variant here is a *document* fact — a duplicate id, a dangling reference, a
//! coordinate that cannot round-trip, a shape the contract does not name — and the
//! point of each is the same: the alternative is a graph that is well-formed and
//! wrong, with nothing in the output to say so.

use core::fmt;

/// Why an ingest document was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestError {
    /// Not JSON at all, or JSON the strict reader refuses.
    Json(crate::canonical_json::JsonError),
    /// JSON, but not this shape: dotted path and what was wrong.
    Shape {
        /// Dotted path of the offending value, e.g. `collections[0].fields[2].role`.
        path: String,
        /// What was wrong with it.
        what: String,
    },
    /// A `source` or collection id that cannot round-trip through the node-id grammar
    /// (H5). Named, because "invalid id" without the coordinate is unactionable.
    IdGrammar {
        /// `source` or the collection id.
        coordinate: &'static str,
        /// The offending text.
        value: String,
    },
}

impl fmt::Display for IngestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => err.fmt(f),
            Self::Shape { path, what } if path.is_empty() => write!(f, "the document: {what}"),
            Self::Shape { path, what } => write!(f, "{path}: {what}"),
            Self::IdGrammar { coordinate, value } => write!(
                f,
                "{coordinate} {value:?} contains `:` and cannot round-trip through the \
                 node-id grammar (H5): the id would parse back shifted"
            ),
        }
    }
}
