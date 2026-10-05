//! The batch: a plugin's records, and the checks a record must clear before it is stored.
//!
//! A batch is two arrays — upserts and deletes — applied together or not at all. The
//! reader's job is to turn the wire text into [`Batch`] with every member named and
//! every id checked, doing nothing else: no cell is looked at until [`Batch::check`],
//! because a cell is only knowable against the manifest, and the manifest is a *separate*
//! versioned declaration that may be newer than the batch's own view of the world.
//!
//! Two rules live in the reader and two in `check`, and which is which is deliberate:
//!
//! | Rule | Where | Why there |
//! |---|---|---|
//! | member names, types | reader | the wire shape, decidable from the body alone |
//! | id grammar, no NUL, size | reader | properties of the bytes, before the shape |
//! | a record at most once | reader | a property of the batch as a set |
//! | the cell fits its declared role | [`Batch::check`] | only the manifest knows a role |
//! | the record's canonical text fits `max_record_bytes` | [`Batch::check`] | needs the written record |

use super::manifest::Manifest;
use super::strict::parse_strict;
use super::{
    HubError, Limits, breaks, check_collection_id, check_record_id, qualify,
};
use crate::canonical_json::Value;
use crate::ingest::read::{array, integer, member, object, require_only, text_of};
use crate::ingest::{JsonValue, Record, cell, record_piece};

mod cells;

/// The members each object names, exactly. An unknown member is refused at every level:
/// a stray camelCase is a mistake, and at this level it is a mistake in the *write* path,
/// where accepting it would store data no later read can return.
const UPSERT_MEMBERS: [&str; 4] = ["collection", "id", "updatedAt", "values"];
const DELETE_MEMBERS: [&str; 2] = ["collection", "id"];

/// One record to store: its identity, its collection, and its cells keyed by field id.
#[derive(Debug, Clone, PartialEq)]
pub struct Upsert {
    /// The collection's id, **unqualified** as the client wrote it: the reader checks the
    /// grammar and refuses a `plugin.coll`, and [`Upsert::record`] qualifies it from the
    /// plugin the batch was sent to.
    pub collection: String,
    /// The record's id within that collection: not empty, no `:` (H5).
    pub id: String,
    /// The client's own version of this record, a `u32` (D6: never `usize` on the wire).
    pub updated_at: u32,
    /// Its cells keyed by field id, **sorted by key**, whatever order they arrived in.
    pub values: Vec<(String, JsonValue)>,
}

/// One record to remove.
#[derive(Debug, Clone, PartialEq)]
pub struct Delete {
    /// The collection's id, unqualified, as in [`Upsert::collection`].
    pub collection: String,
    /// The record's id within that collection.
    pub id: String,
}

/// A whole batch: what to store and what to remove, applied together.
#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    /// The upserts, in document order.
    pub upserts: Vec<Upsert>,
    /// The deletes, in document order.
    pub deletes: Vec<Delete>,
}

impl Upsert {
    /// The [`Record`] this upsert stores, for `plugin`: the collection qualified, not
    /// deleted. `updated_at` is the client's own version — the `rev` the store assigns is
    /// a different number and is not in the batch.
    pub fn record(&self, plugin: &str) -> Record {
        Record {
            id: self.id.clone(),
            collection: qualify(plugin, &self.collection),
            deleted: false,
            updated_at: self.updated_at,
            values: self.values.clone(),
        }
    }
}

/// Reads one batch body, or the refusal. `limits` supplies the two caps decided from the
/// bytes: `max_body` and `max_batch`.
pub fn read_batch(text: &str, limits: &Limits) -> Result<Batch, HubError> {
    let root = parse_strict(text, limits.max_body, "body")?;
    let members = object(&root, "").map_err(HubError::Shape)?;
    require_only(members, &["upserts", "deletes"], "").map_err(HubError::Shape)?;
    let mut upserts = upserts(member(members, "upserts", "").map_err(HubError::Shape)?)?;
    let deletes = deletes(member(members, "deletes", "").map_err(HubError::Shape)?)?;
    if (upserts.len() + deletes.len()) as u64 > limits.max_batch {
        return Err(HubError::TooLarge {
            what: "batch",
            limit: limits.max_batch,
        });
    }
    check_once(&upserts, &deletes)?;
    upserts.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.collection.cmp(&b.collection)));
    Ok(Batch { upserts, deletes })
}

impl Batch {
    /// Every check a record needs against the manifest it will be served from, and the two
    /// caps that need a written record. Every cell is checked *before* anything is stored:
    /// a batch is atomic, so one bad record changes nothing.
    ///
    /// The collection check is per operation, not per cell: a record naming a collection
    /// the manifest does not declare has no fields to check its cells against.
    pub fn check(&self, plugin: &str, manifest: &Manifest, limits: &Limits) -> Result<(), HubError> {
        for (i, up) in self.upserts.iter().enumerate() {
            let path = format!("upserts[{i}]");
            let collection = cells::declared(manifest, plugin, &up.collection, &path)?;
            for (field_id, value) in &up.values {
                cells::check_cell(collection, field_id, value, &path)?;
            }
            if record_piece(&up.record(plugin)).len() as u64 > limits.max_record_bytes {
                return Err(HubError::TooLarge {
                    what: "record",
                    limit: limits.max_record_bytes,
                });
            }
        }
        for (i, delete) in self.deletes.iter().enumerate() {
            let path = format!("deletes[{i}]");
            cells::declared(manifest, plugin, &delete.collection, &path)?;
        }
        Ok(())
    }
}

/// The `upserts` array read. Values sorted by key on the way in, for the reason
/// `ingest/read.rs` sorts a record's: the cell order a client wrote is not a fact the
/// store may keep.
fn upserts(value: &Value) -> Result<Vec<Upsert>, HubError> {
    array(value, "upserts")
        .map_err(HubError::Shape)?
        .iter()
        .enumerate()
        .map(|(i, item)| upsert(item, &format!("upserts[{i}]")))
        .collect()
}

fn upsert(value: &Value, path: &str) -> Result<Upsert, HubError> {
    let members = object(value, path).map_err(HubError::Shape)?;
    require_only(members, &UPSERT_MEMBERS, path).map_err(HubError::Shape)?;
    let id = record_id(members, path)?;
    let collection = collection_id(members, path)?;
    let mut values: Vec<(String, JsonValue)> = object(
        member(members, "values", path).map_err(HubError::Shape)?,
        &format!("{path}.values"),
    )
    .map_err(HubError::Shape)?
    .iter()
    .map(|(key, value)| {
        cell(value, &format!("{path}.values.{key}"))
            .map(|value| (key.clone(), value))
            .map_err(HubError::Shape)
    })
    .collect::<Result<_, _>>()?;
    values.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(Upsert {
        collection,
        id,
        updated_at: integer(
            member(members, "updatedAt", path).map_err(HubError::Shape)?,
            &format!("{path}.updatedAt"),
        )
        .map_err(HubError::Shape)?,
        values,
    })
}

fn deletes(value: &Value) -> Result<Vec<Delete>, HubError> {
    array(value, "deletes")
        .map_err(HubError::Shape)?
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let path = format!("deletes[{i}]");
            let members = object(item, &path).map_err(HubError::Shape)?;
            require_only(members, &DELETE_MEMBERS, &path).map_err(HubError::Shape)?;
            Ok(Delete {
                collection: collection_id(members, &path)?,
                id: record_id(members, &path)?,
            })
        })
        .collect()
}

/// A collection id from a batch object: the grammar, and — unless the reader is
/// deliberately lax — **no dot**, because a batch is the write path and the store
/// qualifies every collection from the plugin itself. A qualified id here would be
/// qualified twice.
fn collection_id(members: &[(String, Value)], path: &str) -> Result<String, HubError> {
    let at = format!("{path}.collection");
    let text = text_of(
        member(members, "collection", path).map_err(HubError::Shape)?,
        &at,
    )
    .map_err(HubError::Shape)?
    .to_string();
    if !breaks::on("lax-reader") {
        check_collection_id(&text).map_err(|e| on_id(e, &at))?;
    }
    Ok(text)
}

/// A record id from a batch object. The `:` refusal keeps H5's reason — a colon in the
/// last coordinate of a node id parses back shifted — because a hub record *is* a node.
fn record_id(members: &[(String, Value)], path: &str) -> Result<String, HubError> {
    let at = format!("{path}.id");
    let text = text_of(member(members, "id", path).map_err(HubError::Shape)?, &at)
        .map_err(HubError::Shape)?
        .to_string();
    check_record_id(&text).map_err(|e| on_id(e, &at))?;
    Ok(text)
}

/// An id refusal lands on the object's own path, so the message names where to look. The
/// `:` case keeps H5's wording because it explains *why*, which a bare "not a legal id"
/// would not.
fn on_id(error: HubError, path: &str) -> HubError {
    match error {
        HubError::Grammar { coordinate, value } if value.contains(':') => HubError::Invalid {
            path: path.to_owned(),
            what: format!(
                "{coordinate} {value:?} contains `:`, which cannot round-trip through the \
                 node-id grammar"
            ),
        },
        HubError::Grammar { coordinate, value } => HubError::Invalid {
            path: path.to_owned(),
            what: format!("{coordinate} {value:?} is not a legal id"),
        },
        other => other,
    }
}

/// A record named at most once across both arrays. Checked after the size cap, so a batch
/// that is too big is refused as too big rather than as a repeat.
fn check_once(upserts: &[Upsert], deletes: &[Delete]) -> Result<(), HubError> {
    for (i, up) in upserts.iter().enumerate() {
        if upserts[..i].iter().any(|o| o.id == up.id && o.collection == up.collection) {
            return Err(repeated(&up.collection, &up.id));
        }
        if deletes.iter().any(|d| d.id == up.id && d.collection == up.collection) {
            return Err(repeated(&up.collection, &up.id));
        }
    }
    for (i, delete) in deletes.iter().enumerate() {
        if deletes[..i].iter().any(|d| d.id == delete.id && d.collection == delete.collection) {
            return Err(repeated(&delete.collection, &delete.id));
        }
    }
    Ok(())
}

fn repeated(collection: &str, id: &str) -> HubError {
    HubError::Invalid {
        path: String::new(),
        what: format!("the batch: record `{collection}`/`{id}` appears more than once"),
    }
}

