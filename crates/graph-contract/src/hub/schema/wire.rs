//! The mirror types: the batch, the change, the notice, the answer and the error, as
//! schemars can describe them.
//!
//! Split out of `schema.rs` by the house's 300-line limit, and because they are a different
//! kind of thing: `schema.rs` is the schema *entry point* and the two version members,
//! these are the payload shapes. Every one of them is a **mirror**, never the reader's own
//! type — `batch::Upsert` holds a `JsonValue`, which is a closed Rust enum this crate
//! publishes no schema for, so re-using it would put a `$ref` in the committed file that
//! resolves to nothing.

use super::bounded;
use crate::ingest::schema::AnyValue;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One batch, as the wire spells it: the records to store and the records to remove.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BatchWire {
    /// The records to remove, as `(collection, id)` pairs with nothing else.
    pub deletes: Vec<DeleteWire>,
    /// The records to store.
    pub upserts: Vec<UpsertWire>,
}

/// One record to remove: its identity and its collection, and nothing else. A delete has
/// no cells and no version — a record the store does not have is not an error, so the wire
/// carries nothing a store would have to reconcile.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DeleteWire {
    /// The collection's id, **unqualified**: the hub qualifies it from the plugin.
    pub collection: String,
    /// The record's id within that collection.
    pub id: String,
}

/// One record to store.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpsertWire {
    /// The collection's id, **unqualified**: a batch is the write path and the hub
    /// qualifies every collection itself, so a qualified one here would be qualified twice.
    pub collection: String,
    /// The record's id within that collection. Not empty, and no `:`.
    pub id: String,
    /// The client's own version of this record. `u32`, never `usize` on the wire (D6).
    pub updated_at: u32,
    /// Its cells keyed by field id. The map is keyed by id rather than positional, so a
    /// reserialized document means the same thing (H6).
    pub values: BTreeMap<String, AnyValue>,
}

/// One change entry: a batch's worth of records, or a manifest's new declaration.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ChangeWire {
    /// When the change was applied, already in the wire's own timestamp spelling — the
    /// contract has no clock (D2).
    pub at: String,
    /// What this change carries.
    pub kind: ChangeKind,
    /// The removed records, each with the revision it had. Present only on a `batch`.
    pub deletes: Option<Vec<StoredDelete>>,
    /// The new declaration. Present only on a `manifest` change.
    pub manifest: Option<super::ManifestWire>,
    /// The plugin the change is for.
    pub plugin: String,
    /// Where the stream is after this change.
    #[schemars(schema_with = "bounded")]
    pub seq: u64,
    /// The stored records. Present only on a `batch` change.
    pub upserts: Option<Vec<StoredRecord>>,
}

/// What a change carries. The two variants are the whole taxonomy of the stream, so a
/// client can switch on this member and be sure it has seen every shape.
#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    /// Records stored and removed.
    Batch,
    /// A plugin's declaration changed.
    Manifest,
}

/// One stored record inside a change: the record the ingest contract writes, plus the hub's
/// own revision.
///
/// `rev` is here and not in the record because it is not a fact about the *record*: it is a
/// fact about the hub's history of it, and two hubs storing the same record assign
/// different ones.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StoredRecord {
    /// The record's collection, qualified.
    pub collection: String,
    /// Whether the source has deleted it.
    pub deleted: bool,
    /// The record's id within that collection.
    pub id: String,
    /// The hub's revision for this record. 1 when it was created.
    #[schemars(schema_with = "bounded")]
    pub rev: u64,
    /// The client's own version of this record.
    pub updated_at: u32,
    /// Its cells keyed by field id.
    pub values: BTreeMap<String, AnyValue>,
}

/// One removed record inside a change: its identity and the revision it had, which is the
/// number a client compares against what it last saw.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StoredDelete {
    /// The collection's id, qualified.
    pub collection: String,
    /// The record's id within that collection.
    pub id: String,
    /// The revision the record had when it was removed.
    #[schemars(schema_with = "bounded")]
    pub rev: u64,
}

/// A change-stream position with nothing in it.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NoticeWire {
    /// When the change was applied.
    pub at: String,
    /// Always `notice`. A member and not a constant because the wire writes every change
    /// envelope the same shape, and a client switching on one member should not need a
    /// special case for the empty one.
    pub kind: String,
    /// The plugin the notice is for.
    pub plugin: String,
    /// Where the stream is.
    #[schemars(schema_with = "bounded")]
    pub seq: u64,
}

/// The answer to an applied batch: where the stream is, and how much of the batch actually
/// changed something.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AnswerWire {
    /// How many of the batch's operations changed something. An idempotent re-send answers
    /// `0`, which is a different fact from "the batch failed".
    pub applied: u32,
    /// Where the stream is.
    #[schemars(schema_with = "bounded")]
    pub seq: u64,
}

/// The service's error envelope. The contract fixes the *class* of each refusal, not the
/// code string: the codes belong to `docs/contract/service-api.md`, and a client that
/// switched on them would be coupled to the service's wording.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ErrorWire {
    /// The class: `invalid`, `conflict`, `too_large`, `cursor`, `internal`.
    pub error: String,
    /// A human-readable reason, naming the path or the coordinate at fault.
    pub message: String,
}
