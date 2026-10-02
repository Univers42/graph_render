//! Provisional ingest (`docs/contract/wasm-abi.md` "Ingest — PROVISIONAL", C13): a
//! versioned JSON array of node/edge records in the shape `graph_core::records` already
//! uses, parsed with `graph_contract::canonical_json`'s strict RFC 8259 reader.
//!
//! **The real ingest contract lives in [`crate::contract`]**, read by
//! `gm_build_contract`. This one is unchanged and stays: it is what the host studio and
//! the hash gate's C20 stage already speak, so replacing it would move a published ABI's
//! meaning. It is deliberately narrow: every member is named and
//! required (a present `null` where a field may be absent, never an omitted key) except an
//! edge's `child_first`, which version 1 reads as `false` when omitted (F-01: the SDK smoke
//! harness and the documented example omit it, so requiring it is a version 2), an
//! unknown member refuses the whole document (so a stray `hasNote` — the oracle's own
//! camelCase — is refused loudly, not silently ignored), and `kind` strings are matched
//! by exact name (`NodeKind`/`EdgeKind::from_name`), never the lossy `edge_kind_from_type`
//! heuristic. Duplicate ids and dangling edges are refused outright (C12): the
//! alternative graph_core::index_model itself takes — first-wins, drop and forget — would
//! make ingest order diverge silently from snapshot order, which is exactly the identity
//! this ABI promises callers (`docs/contract/wasm-abi.md` "Column order").

use graph_contract::canonical_json::{JsonError, Value, parse};
use graph_core::{EdgeKind, EdgeRecord, NodeKind, NodeRecord};

use crate::errors::Code;

mod at;
mod ids;
mod record;
use at::At;
use ids::check_ids;
use record::{edge, node};

/// The only ingest version this reader accepts.
pub const VERSION: u32 = 1;

/// The longest ingest document [`read`] accepts, in bytes: one past this is
/// [`IngestError::TooLarge`], checked before [`read`] parses or `from_utf8` touches a byte.
///
/// Measured, not chosen (`docs/decisions/wasm-ingest-limits.md` steps 1-3,
/// `docs/measurements/fix-wasm-ingest.md`): the studio's own generator at its 1M-node scale
/// target, doubling up, on the `wasm32-unknown-unknown` release artifact under Node. The
/// largest document that built was 774,568,785 bytes; the next one up, 799,922,860 bytes,
/// trapped inside `graph_core::index_model`'s string arena, and so did 842,132,644 bytes at
/// the studio's own `MAX_NODES`. This is the largest power of two at or below the largest
/// that built, so the step down to 536,870,912 is the rule's margin, not a guess.
///
/// Ponytail: it bounds bytes, not the work they imply. The sweep found the boundary at a work
/// level too — 3,679,984 edges built, 3,799,984 edges trapped — and nothing here measures an
/// edge count, so a document shorter than this ceiling carrying that many edges is not
/// excluded by the measurement. Failing input: exactly that document. Direction: refuses early
/// on size, never on shape, and can still trap on work. Escape hatch: raise it with a new
/// measurement, add an edge ceiling beside it, or fix the arena.
pub const MAX_INGEST_BYTES: usize = 536_870_912;

/// Why an ingest buffer was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestError {
    /// Not UTF-8.
    Utf8,
    /// Not JSON at all.
    Json(JsonError),
    /// JSON, but not this shape: dotted path and what was wrong.
    Shape(String),
    /// A duplicate node or edge id (C12: refused, not silently first-wins).
    DuplicateId { what: &'static str, id: String },
    /// An edge naming a node id that is not in `nodes` (C12: refused, not dropped).
    DanglingEndpoint {
        edge: String,
        end: &'static str,
        id: String,
    },
    /// Too many nodes or edges to index (`u32` capacity). Reachable only on a 64-bit host: on
    /// wasm32 `usize` is `u32` (F-79). A ceiling on the document's bytes is
    /// [`IngestError::TooLarge`], which is the one that bites on wasm32.
    Capacity,
    /// The buffer is longer than [`MAX_INGEST_BYTES`]: its bytes, and the limit it was held to.
    TooLarge { bytes: usize, limit: usize },
}

impl IngestError {
    /// The wire code this refusal is published under (C4): every ingest refusal is
    /// [`Code::IngestInvalid`] but the one the host can do something about — an oversized
    /// document is not malformed, and telling a caller the two are the same would send it
    /// looking for a bad member in a document it must instead split.
    pub fn code(&self) -> Code {
        match self {
            Self::TooLarge { .. } => Code::IngestTooLarge,
            _ => Code::IngestInvalid,
        }
    }
}

/// Parse and validate `bytes` into ingest order records, or the refusal.
///
/// The length is checked first, before [`std::str::from_utf8`] and before the parser is
/// given anything: a buffer past [`MAX_INGEST_BYTES`] is refused by its size alone, so no
/// work is done on a document this module has already promised not to read (F-16).
///
/// The tree is consumed by value: each node's and edge's element is moved in, each string
/// is moved out of its `Value` instead of copied with `.to_owned()`, and the element is
/// dropped as soon as its record is built. The order is unchanged from the borrowing
/// reader this replaced, and the differential test in [`differential`] is the judge: whole
/// text parsed before any shape check, root checked before any node, every node before any
/// edge, then `check_ids`.
pub fn read(bytes: &[u8]) -> Result<(Vec<NodeRecord>, Vec<EdgeRecord>), IngestError> {
    if bytes.len() > MAX_INGEST_BYTES {
        return Err(IngestError::TooLarge {
            bytes: bytes.len(),
            limit: MAX_INGEST_BYTES,
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|_| IngestError::Utf8)?;
    let Value::Object(mut root) = parse(text).map_err(IngestError::Json)? else {
        return Err(shape(At::ROOT, "expected an object"));
    };
    let version = version_number(&take_member(&mut root, "version", At::ROOT)?)?;
    if version != VERSION {
        return Err(shape(
            At::list("version"),
            &format!("unsupported version {version}"),
        ));
    }
    let nodes = take_array(&mut root, "nodes", At::ROOT, At::list("nodes"))?;
    let edges = take_array(&mut root, "edges", At::ROOT, At::list("edges"))?;
    require_only(
        root.iter().map(|(k, _)| k.as_str()),
        &["version", "nodes", "edges"],
        At::ROOT,
    )?;
    // Both lists are owned now, so the root is spent and only these two remain; each
    // element is dropped as its record is built.
    let nodes = read_all(nodes, node, At::list("nodes"))?;
    let edges = read_all(edges, edge, At::list("edges"))?;
    check_ids(&nodes, &edges)?;
    Ok((nodes, edges))
}

/// Each element, in order, through `one`. A `Vec` of exactly the right length, never grown.
fn read_all<T, F>(items: Vec<Value>, one: F, at: At) -> Result<Vec<T>, IngestError>
where
    F: Fn(Value, At) -> Result<T, IngestError>,
{
    let mut out = Vec::with_capacity(items.len());
    for (i, item) in items.into_iter().enumerate() {
        out.push(one(item, at.item(i))?);
    }
    Ok(out)
}

pub(in crate::ingest) fn shape(at: At, what: &str) -> IngestError {
    IngestError::Shape(format!("{at}: {what}"))
}

/// An object's members, each value in a slot that can be handed out exactly once: a value
/// is moved out rather than borrowed and copied, and a name taken twice comes back missing
/// rather than yielding the first one again. The keys stay put for `require_only`.
pub(in crate::ingest) struct Slots(Vec<(String, Option<Value>)>);

impl Slots {
    pub(in crate::ingest) fn new(value: Value, at: At) -> Result<Self, IngestError> {
        match value {
            Value::Object(members) => Ok(Slots(
                members.into_iter().map(|(k, v)| (k, Some(v))).collect(),
            )),
            _ => Err(shape(at, "expected an object")),
        }
    }

    /// A named member's value, moved out. The key's `String` is dropped with the slots.
    pub(in crate::ingest) fn take(&mut self, key: &str, at: At) -> Result<Value, IngestError> {
        self.slot(key)
            .ok_or_else(|| shape(at, &format!("missing member `{key}`")))
    }

    pub(in crate::ingest) fn take_optional(&mut self, key: &str) -> Option<Value> {
        self.slot(key)
    }

    fn slot(&mut self, key: &str) -> Option<Value> {
        let (_, value) = self.0.iter_mut().find(|(k, v)| k == key && v.is_some())?;
        value.take()
    }

    /// The member names, for `require_only`. A taken slot's value is `None` but its key is
    /// still named: the strictness is about what the document wrote, not what was read.
    pub(in crate::ingest) fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(k, _)| k.as_str())
    }
}

/// A named member of the root, moved out of it; `Null` fills the hole so a name taken
/// twice comes back missing rather than yielding the first value again.
fn take_member(root: &mut [(String, Value)], key: &str, at: At) -> Result<Value, IngestError> {
    let slot = root
        .iter_mut()
        .find(|(k, v)| k == key && !matches!(v, Value::Null))
        .map(|(_, v)| v)
        .ok_or_else(|| shape(at, &format!("missing member `{key}`")))?;
    Ok(std::mem::replace(slot, Value::Null))
}

/// A named member of the root that has to be an array, moved out whole so its elements can
/// be consumed one at a time. `at` names the member, `where_at` names the array.
fn take_array(
    root: &mut [(String, Value)],
    key: &str,
    at: At,
    where_at: At,
) -> Result<Vec<Value>, IngestError> {
    match take_member(root, key, at)? {
        Value::Array(items) => Ok(items),
        _ => Err(shape(where_at, "expected an array")),
    }
}

/// Refuses a member the shape does not name — the strictness that turns a stray
/// camelCase `hasNote` into a loud refusal instead of a silently-dropped extra.
fn require_only<'a>(
    members: impl Iterator<Item = &'a str>,
    allowed: &[&str],
    at: At,
) -> Result<(), IngestError> {
    for key in members {
        if !allowed.contains(&key) {
            return Err(shape(at, &format!("unknown member `{key}`")));
        }
    }
    Ok(())
}

pub(in crate::ingest) fn string(value: Value, at: At) -> Result<String, IngestError> {
    match value {
        Value::String(s) => Ok(s),
        _ => Err(shape(at, "expected a string")),
    }
}

pub(in crate::ingest) fn opt_string(value: Value, at: At) -> Result<Option<String>, IngestError> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s)),
        _ => Err(shape(at, "expected a string or null")),
    }
}

pub(in crate::ingest) fn boolean(value: Value, at: At) -> Result<bool, IngestError> {
    match value {
        Value::Bool(b) => Ok(b),
        _ => Err(shape(at, "expected a boolean")),
    }
}

/// The document's `version` member: exactly a plain non-negative integer literal, never
/// `1.0` or `1e0` read loosely as `1` — a version is compared for equality, not rounded.
fn version_number(value: &Value) -> Result<u32, IngestError> {
    let Value::Number(text) = value else {
        return Err(shape(At::list("version"), "expected a number"));
    };
    text.parse()
        .map_err(|_| shape(At::list("version"), "expected a plain non-negative integer"))
}

/// A JSON number, refusing one whose text does not parse to a finite `f64` (D9): the
/// grammar itself keeps out `NaN`/`Infinity` literals, but an exponent large enough to
/// overflow `f64` still parses its text and must be refused here, not on the wire later.
pub(in crate::ingest) fn number(value: Value, at: At) -> Result<f64, IngestError> {
    let Value::Number(text) = value else {
        return Err(shape(at, "expected a number"));
    };
    let n: f64 = text.parse().map_err(|_| shape(at, "not a valid number"))?;
    if !n.is_finite() {
        return Err(shape(at, "not finite"));
    }
    Ok(n)
}

#[cfg(test)]
mod differential;
#[cfg(test)]
mod tests;
