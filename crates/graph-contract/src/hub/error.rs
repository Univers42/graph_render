//! The one error every hub refusal is, and the HTTP class each one answers.
//!
//! Seven variants and no codes. A variant is *a reason*; the class it maps to is a
//! property of the reason, computed here so no server has to re-derive it and two
//! servers cannot disagree. The reason strings themselves belong to the service's error
//! envelope, not here: this crate fixes what is refused, the service fixes what it says.

use core::fmt;

use crate::ingest::IngestError;

/// Why a hub wire was refused. Every variant is a fact about the *request*; nothing
/// here is a fault of the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubError {
    /// The ingest reader refused: a JSON fault, or a shape this contract does not name.
    /// Kept whole rather than flattened, because the ingest reader's path-and-reason
    /// message is the more useful half of the answer.
    Shape(IngestError),
    /// A member is missing, unknown or the wrong type: `path` names it, `what` says why.
    Invalid {
        /// Dotted path of the offending value, e.g. `collections[0].id`.
        path: String,
        /// What was wrong with it.
        what: String,
    },
    /// An id does not match its grammar. `coordinate` is *which* id, not which value, so
    /// the message says "collection id" and never a path.
    Grammar {
        /// Which id: `workspace id`, `plugin id`, `collection id`, `record id`.
        coordinate: &'static str,
        /// The text that was refused.
        value: String,
    },
    /// A `\u0000` in a body — in a key as much as in a string, because a NUL in a key
    /// truncates a C string just as silently and reaches a store by the same route.
    Nul {
        /// Path of the string or key holding the NUL. Empty for one at the root.
        path: String,
    },
    /// Over a declared cap. `what` names the cap so the answer is a size the caller can
    /// act on rather than a shape it must guess at.
    TooLarge {
        /// The cap: `body`, `batch`, `manifest`, `record`, `change`, `collections`,
        /// `fields`, `plugins`, `seq`.
        what: &'static str,
        /// The cap itself, so a client can size itself without reading the spec.
        limit: u64,
    },
    /// A manifest that only grew, or a batch that must be atomic, refusing.
    Conflict {
        /// What conflicts: the old and new version, or the field that changed.
        what: String,
    },
    /// A cursor that is not a cursor.
    Cursor {
        /// The text that was refused, whole.
        text: String,
    },
}

impl HubError {
    /// The HTTP class this refusal answers with. The server owns the transport; this is
    /// the one place the mapping is written down, so `422` cannot become `400` in one
    /// handler and `413` cannot become `422` in another.
    pub fn status(&self) -> u16 {
        match self {
            Self::Shape(_) | Self::Invalid { .. } | Self::Grammar { .. } | Self::Nul { .. } => 422,
            Self::TooLarge { .. } => 413,
            Self::Conflict { .. } => 409,
            Self::Cursor { .. } => 400,
        }
    }
}

impl fmt::Display for HubError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape(err) => write!(f, "{err}"),
            Self::Invalid { path, what } if path.is_empty() => write!(f, "the body: {what}"),
            Self::Invalid { path, what } => write!(f, "{path}: {what}"),
            Self::Grammar { coordinate, value } => {
                write!(f, "{coordinate} {value:?} is not a legal id")
            }
            Self::Nul { path } if path.is_empty() => {
                write!(f, "the body holds a NUL character")
            }
            Self::Nul { path } => write!(f, "{path}: a NUL character"),
            Self::TooLarge { what, limit } => write!(f, "over the {what} limit of {limit}"),
            Self::Conflict { what } => write!(f, "{what}"),
            Self::Cursor { text } => write!(f, "{text:?} is not a cursor `<epoch>.<seq>`"),
        }
    }
}