//! The checks that need the whole document: duplicate ids, dangling references, and
//! the id grammar (H5).
//!
//! Kept apart from `read.rs` because each of these is a *document-level* fact — a
//! duplicate id or a link to a collection that does not exist cannot be decided one
//! member at a time — and because separating them makes the list of cross-checks one
//! list rather than something threaded through the parser.
//!
//! Every lookup by id goes through an index built once per document — collections by id,
//! records by `(collection, id)`, a sorted field list searched by halving — so the whole
//! check is `O(n log n)` in the document's size. The scans these replaced made a 64 MiB
//! body take over 20 minutes (each record re-scanned every earlier one).

use super::{Collection, Field, Ingest, IngestError, Role};
use std::collections::{BTreeMap, BTreeSet};

mod cells;

/// Each declared collection by id. Ids are unique by the time anything reads it.
type Collections<'a> = BTreeMap<&'a str, &'a Collection>;

/// Every record's `(collection, id)`, deleted records included.
type Records<'a> = BTreeSet<(&'a str, &'a str)>;

/// Every check that needs the whole document, in a fixed order so the refusal a
/// document gets does not depend on which member the parser happened to reach first.
pub(super) fn check(mut doc: Ingest) -> Result<Ingest, IngestError> {
    check_id_grammar(&doc)?;
    // Two passes, and the order is load-bearing. Everything that *reports* runs first and
    // borrows the document immutably, so every refusal names the index the field has in
    // the file the reader has open. The canonical sort runs second: from here on the
    // field list is sorted, so the derivation's "first field with this role" and the
    // derived edge order depend on the ids and never on how the source happened to list
    // them (H6, D5).
    check_declarations(&doc)?;
    for collection in &mut doc.collections {
        collection.fields.sort_by(|a, b| a.id.cmp(&b.id));
    }
    // Built again rather than kept: the first index borrows the lists the sort rewrote. It
    // cannot refuse here, since `check_declarations` already refused a repeated id.
    let collections = collections_by_id(&doc)?;
    let records = record_keys(&doc)?;
    check_references(&doc, &collections)?;
    // Last, because it walks the cells of fields whose collections `check_references` has
    // already found (see `cells`'s module doc for the order this buys).
    cells::check_link_cells(&doc, &collections, &records)?;
    Ok(doc)
}

/// Collection ids unique, then each collection's own declarations, against the field
/// order the document wrote (see `check`).
fn check_declarations(doc: &Ingest) -> Result<(), IngestError> {
    let collections = collections_by_id(doc)?;
    for (i, collection) in doc.collections.iter().enumerate() {
        check_fields(collection, &collections, &format!("collections[{i}]"))?;
    }
    Ok(())
}

/// H5, decided (see the module doc): the node-id grammar is
/// `source:collection:record` and cannot represent `:` inside `source` or the
/// collection id — the parse comes back *shifted*, not `None`. Broadening the grammar
/// would move every existing node id and therefore every layout, which is a stop-and-
/// ask this phase does not take, so the coordinates are constrained instead and the
/// refusal names the coordinate.
///
/// Every coordinate a node id is built from is checked, and every field id with it:
/// `source`, each collection id, each record id, each field id. Only `source` and a
/// collection id carry the `:` rule (H5) — a record id is the last segment and a field id
/// is a cell key, so a colon in either is never split and is left alone. **Empty** is
/// refused for all four: `parse_node_id` splits on the first two colons and answers `Some`
/// with an empty segment rather than `None`, so `:task:r1` is a node id no consumer can
/// attribute back to a record and nothing downstream would say so; and a field id is not
/// in a node id at all, but it is the key a record's cells are read by, so an empty one is
/// the same mistake one level down.
fn check_id_grammar(doc: &Ingest) -> Result<(), IngestError> {
    check_coordinate("source", &doc.source)?;
    for collection in &doc.collections {
        check_coordinate("collection id", &collection.id)?;
        for field in &collection.fields {
            check_present("field id", &field.id)?;
        }
    }
    for record in &doc.records {
        check_present("record id", &record.id)?;
    }
    Ok(())
}

/// A coordinate that goes into a node id: present, and free of `:` (H5).
fn check_coordinate(coordinate: &'static str, value: &str) -> Result<(), IngestError> {
    check_present(coordinate, value)?;
    if value.contains(':') {
        return Err(IngestError::IdGrammar {
            coordinate,
            value: value.to_owned(),
        });
    }
    Ok(())
}

/// The rule both halves share: an id is not an empty string.
fn check_present(coordinate: &'static str, value: &str) -> Result<(), IngestError> {
    if value.is_empty() {
        return Err(shape(
            coordinate,
            "an id coordinate cannot be empty".to_owned(),
        ));
    }
    Ok(())
}

/// Each collection by id, or the refusal naming the first collection whose id an earlier
/// one already has.
fn collections_by_id(doc: &Ingest) -> Result<Collections<'_>, IngestError> {
    let mut by_id = Collections::new();
    for (i, collection) in doc.collections.iter().enumerate() {
        if by_id.insert(&collection.id, collection).is_some() {
            return Err(shape(
                &format!("collections[{i}]"),
                format!("duplicate collection id `{}`", collection.id),
            ));
        }
    }
    Ok(by_id)
}

/// Per collection: field ids unique, `titleField` naming a declared `title` field,
/// every `link.collection` naming a collection the document declares.
/// `path` is the collection's own path, so a refusal names which collection a bad field
/// is in — with two collections in a document, `collections.fields[1]` is not an answer.
/// The indices below are the *document's*, which is what a reader with the file open is
/// looking at; the canonical sort happens in a later pass.
fn check_fields(
    collection: &Collection,
    collections: &Collections<'_>,
    path: &str,
) -> Result<(), IngestError> {
    let mut ids = BTreeSet::new();
    for (i, field) in collection.fields.iter().enumerate() {
        if !ids.insert(field.id.as_str()) {
            return Err(shape(
                &format!("{path}.fields[{i}]"),
                format!("duplicate field id `{}`", field.id),
            ));
        }
    }
    check_title(collection, path)?;
    check_link_targets(collection, collections, path)
}

/// The collection's `titleField` must name a field that exists *and* has the `title`
/// role. Both directions are refused: a label read from a `group` field would look
/// right and mean something else, which is the direction that matters.
fn check_title(collection: &Collection, path: &str) -> Result<(), IngestError> {
    let title_path = format!("{path}.titleField");
    let Some(field) = collection.field(&collection.title_field) else {
        return Err(shape(
            &title_path,
            format!("no field with id `{}`", collection.title_field),
        ));
    };
    if field.role != Role::Title {
        return Err(shape(
            &title_path,
            format!(
                "field `{}` has role `{}`, not `title`",
                field.id,
                field.role.as_str()
            ),
        ));
    }
    Ok(())
}

/// A record id is unique **within its collection**, not in the document: the derived node
/// id is `source:collection:record`, so the same id in two collections is two records
/// with two distinct nodes, and `graph-core`'s own duplicate check keys on the pair. The
/// message is the one this check has always given, so a document that really does repeat
/// an id inside one collection reads the same as it did before. The refusal names the
/// first record whose key an earlier record already has; the keys are what
/// `cells::check_link_cells` resolves references against.
fn record_keys(doc: &Ingest) -> Result<Records<'_>, IngestError> {
    let mut keys = Records::new();
    for (i, record) in doc.records.iter().enumerate() {
        if !keys.insert((&record.collection, &record.id)) {
            return Err(shape(
                &format!("records[{i}]"),
                format!("duplicate record id `{}`", record.id),
            ));
        }
    }
    Ok(keys)
}

/// Every record names a declared collection, and every cell key names a declared field
/// of *that* collection. A cell naming a field of some other collection is a mistake
/// the derivation would otherwise read as absent — silently, which is the direction
/// that matters.
fn check_references(doc: &Ingest, collections: &Collections<'_>) -> Result<(), IngestError> {
    for (i, record) in doc.records.iter().enumerate() {
        let path = format!("records[{i}]");
        let Some(collection) = collections.get(record.collection.as_str()) else {
            return Err(shape(
                &path,
                format!("no collection with id `{}`", record.collection),
            ));
        };
        for (field_id, _) in &record.values {
            if sorted_field(collection, field_id).is_none() {
                return Err(shape(
                    &format!("{path}.values"),
                    format!(
                        "collection `{}` declares no field `{field_id}`",
                        collection.id
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Every `link` field names a collection the document declares. A link to a collection
/// that does not exist would derive no edges at all, and nothing in the output would
/// say the field was dead.
fn check_link_targets(
    collection: &Collection,
    collections: &Collections<'_>,
    path: &str,
) -> Result<(), IngestError> {
    for (i, field) in collection.fields.iter().enumerate() {
        if field.role != Role::Link {
            continue;
        }
        let Some(link) = &field.link else {
            continue;
        };
        if !collections.contains_key(link.collection.as_str()) {
            return Err(shape(
                &format!("{path}.fields[{i}].link.collection"),
                format!(
                    "field `{}` links to collection `{}`, which is not declared",
                    field.id, link.collection
                ),
            ));
        }
    }
    Ok(())
}

/// The field `id` of a collection whose fields `check` has sorted: a search by halving where
/// [`Collection::field`] scans, because it runs once per cell. It answers what that scan
/// answers only on a sorted, duplicate-free field list, which is what every pass after the
/// sort in `check` holds.
fn sorted_field<'a>(collection: &'a Collection, id: &str) -> Option<&'a Field> {
    let fields = &collection.fields;
    let at = fields
        .binary_search_by(|field| field.id.as_str().cmp(id))
        .ok()?;
    Some(&fields[at])
}

fn shape(path: &str, what: String) -> IngestError {
    IngestError::Shape {
        path: path.to_owned(),
        what,
    }
}
