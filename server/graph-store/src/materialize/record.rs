//! A stored record's text read back into a `Record`.
//!
//! graph-contract's own record reader is crate-private, so this reads the text through the public
//! `read_value` and takes the five members `record_piece` writes. A stored text is the store's
//! own output, so any other shape is a store fault, not a client error.

use graph_contract::ingest::{JsonValue, Record, read_value};

use crate::error::{DbError, StoreError};

/// The record `text` was written from.
pub(crate) fn parse(text: &str) -> Result<Record, StoreError> {
    let value = read_value(text).map_err(|e| fault(format!("does not read: {e}")))?;
    let JsonValue::Map(members) = value else {
        return Err(fault("is not an object".to_owned()));
    };
    if members.len() != 5 {
        return Err(fault(format!("has {} members, not 5", members.len())));
    }
    Ok(Record {
        collection: text_of(&members, "collection")?,
        deleted: match member(&members, "deleted")? {
            JsonValue::Bool(b) => *b,
            _ => return Err(fault("`deleted` is not a bool".to_owned())),
        },
        id: text_of(&members, "id")?,
        updated_at: updated_at(member(&members, "updatedAt")?)?,
        values: match member(&members, "values")? {
            JsonValue::Map(cells) => cells.clone(),
            _ => return Err(fault("`values` is not an object".to_owned())),
        },
    })
}

/// The member named `key`.
fn member<'a>(members: &'a [(String, JsonValue)], key: &str) -> Result<&'a JsonValue, StoreError> {
    members
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .ok_or_else(|| fault(format!("has no `{key}`")))
}

/// The text member named `key`.
fn text_of(members: &[(String, JsonValue)], key: &str) -> Result<String, StoreError> {
    match member(members, key)? {
        JsonValue::Text(text) => Ok(text.clone()),
        _ => Err(fault(format!("`{key}` is not a string"))),
    }
}

/// `updatedAt`, which `record_piece` writes from a `u32`.
fn updated_at(value: &JsonValue) -> Result<u32, StoreError> {
    match value {
        JsonValue::Number(n) if n.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(n) => {
            Ok(*n as u32)
        }
        _ => Err(fault("`updatedAt` is not a u32".to_owned())),
    }
}

/// A stored record text the store cannot read back.
fn fault(what: String) -> StoreError {
    StoreError::Db(DbError::store(
        "XX000",
        format!("a stored record text {what}"),
    ))
}
