//! The cells a `link` or `parent` field carries: every record id in one must name a record
//! the document declares.
//!
//! A child module for one reason — `validate.rs` sits at the house's 300-line limit and
//! this is the longest walk in it (every record, every field of its collection, every
//! cell of those fields).

use super::{Ingest, IngestError, Role, shape};
use crate::ingest::{Field, JsonValue, Record};

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
pub(super) fn check_link_cells(doc: &Ingest) -> Result<(), IngestError> {
    for (i, record) in doc.records.iter().enumerate() {
        let Some(collection) = doc.collection(&record.collection) else {
            continue;
        };
        let path = format!("records[{i}]");
        for field in &collection.fields {
            if matches!(field.role, Role::Link | Role::Parent) {
                check_field(doc, record, field, &path)?;
            }
        }
    }
    Ok(())
}

fn check_field(
    doc: &Ingest,
    record: &Record,
    field: &Field,
    path: &str,
) -> Result<(), IngestError> {
    let (Some(target), Some(cell)) = (target_of(record, field), record.value(&field.id)) else {
        return Ok(());
    };
    for id in referenced_ids(cell) {
        if !declares(doc, target, id) {
            return Err(shape(
                &format!("{path}.values.{}", field.id),
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

/// Whether `id` is a record of `collection`, deleted ones included (see the module doc).
/// A linear scan, the shape `check_unique_records` already uses: linear in practice,
/// quadratic in the worst case, which is this reader's existing bound.
fn declares(doc: &Ingest, collection: &str, id: &str) -> bool {
    doc.records
        .iter()
        .any(|record| record.collection == collection && record.id == id)
}
