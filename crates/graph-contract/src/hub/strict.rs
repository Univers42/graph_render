//! The two rules every hub body is read under, before anything else looks at it: a size
//! cap, and no NUL anywhere.
//!
//! Both are here rather than in each reader because both are properties of the *bytes*,
//! decided before the shape is: a body over its cap is refused with [`HubError::TooLarge`]
//! whether it is a manifest, a batch or a change, and a NUL is refused whatever it is
//! inside. Spreading either rule into three readers would give three chances to write it
//! differently, and "the manifest reader walks strings but the batch reader does not" is
//! exactly the difference a caller finds out about in production.
//!
//! ## The NUL walk, and its cost
//!
//! `canonical_json::parse` already unescapes every string, so the walk is over the parsed
//! tree rather than the text: no un-escaping here, and no way for the two to disagree
//! about what a `\u0000` in the text means. Depth needs no bound of its own — the parser
//! refuses past [`crate::canonical_json::MAX_DEPTH`] already.

use super::{HubError, breaks};
use crate::canonical_json::{self, Value};
use crate::ingest::IngestError;

/// One hub body as a parsed value, or the refusal. The size is checked on the *bytes*,
/// before the parse: a body four times the cap must not cost four times the parse.
pub(crate) fn parse_strict(
    text: &str,
    max: u64,
    what: &'static str,
) -> Result<Value, HubError> {
    if text.len() as u64 > max {
        return Err(HubError::TooLarge {
            what,
            limit: max,
        });
    }
    let value = canonical_json::parse(text).map_err(|e| HubError::Shape(IngestError::from_json(e)))?;
    if !breaks::on("lax-reader") {
        check_nul(&value, "")?;
    }
    Ok(value)
}

/// The first NUL in the tree, by a fixed order: members in the order the parser kept
/// them (document order — a refusal must not depend on which member a walk reached
/// first, D5), then each member's key before its value.
///
/// A NUL in a **key** counts as much as one in a string. That is the whole reason this
/// walk exists: a store that keys a column or a file by a name is truncated by a NUL in
/// the name in exactly the way it is truncated by one in the value, and nothing above
/// this function looks at keys.
fn check_nul(value: &Value, path: &str) -> Result<(), HubError> {
    match value {
        Value::String(text) if text.contains('\0') => Err(HubError::Nul {
            path: path.to_owned(),
        }),
        Value::String(_) | Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .try_for_each(|(i, item)| check_nul(item, &format!("{path}[{i}]"))),
        Value::Object(members) => members.iter().try_for_each(|(key, member)| {
            // A NUL in a key is reported at the *object's* path, not at the key: the key
            // is what is broken, so a path naming it would be a path that cannot be typed
            // into a reader. At the root that is the empty path, which is why a root-level
            // `{"k\0": 1}` answers `Nul { path: "" }`.
            if key.contains('\0') {
                return Err(HubError::Nul {
                    path: path.to_owned(),
                });
            }
            check_nul(member, &child(path, key))
        }),
    }
}

/// One step down the path: `key` at the root, `path.key` below it.
///
/// A member at the root has no path, so its key *is* the path — that is what makes
/// `Nul { path: "" }` the answer for a NUL in a root-level key, and what a reader with
/// the file open needs to find it.
pub(crate) fn child(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}