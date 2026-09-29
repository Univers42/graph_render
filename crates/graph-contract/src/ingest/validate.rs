//! The checks that need the whole document: duplicate ids, dangling references, and
//! the id grammar (H5).
//!
//! Kept apart from `read.rs` because each of these is a *document-level* fact — a
//! duplicate id or a link to a collection that does not exist cannot be decided one
//! member at a time — and because separating them makes the list of cross-checks one
//! list rather than something threaded through the parser.

use super::{Collection, Ingest, IngestError, Role};

/// Every check that needs the whole document, in a fixed order so the refusal a
/// document gets does not depend on which member the parser happened to reach first.
pub(super) fn check(mut doc: Ingest) -> Result<Ingest, IngestError> {
    check_id_grammar(&doc)?;
    check_unique_collections(&doc)?;
    // Two passes, and the order is load-bearing. Everything that *reports* runs first and
    // borrows the document immutably, so every refusal names the index the field has in
    // the file the reader has open. The canonical sort runs second: from here on the
    // field list is sorted, so `first_with_role` and the derived edge order depend on the
    // ids and never on how the source happened to list them.
    for (i, collection) in doc.collections.iter().enumerate() {
        check_fields(collection, &doc, &format!("collections[{i}]"))?;
    }
    for collection in &mut doc.collections {
        collection.fields.sort_by(|a, b| a.id.cmp(&b.id));
    }
    check_unique_records(&doc)?;
    check_references(&doc)?;
    Ok(doc)
}

/// H5, decided (see the module doc): the node-id grammar is
/// `source:collection:record` and cannot represent `:` inside `source` or the
/// collection id — the parse comes back *shifted*, not `None`. Broadening the grammar
/// would move every existing node id and therefore every layout, which is a stop-and-
/// ask this phase does not take, so the coordinates are constrained instead and the
/// refusal names the coordinate.
///
/// The record id is deliberately unconstrained: it is the last segment, so a `:` in it
/// round-trips exactly (`parse_node_id`'s `splitn(3, ':')` rejoins the remainder).
fn check_id_grammar(doc: &Ingest) -> Result<(), IngestError> {
    check_coordinate("source", &doc.source)?;
    for collection in &doc.collections {
        check_coordinate("collection id", &collection.id)?;
    }
    Ok(())
}

fn check_coordinate(coordinate: &'static str, value: &str) -> Result<(), IngestError> {
    if value.contains(':') {
        return Err(IngestError::IdGrammar {
            coordinate,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn check_unique_collections(doc: &Ingest) -> Result<(), IngestError> {
    for (i, collection) in doc.collections.iter().enumerate() {
        if doc.collections[..i]
            .iter()
            .any(|earlier| earlier.id == collection.id)
        {
            return Err(shape(
                &format!("collections[{i}]"),
                format!("duplicate collection id `{}`", collection.id),
            ));
        }
    }
    Ok(())
}

/// Per collection: field ids unique, `titleField` naming a declared `title` field,
/// every `link.collection` naming a collection the document declares.
/// `path` is the collection's own path, so a refusal names which collection a bad field
/// is in — with two collections in a document, `collections.fields[1]` is not an answer.
/// `path` is the collection's own path, so a refusal names which collection a bad field
/// is in — with two collections in a document, `collections.fields[1]` is not an answer.
/// The indices below are the *document's*, which is what a reader with the file open is
/// looking at; the canonical sort happens in a later pass.
fn check_fields(collection: &Collection, doc: &Ingest, path: &str) -> Result<(), IngestError> {
    for (i, field) in collection.fields.iter().enumerate() {
        if collection.fields[..i]
            .iter()
            .any(|earlier| earlier.id == field.id)
        {
            return Err(shape(
                &format!("{path}.fields[{i}]"),
                format!("duplicate field id `{}`", field.id),
            ));
        }
    }
    check_title(collection, path)?;
    check_link_targets(collection, doc, path)
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

fn check_unique_records(doc: &Ingest) -> Result<(), IngestError> {
    for (i, record) in doc.records.iter().enumerate() {
        if doc.records[..i]
            .iter()
            .any(|earlier| earlier.id == record.id)
        {
            return Err(shape(
                &format!("records[{i}]"),
                format!("duplicate record id `{}`", record.id),
            ));
        }
    }
    Ok(())
}

/// Every record names a declared collection, and every cell key names a declared field
/// of *that* collection. A cell naming a field of some other collection is a mistake
/// the derivation would otherwise read as absent — silently, which is the direction
/// that matters.
fn check_references(doc: &Ingest) -> Result<(), IngestError> {
    for (i, record) in doc.records.iter().enumerate() {
        let path = format!("records[{i}]");
        let Some(collection) = doc.collection(&record.collection) else {
            return Err(shape(
                &path,
                format!("no collection with id `{}`", record.collection),
            ));
        };
        for (field_id, _) in &record.values {
            if collection.field(field_id).is_none() {
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
    doc: &Ingest,
    path: &str,
) -> Result<(), IngestError> {
    for (i, field) in collection.fields.iter().enumerate() {
        if field.role != Role::Link {
            continue;
        }
        let Some(link) = &field.link else {
            continue;
        };
        if doc.collection(&link.collection).is_none() {
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

fn shape(path: &str, what: String) -> IngestError {
    IngestError::Shape {
        path: path.to_owned(),
        what,
    }
}
