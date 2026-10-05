//! The hub wire contract: ids, limits, cursors and one error for all of it.
//!
//! A hub is a store of records kept on behalf of one plugin inside one workspace, and
//! this module is everything about that wire *except* the documents themselves: the
//! grammar every id obeys, the caps a body is refused past, the cursor that says "from
//! here", and the seven refusals — one per reason a hub says no, so a server can map a
//! reason to a status without reading a message.
//!
//! Nothing here reads or writes a document. The readers reuse the ingest ones
//! (`ingest/read.rs`, `validate.rs`), widened to `pub(crate)`: two readers for one set
//! of rules cannot both be right, so there is one set and this module names what it is
//! checking.
//!
//! ## The refusals, and what they cost the caller
//!
//! | Variant | Status | Why this is not something else |
//! |---|---|---|
//! | [`HubError::Shape`] | 422 | the ingest reader's own fault, re-wrapped so one `match` covers both levels |
//! | [`HubError::Invalid`] | 422 | a member is named and required and is missing, unknown or the wrong type |
//! | [`HubError::Grammar`] | 422 | an id does not match its grammar; the *server* owns the 400 for a path id |
//! | [`HubError::Nul`] | 422 | a `\u0000` anywhere in a body, key or string (spec: nothing else refuses it) |
//! | [`HubError::TooLarge`] | 413 | over a declared cap, so the answer is a size and not a shape |
//! | [`HubError::Conflict`] | 409 | manifests only grow, and a batch is atomic |
//! | [`HubError::Cursor`] | 400 | a cursor is not a cursor; only the server's address space turns this into a 400 |
//!
//! Error *codes* are deliberately absent: the strings belong to the service's own error
//! envelope (`docs/contract/service-api.md`), not to the contract that decides the class.

pub mod batch;
pub mod breaks;
pub mod change;
mod error;
mod ids;
pub mod manifest;
mod strict;

#[cfg(test)]
mod tests;

pub use change::{
    ChangeHead, answer_json, change_json, check_change, manifest_change_json, max_change,
    notice_json,
};
pub use error::HubError;
pub use ids::{
    Cursor, MAX_SEQ, check_collection_id, check_plugin_id, check_record_id, check_workspace_id,
    qualify,
};
pub use manifest::{Growth, Manifest, growth, manifest_json, read_manifest};

/// The only hub wire version this contract reads and writes.
pub const VERSION: u32 = 1;

/// Collections in one manifest.
pub const MAX_COLLECTIONS: u64 = 64;

/// Fields in one collection.
pub const MAX_FIELDS: u64 = 256;

/// Bytes in one manifest body.
pub const MAX_MANIFEST_BYTES: u64 = 262_144;

/// Plugins registered in one workspace.
pub const MAX_PLUGINS: u64 = 64;

/// The three size caps a request body can be refused past.
///
/// A `Limits` value rather than three constants so a server can answer a smaller body
/// without a second code path — the caps are the only numbers in the hub wire a caller
/// may want to change, and nothing else scales with a deployment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Bytes in one request body.
    pub max_body: u64,
    /// Operations (upserts plus deletes) in one batch.
    pub max_batch: u64,
    /// Bytes in one record's canonical text.
    pub max_record_bytes: u64,
}

impl Limits {
    /// The caps the spec fixes: 4 MiB, 10 000 operations, 1 MiB per record.
    pub const DEFAULT: Limits = Limits {
        max_body: 4 << 20,
        max_batch: 10_000,
        max_record_bytes: 1 << 20,
    };
}
