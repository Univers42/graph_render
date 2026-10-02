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

mod at;
mod ids;
use at::At;
use ids::check_ids;

/// The only ingest version this reader accepts.
pub const VERSION: u32 = 1;

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
    /// wasm32 `usize` is `u32` (F-79). A ceiling below that is F-16's, not decided here.
    Capacity,
}

/// Parses and validates `bytes` into ingest order records, or the refusal.
pub fn read(bytes: &[u8]) -> Result<(Vec<NodeRecord>, Vec<EdgeRecord>), IngestError> {
    let text = std::str::from_utf8(bytes).map_err(|_| IngestError::Utf8)?;
    let document = parse(text).map_err(IngestError::Json)?;
    let root = object(&document, At::ROOT)?;
    let version = version_number(member(root, "version", At::ROOT)?)?;
    if version != VERSION {
        return Err(shape(
            At::list("version"),
            &format!("unsupported version {version}"),
        ));
    }
    let nodes = array(member(root, "nodes", At::ROOT)?, At::list("nodes"))?;
    let edges = array(member(root, "edges", At::ROOT)?, At::list("edges"))?;
    require_only(root, &["version", "nodes", "edges"], At::ROOT)?;
    let nodes: Vec<NodeRecord> = nodes
        .iter()
        .enumerate()
        .map(|(i, v)| node(v, At::list("nodes").item(i)))
        .collect::<Result<_, _>>()?;
    let edges: Vec<EdgeRecord> = edges
        .iter()
        .enumerate()
        .map(|(i, v)| edge(v, At::list("edges").item(i)))
        .collect::<Result<_, _>>()?;
    check_ids(&nodes, &edges)?;
    Ok((nodes, edges))
}

/// Field names `node`/`edge` require, exactly (`require_only`). Module-level rather than
/// a `let` inside each function (house limit: the array itself was most of what pushed
/// both functions past 40 lines).
const NODE_FIELDS: [&str; 10] = [
    "id",
    "kind",
    "database_id",
    "source",
    "label",
    "group",
    "weight",
    "version",
    "has_note",
    "icon",
];

const EDGE_FIELDS: [&str; 9] = [
    "id",
    "source",
    "target",
    "kind",
    "label",
    "strength",
    "directed",
    "record_id",
    "child_first",
];

fn node(value: &Value, at: At) -> Result<NodeRecord, IngestError> {
    let members = object(value, at)?;
    require_only(members, &NODE_FIELDS, at)?;
    let kind_name = string(member(members, "kind", at)?, at.field("kind"))?;
    let kind = NodeKind::from_name(kind_name).ok_or_else(|| {
        shape(
            at.field("kind"),
            &format!("unknown node kind {kind_name:?}"),
        )
    })?;
    Ok(NodeRecord {
        id: string(member(members, "id", at)?, at.field("id"))?.to_owned(),
        kind,
        database_id: opt_string(member(members, "database_id", at)?, at.field("database_id"))?,
        source: string(member(members, "source", at)?, at.field("source"))?.to_owned(),
        label: string(member(members, "label", at)?, at.field("label"))?.to_owned(),
        group: opt_string(member(members, "group", at)?, at.field("group"))?,
        weight: number(member(members, "weight", at)?, at.field("weight"))?,
        version: number(member(members, "version", at)?, at.field("version"))?,
        has_note: boolean(member(members, "has_note", at)?, at.field("has_note"))?,
        icon: opt_string(member(members, "icon", at)?, at.field("icon"))?,
    })
}

fn edge(value: &Value, at: At) -> Result<EdgeRecord, IngestError> {
    let members = object(value, at)?;
    require_only(members, &EDGE_FIELDS, at)?;
    let kind_name = string(member(members, "kind", at)?, at.field("kind"))?;
    let kind = EdgeKind::from_name(kind_name).ok_or_else(|| {
        shape(
            at.field("kind"),
            &format!("unknown edge kind {kind_name:?}"),
        )
    })?;
    // A struct literal, no `..` (C13): p3's `child_first` field must break this build.
    Ok(EdgeRecord {
        id: string(member(members, "id", at)?, at.field("id"))?.to_owned(),
        source: string(member(members, "source", at)?, at.field("source"))?.to_owned(),
        target: string(member(members, "target", at)?, at.field("target"))?.to_owned(),
        kind,
        label: string(member(members, "label", at)?, at.field("label"))?.to_owned(),
        strength: number(member(members, "strength", at)?, at.field("strength"))?,
        directed: boolean(member(members, "directed", at)?, at.field("directed"))?,
        record_id: opt_string(member(members, "record_id", at)?, at.field("record_id"))?,
        // Optional: an edge document written before p3's hierarchy direction reads as
        // parent-first, the same default as `graph-cli`'s `oracle_fixtures/wire.rs`.
        child_first: match members.iter().find(|(k, _)| k == "child_first") {
            Some((_, value)) => boolean(value, at.field("child_first"))?,
            None => false,
        },
    })
}

fn shape(at: At, what: &str) -> IngestError {
    IngestError::Shape(format!("{at}: {what}"))
}

fn object(value: &Value, at: At) -> Result<&[(String, Value)], IngestError> {
    match value {
        Value::Object(members) => Ok(members),
        _ => Err(shape(at, "expected an object")),
    }
}

fn array(value: &Value, at: At) -> Result<&[Value], IngestError> {
    match value {
        Value::Array(items) => Ok(items),
        _ => Err(shape(at, "expected an array")),
    }
}

fn member<'a>(members: &'a [(String, Value)], key: &str, at: At) -> Result<&'a Value, IngestError> {
    members
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .ok_or_else(|| shape(at, &format!("missing member `{key}`")))
}

/// Refuses a member the shape does not name — the strictness that turns a stray
/// camelCase `hasNote` into a loud refusal instead of a silently-dropped extra.
fn require_only(members: &[(String, Value)], allowed: &[&str], at: At) -> Result<(), IngestError> {
    for (key, _) in members {
        if !allowed.contains(&key.as_str()) {
            return Err(shape(at, &format!("unknown member `{key}`")));
        }
    }
    Ok(())
}

fn string(value: &Value, at: At) -> Result<&str, IngestError> {
    match value {
        Value::String(s) => Ok(s),
        _ => Err(shape(at, "expected a string")),
    }
}

fn opt_string(value: &Value, at: At) -> Result<Option<String>, IngestError> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => Err(shape(at, "expected a string or null")),
    }
}

fn boolean(value: &Value, at: At) -> Result<bool, IngestError> {
    match value {
        Value::Bool(b) => Ok(*b),
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
fn number(value: &Value, at: At) -> Result<f64, IngestError> {
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
mod tests;
