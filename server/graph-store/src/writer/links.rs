//! The `links` rows one record contributes, so materialization can anti-join instead of
//! re-reading every record (spec §4).
//!
//! A link row is `(ws, src_qcoll, src_id, field, target_qcoll, target_id)`: the field is named
//! because a cell is dropped or kept per *field*, not per record, and the two reference roles
//! resolve against different collections (see graph-contract's `prune`, which is the reader of
//! these rows).

use std::collections::BTreeSet;

use graph_contract::ingest::{Collection, Field, JsonValue, Record, Role};

/// The two reference roles, and the collection each one resolves against.
///
/// A `link` names its target in its own `link` member, so the target is already qualified. A
/// `parent` has no `link` member at all — the ingest reader refuses one — because a parent is by
/// definition a record in the *same* collection, which is why this needs the record's own
/// collection and cannot read it from the field.
///
/// Getting the second wrong would drop every parent edge in a document silently: the document
/// still reads, and the hierarchy is simply gone.
fn target_of<'a>(field: &'a Field, record: &'a Record) -> &'a str {
    match field.role {
        Role::Parent => &record.collection,
        _ => field
            .link
            .as_ref()
            .map_or("", |link| link.collection.as_str()),
    }
}

/// The link rows `(field, target_qcoll, target_id)` for one record, in `(field, target)` order.
///
/// A `BTreeSet`, so a list cell that names the same record twice contributes one row rather than
/// violating the table's primary key. The order is the set's, never the cell's, because a cell's
/// order is not a fact the store may keep.
///
/// Caveat: a cell that is not a string and not a list is written by no row here. graph-contract's
/// `Batch::check` has already refused that shape at the write, so the case is unreachable through
/// the entry points; it is dropped rather than refused here because this function runs inside a
/// transaction whose writes are the records themselves.
pub(crate) fn tuples(record: &Record, collection: &Collection) -> Vec<(String, String, String)> {
    let mut out = BTreeSet::new();
    for (field_id, value) in &record.values {
        let Some(field) = collection.field(field_id) else {
            continue;
        };
        if !matches!(field.role, Role::Link | Role::Parent) {
            continue;
        }
        let target = target_of(field, record);
        push(&mut out, field_id, target, value);
    }
    out.into_iter().collect()
}

/// One cell's references, added to the set.
fn push(
    out: &mut BTreeSet<(String, String, String)>,
    field_id: &str,
    target: &str,
    value: &JsonValue,
) {
    match value {
        JsonValue::Text(id) => {
            out.insert((field_id.to_owned(), target.to_owned(), id.clone()));
        }
        JsonValue::List(items) => {
            for item in items {
                if let JsonValue::Text(id) = item {
                    out.insert((field_id.to_owned(), target.to_owned(), id.clone()));
                }
            }
        }
        _ => {}
    }
}