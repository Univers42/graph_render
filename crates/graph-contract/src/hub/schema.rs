//! The hub wire contract's own JSON Schema: the mirror of the wire types.
//!
//! Same arrangement as `ingest/schema.rs` and for the same reason: the contract is
//! dependency-free so graph-core can carry it, and `serde` is behind the `codegen` feature.
//! Re-declaring the shape here is what lets `docs/contract/hub-schema.json` be generated
//! from Rust rather than trusted — and a client SDK generated from the same types cannot
//! drift from the reader that enforces them.
//!
//! Two things the ingest schema does not have to do:
//!
//! - **Wire integers are bounded.** `seq` and `rev` stop at `2^53 − 1`, so a JSON consumer
//!   reading them as a double cannot round them. The schema says so with a `maximum`, which
//!   is the only place a client learns it without reading the prose.
//! - **The change is a discriminated shape.** `kind` is `batch` or `manifest`, and each
//!   carries a different payload, so `upserts`/`deletes` are present only on a batch and
//!   `manifest` only on a manifest. `Option` is how a JSON Schema says "may be absent" —
//!   the wire never sends a member it does not mean, so an absent member is not an error.
//!
//! Every member is refused if unknown, exactly as the ingest schema does it: a schema that
//! admitted what the reader refuses would validate a document the motor then rejects, and
//! the two disagreeing about the same file is the worst place for them to.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;
mod wire;

pub use wire::{
    AnswerWire, BatchWire, ChangeKind, ChangeWire, DeleteWire, ErrorWire, NoticeWire, StoredDelete,
    StoredRecord, UpsertWire,
};

/// The hub wire, all of it: what a client may send and what a hub may answer.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HubWire {
    /// A plugin's declaration of its collections.
    pub manifest: ManifestWire,
    /// A plugin's records to store and remove.
    pub batch: BatchWire,
    /// One entry of the change stream.
    pub change: ChangeWire,
    /// A change-stream position with nothing in it.
    pub notice: NoticeWire,
    /// The answer to an applied batch.
    pub answer: AnswerWire,
    /// The error envelope the *service* writes; the contract fixes only the class.
    pub error: ErrorWire,
}

/// The JSON Schema (draft 2020-12) of the hub wire.
pub fn schema() -> serde_json::Value {
    let mut schema = schemars::schema_for!(HubWire).to_value();
    schema["title"] = serde_json::json!("HubWire");
    schema["description"] = serde_json::json!(
        "The graph-motor hub wire: a manifest declares what a plugin's collections mean, \
         a batch stores and removes records, and a change stream reports what was done. \
         Every object refuses an unknown member, and every document is written in \
         canonical order — object keys sorted by bytes, collections by qualified id, \
         records by (qualified collection, id) — so two hubs that stored the same change \
         agree byte for byte about what it was."
    );
    schema
}

/// The wire's upper bound on a `seq` or a `rev`: the last integer a JSON consumer holds
/// exactly. Past it a double would round the value, and a cursor that rounds is a cursor
/// that skips or repeats a change.
pub const MAX_WIRE_INT: u64 = (1 << 53) - 1;

/// A `u64` that must survive a JSON round trip through a double.
pub(crate) fn bounded(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "integer",
        "format": "uint64",
        "minimum": 0,
        "maximum": MAX_WIRE_INT,
    })
}

/// The manifest, as the wire spells it — see `Manifest` for what the reader holds.
///
/// `manifestVersion` is the client's publication counter and `version` is this wire
/// format's. Both are in the schema because a client that confuses them refuses its own
/// second manifest, and the schema is where a client learns they are different facts.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ManifestWire {
    /// The declared collections.
    pub collections: Vec<crate::ingest::schema::Collection>,
    /// The client's own publication counter; what growth compares.
    pub manifest_version: u32,
    /// Human name, for diagnostics only.
    pub name: String,
    /// This wire format's version. 1 is the only one defined.
    #[schemars(schema_with = "wire_version_schema")]
    pub version: u32,
}

/// The wire's own version, as a `const` so a manifest claiming anything else fails to
/// validate rather than passing a range check.
fn wire_version_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "integer",
        "format": "uint32",
        "const": super::VERSION,
        "description": "The hub wire format's version. 1 is the only one this contract \
            defines; a client that reads another must not guess what changed.",
    })
}
