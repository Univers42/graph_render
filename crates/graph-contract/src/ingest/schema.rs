//! The ingest contract's shape as Rust types, for `docs/contract/ingest-schema.json`.
//! They describe; they do not read or write — [`super::read()`] and [`super::to_json`]
//! do. The tests hold the two together: `write_then_read_gives_back_the_same_document`
//! reads back what the writer writes, and the schema's own tests check the committed
//! file against the contract's rules rather than against this file.
//!
//! The types are a *mirror*, not the contract's own, for one reason: the contract is
//! dependency-free so `graph-core` can read it, and `serde` is behind the `codegen`
//! feature. Re-declaring the shape here is what lets the schema be generated from Rust
//! without a serializer leaking into the motor's allow-list — the arrangement
//! `canonical_json::schema` already uses, and the reason its file is committed and
//! diffed rather than trusted.
//!
//! Three members cannot come from a derive and are written by hand, each with the
//! reason at its definition: the two closed enums (whose authority is `Role::as_str`,
//! not a second list), the document's version (a closed set of one), and a cell value.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;

use super::{Cardinality, Role, VERSION};

mod enums;

/// A whole ingest document.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ingest {
    /// The declared collections, in document order.
    pub collections: Vec<Collection>,
    /// The records, in document order.
    pub records: Vec<Record>,
    /// The backend the records came from; the first coordinate of every node id. May
    /// not contain `:` (H5).
    pub source: String,
    /// The document's version. 1 is the only one this reader accepts, and the schema
    /// says exactly that rather than a range a future version would widen.
    #[schemars(schema_with = "version_schema")]
    pub version: u32,
}

/// The version member: the literal [`VERSION`], as a `const` so a document claiming
/// anything else fails to validate rather than passing a range check.
fn version_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "integer",
        "format": "uint32",
        "const": VERSION,
        "description": "The document's version. A reader refuses a version it does not \
            know; 1 is the only one defined.",
    })
}

/// A cell value: any JSON.
///
/// `serde_json::Value`'s own `JsonSchema` is an opaque `$ref` to a def this crate does
/// not publish, which would leave the committed schema with a dangling reference and so
/// unable to validate anything. This transparent wrapper carries the open schema
/// instead, under the name the contract's own type has — the contract constrains the
/// envelope and never the payload.
#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AnyValue(pub serde_json::Value);

impl JsonSchema for AnyValue {
    fn schema_name() -> Cow<'static, str> {
        "JsonValue".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "Any JSON value. The contract constrains the envelope and \
                never the payload: a source's own cell types survive it untouched, so \
                what a `title` must hold is a role the declaration states, not a JSON \
                type here.",
        })
    }
}

/// A collection of records and the roles of its fields.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Collection {
    /// Its declared fields, in canonical order (sorted by field id).
    pub fields: Vec<Field>,
    /// Stable id, unique in the document; the middle coordinate of a node id. May not
    /// contain `:` (H5).
    pub id: String,
    /// Human name, for diagnostics only.
    pub name: String,
    /// Id of the field whose value is a record's label. That field's role is `title`.
    pub title_field: String,
}

/// One declared field of a collection.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Field {
    /// Stable id, unique within its collection; the key a record's `values` uses.
    pub id: String,
    /// What a `link` field points at; `null` for every other role. Required in the
    /// schema even though it is nullable, so "no link" is stated rather than omitted —
    /// see `required_link`, which is what puts it there.
    pub link: Option<Link>,
    /// Human name, for diagnostics only.
    pub name: String,
    /// What the field means. Declared, never inferred.
    pub role: Role,
}

/// What a `link` field points at.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Link {
    /// How many references the field may hold.
    pub cardinality: Cardinality,
    /// Id of the collection the references name.
    pub collection: String,
    /// Whether the relation is undirected: `A → B` and `B → A` are one edge, drawn
    /// without an arrowhead.
    pub symmetric: bool,
}

/// One record: its identity, its collection, and its values keyed by field id.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Record {
    /// The collection this record belongs to.
    pub collection: String,
    /// Whether the source has deleted it. A deleted record derives nothing at all.
    pub deleted: bool,
    /// Stable id within its collection; the last coordinate of a node id. May contain
    /// `:` — it is the last segment, so it round-trips.
    pub id: String,
    /// Source-assigned version.
    pub updated_at: u32,
    /// Its cells, keyed by field id. A `BTreeMap`, not a `HashMap`: the schema is
    /// generated from these types and must not depend on a random seed (D4).
    pub values: BTreeMap<String, AnyValue>,
}

/// The JSON Schema (draft 2020-12) of one ingest document.
pub fn schema() -> serde_json::Value {
    let mut schema = schemars::schema_for!(Ingest).to_value();
    required_link(&mut schema);
    schema["title"] = serde_json::json!("Ingest");
    schema["description"] = serde_json::json!(
        "The graph-motor ingest contract: declared roles and records, the neutral front \
         of the pipeline. Written by every adapter, read by one derivation. Every \
         object refuses an unknown member, and every object is written in canonical \
         order — object keys sorted by bytes, a collection's fields sorted by field id, \
         a record's cells sorted by field id — so a document's bytes depend on what it \
         declares and not on the order the source listed it in."
    );
    schema
}

/// `Option<T>` is optional to schemars for the same reason it is optional to serde —
/// a missing member deserializes to `None` — and `link` must be **present** even when
/// it is `null`. The reader refuses a missing one, so a schema that admitted it would
/// validate a document the motor then rejects: the schema and the reader disagreeing
/// about the same file is the worst place for them to. serde has no attribute for
/// "present but nullable", so this is the one place that mismatch is corrected.
///
/// It runs over the whole `$defs` rather than naming `Field`, because schemars inlines
/// a type used once (`Link` here) and `$defs` membership is its decision, not ours.
fn required_link(schema: &mut serde_json::Value) {
    let Some(field) = schema["$defs"]["Field"]
        .get_mut("required")
        .and_then(|r| r.as_array_mut())
    else {
        return;
    };
    if !field.iter().any(|v| v == "link") {
        field.push(serde_json::json!("link"));
    }
    field.sort_by(|a, b| {
        a.as_str()
            .unwrap_or_default()
            .cmp(b.as_str().unwrap_or_default())
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_is_pinned_to_the_one_the_reader_accepts() {
        assert_eq!(schema()["properties"]["version"]["const"], VERSION);
        assert_eq!(schema()["properties"]["version"]["const"], 1);
    }

    #[test]
    fn every_role_and_cardinality_is_a_one_of_const_named_as_its_as_str() {
        let schema = schema();
        let roles = schema["$defs"]["Role"]["oneOf"].as_array().unwrap();
        assert_eq!(roles.len(), Role::ALL.len());
        for (role, def) in Role::ALL.iter().zip(roles) {
            assert_eq!(def["const"], role.as_str(), "{role:?}");
        }
        let cards = schema["$defs"]["Cardinality"]["oneOf"].as_array().unwrap();
        assert_eq!(cards.len(), Cardinality::ALL.len());
        for (card, def) in Cardinality::ALL.iter().zip(cards) {
            assert_eq!(def["const"], card.as_str(), "{card:?}");
        }
    }

    #[test]
    fn the_committed_schema_contains_no_dangling_reference() {
        // Every `$ref` the file carries must resolve inside it: a `$ref` to a def this
        // crate does not publish is a schema that cannot be used to validate anything.
        let text = schema().to_string();
        for name in [
            "Ingest",
            "Collection",
            "Field",
            "Link",
            "Record",
            "Role",
            "Cardinality",
        ] {
            let marker = format!("\"#/$defs/{name}\"");
            if text.contains(&marker) {
                assert!(
                    schema()["$defs"].get(name).is_some(),
                    "{name} is referenced but not defined"
                );
            }
        }
        assert!(!text.contains("io::"), "an external schema URI leaked in");
    }
}
