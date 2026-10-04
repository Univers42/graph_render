//! The cells a `link` or `parent` field carries: every record id in one must name a record
//! the document declares.
//!
//! A child module for one reason — `validate.rs` sits at the house's 300-line limit and
//! this is the longest walk in it (every record, every cell it carries, every id in those
//! cells).

use super::{Collections, IngestError, Records, Role, shape, sorted_field};
use crate::ingest::{Collection, Field, Ingest, JsonValue, Record};

/// Every `link` and `parent` cell names records of **one** collection: a `link` field's
/// declared `link.collection`, or the record's own collection for a `parent` field, which
/// declares no link. Nothing else in the reader can see this: the parser reads a cell as
/// JSON, and `check_link_targets` only checks that a `link` field's declared collection
/// *exists* — never that the ids inside the cells name records of it. So a cell naming
/// `ghost` derived a `relation` edge to `source:collection:ghost`, a node no record
/// defines, with nothing in the output to say so.
///
/// **Deleted records are in the universe, deliberately.** A reference to a deleted record
/// is legitimate data — the derivation is what skips a deleted *target* (review finding
/// F-106) — and a reader that refused the document saying so would reject documents a
/// real source produced, and would be stricter than the derivation it feeds.
///
/// Runs last, after `check_references`, because it reads a record's collection and that
/// collection's fields, which `check_references` has already established exist. A document
/// wrong in both ways is refused as the undeclared collection or cell key first — the fact
/// nearer the top of the document, and the one that makes the later walk meaningless.
pub(super) fn check_link_cells(
    doc: &Ingest,
    collections: &Collections<'_>,
    records: &Records<'_>,
) -> Result<(), IngestError> {
    for (i, record) in doc.records.iter().enumerate() {
        let Some(collection) = collections.get(record.collection.as_str()) else {
            continue;
        };
        check_record(record, collection, records, i)?;
    }
    Ok(())
}

/// One record's reference cells, walked in key order. `read` sorted the cells by key and
/// `check` sorted the fields by id, so this meets the reference fields in the order the
/// field list holds them and the refusal is the first bad cell by field id, as it was when
/// the walk went field by field; it costs one search per cell the record carries instead
/// of one scan of the cells per field the collection declares.
fn check_record(
    record: &Record,
    collection: &Collection,
    records: &Records<'_>,
    index: usize,
) -> Result<(), IngestError> {
    for (field_id, cell) in &record.values {
        let Some(target) = sorted_field(collection, field_id).and_then(|f| target_of(record, f))
        else {
            continue;
        };
        let mut ids = referenced_ids(cell).into_iter();
        if let Some(id) = ids.find(|id| !records.contains(&(target, *id))) {
            return Err(shape(
                &format!("records[{index}].values.{field_id}"),
                format!(
                    "record `{}` names `{id}`, which is not a record of collection `{target}`",
                    record.id
                ),
            ));
        }
    }
    Ok(())
}

/// The collection a field's cell ids must name, or `None` when the field carries no
/// reference to check. A `link` field declares its target; a `parent` field declares none,
/// so it names records of the record's own collection.
fn target_of<'a>(record: &'a Record, field: &'a Field) -> Option<&'a str> {
    match field.role {
        Role::Link => field.link.as_ref().map(|link| link.collection.as_str()),
        Role::Parent => Some(record.collection.as_str()),
        _ => None,
    }
}

/// The record ids a cell names. A bare string is one reference and a list is one per
/// string — both shapes are committed data (`up: ["t1"]` on a `parent` field,
/// `blocks: ["t3"]` on a `many` link) — while an absent cell, `null` and `[]` name none.
///
/// Ponytail: a cell that is a number, a bool, a nested object, or a list nested more than
/// one deep carries no reference this reader recognises, so it is left to the derivation
/// rather than refused. Failing input: `{"blocks": 7}`, `{"blocks": [["t3"]]}`,
/// `{"blocks": [{"id": "t3"}]}`. Direction: SAFE — the derivation reads no reference out
/// of those shapes either (`graph-core`'s `roles::references` takes a string or a list of
/// strings and nothing else), so no edge is invented and no node is lost; the price is
/// that a mistyped cell stays structural rather than becoming a loud refusal. Escape
/// hatch: the declaration — a source whose references arrive in another shape declares
/// them `scalar` and carries the value with no edge, or normalises before it declares.
fn referenced_ids(cell: &JsonValue) -> Vec<&str> {
    match cell {
        JsonValue::Text(id) => vec![id.as_str()],
        JsonValue::List(items) => items.iter().filter_map(JsonValue::as_text).collect(),
        _ => Vec::new(),
    }
}
