//! Every id a hub wire carries, and the grammar each one obeys.
//!
//! Four grammars, chosen so that a node id built from them round-trips through the
//! motor's own `source:collection:record` grammar (ingest.rs, "The id grammar"):
//!
//! | What | Grammar | Why this shape |
//! |---|---|---|
//! | workspace, plugin | `[a-z0-9][a-z0-9-]{0,62}` | a URL path segment and a DNS-ish label at once, so a plugin id can be a filename, a path segment and an object key without escaping |
//! | collection | `[A-Za-z0-9_-]{1,64}` | vendor collection ids are mixed-case and hold `_` and `-` |
//! | qualified | `<plugin>.<collection>` | one dot separates the two, and neither grammar has a dot, so the split is never ambiguous |
//! | record | any non-empty text with no `:` | the last segment of a node id, so a colon would parse back shifted (H5) |
//!
//! Every check is a byte loop, not a regular expression: the crate is dependency-free so
//! graph-core can carry it, and the grammars are three rules each.
//!
//! [`qualify`] is the one place the qualified form is built, because a qualified id that
//! was assembled two ways would sort two ways.

use super::HubError;

/// The largest a `seq` or `rev` may be: `2^53 − 1`, the last integer a JavaScript
/// `number` holds exactly. Past it a JSON consumer would round the value, so a cursor
/// or a rev that cannot survive the round trip is refused at the reader.
pub const MAX_SEQ: u64 = (1 << 53) - 1;

/// A workspace id: `[a-z0-9][a-z0-9-]{0,62}`.
pub fn check_workspace_id(id: &str) -> Result<(), HubError> {
    check_slug(id, "workspace id")
}

/// A plugin id: `[a-z0-9][a-z0-9-]{0,62]`, the same grammar as a workspace id.
pub fn check_plugin_id(id: &str) -> Result<(), HubError> {
    check_slug(id, "plugin id")
}

/// A collection id: `[A-Za-z0-9_-]{1,64}`. Mixed case on purpose — the vendors' ids are.
pub fn check_collection_id(id: &str) -> Result<(), HubError> {
    if id.is_empty() || id.len() > 64 {
        return Err(grammar("collection id", id));
    }
    for byte in id.bytes() {
        if !(byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-') {
            return Err(grammar("collection id", id));
        }
    }
    Ok(())
}

/// A record id: not empty and free of `:`. Everything else is legal, including `"`
/// and a space — a record id is opaque data, and refusing punctuation would refuse a
/// vendor's own ids for a reason the motor never uses.
pub fn check_record_id(id: &str) -> Result<(), HubError> {
    if id.is_empty() {
        return Err(grammar("record id", id));
    }
    if id.contains(':') {
        return Err(HubError::Grammar {
            coordinate: "record id",
            value: id.to_owned(),
        });
    }
    Ok(())
}

/// `<plugin>.<collection>` — the id every collection is stored, written and sorted
/// under, so a document from two plugins can hold both without a name clash.
pub fn qualify(plugin: &str, collection: &str) -> String {
    format!("{plugin}.{collection}")
}

/// A slug check, shared by the two ids with the same grammar. `max` is the total
/// length, so the grammar and its bound are one rule rather than two.
fn check_slug(id: &str, what: &'static str) -> Result<(), HubError> {
    let bytes = id.as_bytes();
    let head = bytes
        .first()
        .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    let tail = bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-');
    if !(head && tail && bytes.len() <= 63) {
        return Err(grammar(what, id));
    }
    Ok(())
}

fn grammar(coordinate: &'static str, value: &str) -> HubError {
    HubError::Grammar {
        coordinate,
        value: value.to_owned(),
    }
}

/// Where a hub's change stream is, and how far along it is.
///
/// Written `<epoch>.<seq>`: an epoch is a store generation and a seq counts changes
/// within it, so a store that is rebuilt can answer "from epoch 4" and a reader that
/// was behind stays behind *in its own epoch* rather than silently jumping into a
/// different numbering. Both halves stop at [`MAX_SEQ`], for the reason that constant
/// gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// The store generation.
    pub epoch: u64,
    /// How many changes have been applied in it.
    pub seq: u64,
}

impl Cursor {
    /// The cursor `text` names, or the refusal. Strict: two halves, a dot, both plain
    /// non-negative integers with no leading zero — so one text has exactly one cursor
    /// and a round trip through [`Display`] is the identity.
    pub fn parse(text: &str) -> Result<Cursor, HubError> {
        let bad = || HubError::Cursor {
            text: text.to_owned(),
        };
        let (epoch, seq) = text.split_once('.').ok_or_else(bad)?;
        Ok(Cursor {
            epoch: half(epoch).ok_or_else(bad)?,
            seq: half(seq).ok_or_else(bad)?,
        })
    }
}

/// One cursor half: a plain integer below [`MAX_SEQ`]. A leading zero is refused because
/// `"03.1"` would then name the same cursor as `"3.1"` in two texts.
fn half(text: &str) -> Option<u64> {
    if text.is_empty() || (text.len() > 1 && text.starts_with('0')) {
        return None;
    }
    if !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let value = text.parse::<u64>().ok()?;
    (value <= MAX_SEQ).then_some(value)
}

impl core::fmt::Display for Cursor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}", self.epoch, self.seq)
    }
}
