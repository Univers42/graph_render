//! Pruning: what the document drops that a declaration or a record still mentions.
//!
//! A hub serves a *document*, and a document is read by `ingest::read`, which refuses
//! three things a stored hub can legitimately be holding:
//!
//! - a `link` field pointing at a collection no registered plugin declares;
//! - a link or parent cell naming a record that is not stored;
//! - a `values` key naming a field the collection does not declare.
//!
//! So the writer drops those, rather than writing a document its own reader refuses. The
//! consequence is deliberate and is the reason this file exists: **a hub's document is not
//! necessarily a faithful copy of what was sent**. A dangling reference is invisible in the
//! document, and a client that diffs documents cannot tell a deleted record from a dropped
//! link. That is the trade the spec makes, and [`doc_bytes`](crate::hub::Model::doc_bytes) is the bound that keeps it
//! honest.
//!
//! # Two rules that are not the same rule
//!
//! | Dropped | Rule | Negctl |
//! |---|---|---|
//! | a `link` **field** whose target is unregistered | `kept_collection` | `keep-cells` |
//! | a **cell** whose target record is absent | `prune_record` | `keep-dangling` |
//!
//! An empty list emptied by pruning is **removed**; a list that was already empty is
//! **kept**. The difference is visible: a client that sent `[]` means "this record has no
//! tags", and a hub that dropped the key would answer a read with a document where the
//! record has no tags *field* — two different documents to a client that is diffing them.

use super::breaks;
use crate::ingest::{Collection, Field, JsonValue, Record, Role};

/// The declaration as it is written: `c` with every `link` field whose target is not
/// registered dropped.
///
/// A dropped field is dropped from the *declaration* too, not only from the records' cells:
/// a document that declared a field no cell carried would read back with a field the
/// records do not use, which is legal but is a different document from the one the model
/// means.
pub fn kept_collection(c: &Collection, registered: &dyn Fn(&str) -> bool) -> Collection {
    let mut kept = c.clone();
    kept.fields.retain(|f| {
        let Some(link) = &f.link else { return true };
        // `keep-cells` turns the whole rule off, and the record cells with it: a negative
        // control that dropped only the *declaration* would leave a document whose cells
        // name fields no collection declares, which the reader refuses for a different
        // reason — and the row would go red without testing what it claims to.
        if breaks::on("keep-cells") {
            return true;
        }
        registered(&link.collection)
    });
    kept
}

/// `r` with everything that does not resolve dropped, or `None` when there was nothing to
/// drop — the signal the writer uses to write the record's stored bytes verbatim instead of
/// re-writing a record that is already right.
///
/// `exists` answers "is a record with this qualified collection and id stored", which only
/// the model knows: a link into another plugin's collection is decidable from the manifest,
/// but whether that plugin has registered *that collection* is a fact about the workspace.
pub fn prune_record(
    r: &Record,
    kept: &Collection,
    exists: &dyn Fn(&str, &str) -> bool,
) -> Option<Record> {
    let mut out = r.clone();
    let mut dropped_any = false;
    out.values.retain(|(field_id, value)| {
        let Some(field) = kept.field(field_id) else {
            // A field the kept declaration no longer declares: its cells cannot be written
            // under a key the reader would refuse.
            dropped_any = true;
            return false;
        };
        match prune_cell(field, &r.collection, value, exists) {
            Fate::Unchanged => true,
            Fate::Dropped => {
                dropped_any = true;
                false
            }
            // An empty list emptied by pruning is removed, and an empty list that was
            // already empty is kept. A client that sent `[]` said something; a client whose
            // references all dangled did not.
            Fate::Emptied => {
                dropped_any = true;
                false
            }
        }
    });
    dropped_any.then_some(out)
}

/// One cell's fate. Three answers rather than a bool, because "the list is now empty" and
/// "the cell is not a list at all" both mean *drop* for the key and mean different things
/// for the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fate {
    /// Written as it is.
    Unchanged,
    /// The cell itself does not resolve: dropped.
    Dropped,
    /// The cell was a list and pruning emptied it: the key is dropped, and that is a
    /// different fact from a list that arrived empty.
    Emptied,
}

/// One cell against the field that declares it, with `exists` resolving references.
///
/// The two reference roles resolve against **different** collections, and that is the
/// whole reason both are named here:
///
/// - a `link` names a collection in its own `link` member, so a bare target means this
///   plugin's collection (qualified on the way in);
/// - a `parent` has **no** `link` member at all — the ingest reader refuses one — because
///   a parent is by definition a record in the *same* collection. So its target is the
///   record's own collection, which the caller supplies.
///
/// Getting the second wrong would prune every parent edge in a document, silently: the
/// cell is dropped, the document still reads, and the hierarchy is simply gone.
fn prune_cell(
    field: &Field,
    own: &str,
    value: &JsonValue,
    exists: &dyn Fn(&str, &str) -> bool,
) -> Fate {
    match field.role {
        Role::Tags | Role::Scalar | Role::Title | Role::Label | Role::Group | Role::Weight => {
            Fate::Unchanged
        }
        Role::Parent => match value {
            JsonValue::Null => Fate::Unchanged,
            JsonValue::Text(id) => resolves(own, id, exists),
            _ => Fate::Dropped,
        },
        Role::Link => {
            let target = field
                .link
                .as_ref()
                .map_or("", |link| link.collection.as_str());
            match value {
                JsonValue::Null => Fate::Unchanged,
                JsonValue::Text(id) => resolves(target, id, exists),
                JsonValue::List(items) => prune_list(target, items, exists),
                _ => Fate::Dropped,
            }
        }
    }
}

/// A list of references, keeping the ones that resolve. An *empty* list is unchanged — the
/// client said there are none — and a list whose every element dangled is `Emptied`, which
/// drops the key.
fn prune_list(target: &str, items: &[JsonValue], exists: &dyn Fn(&str, &str) -> bool) -> Fate {
    if items.is_empty() {
        return Fate::Unchanged;
    }
    let mut kept: Vec<JsonValue> = Vec::with_capacity(items.len());
    for item in items {
        match item {
            JsonValue::Text(id) if resolves(target, id, exists) == Fate::Unchanged => {
                kept.push(item.clone());
            }
            JsonValue::Null => kept.push(item.clone()),
            _ => {}
        }
    }
    if kept.len() == items.len() {
        return Fate::Unchanged;
    }
    if kept.is_empty() {
        return Fate::Emptied;
    }
    Fate::Dropped
}

/// Whether the reference `id` resolves in `target`. `keep-dangling` turns this off, which
/// is what makes `negctl-keep-dangling` go red: with it on, a dangling reference is written
/// and the document carries a cell naming a record that is not there.
fn resolves(target: &str, id: &str, exists: &dyn Fn(&str, &str) -> bool) -> Fate {
    if breaks::on("keep-dangling") {
        return Fate::Unchanged;
    }
    if exists(target, id) {
        Fate::Unchanged
    } else {
        Fate::Dropped
    }
}
