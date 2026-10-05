//! Whether a second manifest may say something new about the first.
//!
//! The rule is one sentence — **manifests only grow** — and it exists because a manifest
//! is not a file that replaces the last one: the records already stored carry the *old*
//! declarations' cells, so a declaration that disappears or changes meaning leaves stored
//! data that no document can describe. A hub that absorbed such a change would answer a
//! read with text the client cannot round trip, and nothing in the response would say
//! why.
//!
//! So, in order:
//!
//! | `new.version` vs `old.version` | What may differ | Answer |
//! |---|---|---|
//! | equal | nothing | [`Growth::Same`] |
//! | equal | anything | 409 — the version is the client's promise that this is a *different* manifest |
//! | greater | collections and fields added, `name` changed | [`Growth::Grown`] |
//! | greater | a collection or field removed, or a field's role/target/name changed | 409 |
//! | less | anything | 409 — a client does not unpublish by going backwards |
//!
//! What a field's `name` and a collection's `name` may do is the one place the rule is
//! generous, and it is generous for the reason above rather than for convenience: nothing
//! in the motor derives from a name, so a rename carries no stored data with it.

use super::Manifest;
use crate::hub::HubError;
use crate::ingest::{Collection, Field, Role};

/// What a second manifest says about the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Growth {
    /// The same manifest: same version, same content. A retry, not a conflict.
    Same,
    /// The second adds to the first. Every collection and field of the first is still
    /// declared, with the meaning it had.
    Grown,
}

/// What `new` says about `old`, or the conflict that stops it. Order of the checks is
/// load-bearing and fixed: the version first (so a rollback is a conflict whatever it
/// contains), then the collections in id order, then each collection's fields in id order,
/// so the refusal names the same declaration whatever order the two manifests list them.
pub fn growth(old: &Manifest, new: &Manifest) -> Result<Growth, HubError> {
    if new.version == old.version {
        return same(old, new);
    }
    if new.version < old.version {
        return Err(conflict(format!(
            "manifest v{} is older than the registered v{}",
            new.version, old.version
        )));
    }
    for collection in &old.collections {
        let Some(kept) = new.collections.iter().find(|c| c.id == collection.id) else {
            return Err(conflict(format!(
                "manifest v{} removed collection `{}`",
                new.version, collection.id
            )));
        };
        check_fields(&collection.id, collection, kept, new.version)?;
    }
    Ok(Growth::Grown)
}

/// The equal-version case, which has two answers and not one: identical content is
/// nothing to do, and any difference is a conflict because the client said it was
/// publishing a new version and did not.
fn same(old: &Manifest, new: &Manifest) -> Result<Growth, HubError> {
    if old == new {
        return Ok(Growth::Same);
    }
    Err(conflict(format!(
        "manifest v{} was published with different content",
        new.version
    )))
}

/// Every field of `old` still declared in `new`, with the role and link target it had.
/// The three failures are one per thing a stored cell depends on: the *existence* of a
/// field (its cells are keyed by id), its *role* (which decides whether the motor reads
/// them) and its *target* (which decides what a reference in them resolves to).
fn check_fields(
    collection: &str,
    old: &Collection,
    new: &Collection,
    version: u32,
) -> Result<(), HubError> {
    for field in &old.fields {
        let Some(kept) = new.fields.iter().find(|f| f.id == field.id) else {
            return Err(conflict(format!(
                "manifest v{version} removed field `{collection}.{}`",
                field.id
            )));
        };
        if kept.role != field.role {
            return Err(conflict(format!(
                "manifest v{version} changed field `{collection}.{}` from role `{}` to `{}`",
                field.id,
                field.role.as_str(),
                kept.role.as_str()
            )));
        }
        check_target(collection, field, kept, version)?;
    }
    Ok(())
}

/// A link field's target: re-pointing one would leave every stored reference under it
/// resolving to a collection it was not written against. A field that gains or loses its
/// `link` member with its role is already refused by the ingest reader, so by the time
/// this runs both fields are `link` fields or neither is.
fn check_target(
    collection: &str,
    old: &Field,
    new: &Field,
    version: u32,
) -> Result<(), HubError> {
    if target_of(old) == target_of(new) {
        return Ok(());
    }
    let (from, to) = (target_of(old), target_of(new));
    Err(conflict(match (from, to) {
        (Some(from), Some(to)) => format!(
            "manifest v{version} re-pointed field `{collection}.{}` from `{from}` to `{to}`",
            old.id
        ),
        _ => format!(
            "manifest v{version} changed the link target of field `{collection}.{}`",
            old.id
        ),
    }))
}

fn target_of(field: &Field) -> Option<&str> {
    if field.role != Role::Link {
        return None;
    }
    field.link.as_ref().map(|link| link.collection.as_str())
}

fn conflict(what: String) -> HubError {
    HubError::Conflict { what }
}