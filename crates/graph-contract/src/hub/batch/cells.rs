//! One cell against the field that declares it: the write-time check.
//!
//! // Caveat: mirrors graph-core's role readers (`roles.rs`); a reader change there that
//! // this misses lets a cell in that the motor then ignores.
//!
//! Every role names one shape, and this is the only place a hub enforces it. The engine
//! would catch most of it later — `validate/cells.rs` walks link cells and `roles.rs`
//! reads each role — but "later" is a document the store already accepted and a client
//! already got a 200 for. So a hub checks at the write, and this file is a *mirror* of the
//! engine's readers rather than a copy of them: the two are separate code in separate
//! crates, which is exactly why the caveat at the top exists.
//!
//! `null` is legal everywhere. A source that has no value for a field sends `null`, and
//! refusing it would make "absent" and "empty" the same thing on the wire — which the
//! materializer's pruning rule then cannot express.

use super::super::HubError;
use super::super::manifest::Manifest;
use crate::hub::ids::qualify;
use crate::ingest::{Cardinality, Collection, JsonValue, Role};

/// The collection `name` names in the manifest for `plugin`, or the refusal. A batch
/// spells its collections unqualified, so this is where the qualification happens — and it
/// is the only place, so a stored record's collection is qualified exactly once.
pub(super) fn declared<'a>(
    manifest: &'a Manifest,
    plugin: &str,
    name: &str,
    path: &str,
) -> Result<&'a Collection, HubError> {
    let qualified = qualify(plugin, name);
    manifest
        .collections
        .iter()
        .find(|c| qualify(plugin, &c.id) == qualified)
        .ok_or_else(|| HubError::Invalid {
            path: path.to_owned(),
            what: format!("collection `{qualified}` is not declared by the manifest"),
        })
}

/// The cell `field_id` carries, against the field that declares it. Two refusals, in a
/// fixed order: the field must exist in this record's own collection, and its value must
/// fit the role. The field lookup is first because "no such field" is the more useful
/// answer when both are wrong.
pub(super) fn check_cell(
    plugin: &str,
    collection: &Collection,
    field_id: &str,
    value: &JsonValue,
    path: &str,
) -> Result<(), HubError> {
    let field = collection
        .field(field_id)
        .ok_or_else(|| HubError::Invalid {
            path: format!("{path}.values"),
            what: format!(
                "collection `{}` declares no field `{field_id}`",
                qualify(plugin, &collection.id)
            ),
        })?;
    let at = format!("{path}.values.{field_id}");
    match field.role {
        // `title`, `label` and `group` are read as text. A number there would be a label
        // no node can carry, and the engine's own reader treats it as absent — silently,
        // which is the direction that matters.
        Role::Title | Role::Label | Role::Group => text(value, &at),
        Role::Tags => tags(value, &at),
        // `scalar` is anything at all: it exists so a source can declare every field it
        // has, and the motor reads nothing from it.
        Role::Scalar => Ok(()),
        Role::Weight => number(value, &at),
        Role::Link => references(value, cardinality(field), &at),
        Role::Parent => single(value, &at),
    }
}

/// The cardinality a link field declares, defaulting to `One` — which the ingest reader
/// has already refused for a `link` field with no `link` member, so the `unwrap_or` is
/// unreachable rather than a decision.
fn cardinality(field: &crate::ingest::Field) -> Cardinality {
    field
        .link
        .as_ref()
        .map_or(Cardinality::One, |link| link.cardinality)
}

fn text(value: &JsonValue, at: &str) -> Result<(), HubError> {
    match value {
        JsonValue::Null | JsonValue::Text(_) => Ok(()),
        _ => Err(shape(at, "expected a string")),
    }
}

fn number(value: &JsonValue, at: &str) -> Result<(), HubError> {
    match value {
        JsonValue::Null | JsonValue::Number(_) => Ok(()),
        _ => Err(shape(at, "expected a number")),
    }
}

/// A tag list: strings, and no colon in one. The colon rule is H5's again — a tag becomes
/// a node id, and a `:` in it would parse back shifted.
fn tags(value: &JsonValue, at: &str) -> Result<(), HubError> {
    let JsonValue::Null = value else {
        let JsonValue::List(items) = value else {
            return Err(shape(at, "expected a list of strings"));
        };
        for (i, item) in items.iter().enumerate() {
            match item {
                JsonValue::Text(tag) if tag.contains(':') => {
                    return Err(shape(&format!("{at}[{i}]"), "a tag may not contain `:`"));
                }
                JsonValue::Text(_) => {}
                _ => return Err(shape(&format!("{at}[{i}]"), "expected a string")),
            }
        }
        return Ok(());
    };
    Ok(())
}

/// A link cell: a list of references for `many`, one for `one`, and never the other way
/// round — the cardinality is declared, so reading one as the other would hide a schema
/// mistake behind a value that happens to have one element.
fn references(value: &JsonValue, cardinality: Cardinality, at: &str) -> Result<(), HubError> {
    if matches!(value, JsonValue::Null) {
        return Ok(());
    }
    // The cardinality is checked **first**, because it decides which shape is even
    // possible: a `one` link is a single reference (bare, or a one-element list) and a
    // `many` link is a list. Reading a `many` as a single would hide a schema mistake
    // behind a value that happens to have one element.
    if cardinality == Cardinality::One {
        return single(value, at);
    }
    let JsonValue::List(items) = value else {
        return Err(shape(at, "expected a list of references"));
    };
    items
        .iter()
        .enumerate()
        .try_for_each(|(i, item)| single(item, &format!("{at}[{i}]")))
}

/// One reference: a string, or a list of exactly one — which is how a client writes a
/// `one` link it built from the same code that builds a `many` one. More than one element
/// under a `one` cardinality is the refusal; the single-element list is accepted so a
/// client is not forced to branch on the declaration to send the same value.
fn single(value: &JsonValue, at: &str) -> Result<(), HubError> {
    match value {
        JsonValue::Null => Ok(()),
        JsonValue::Text(_) => Ok(()),
        JsonValue::List(items) if items.len() == 1 => single(&items[0], at),
        JsonValue::List(_) => Err(shape(at, "expected a single reference")),
        _ => Err(shape(at, "expected a record reference")),
    }
}

fn shape(path: &str, what: &str) -> HubError {
    HubError::Invalid {
        path: path.to_owned(),
        what: what.to_owned(),
    }
}
