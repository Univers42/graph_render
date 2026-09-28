//! Provisional ingest (`docs/contract/wasm-abi.md` "Ingest — PROVISIONAL", C13): a
//! versioned JSON array of node/edge records in the shape `graph_core::records` already
//! uses, parsed with `graph_contract::canonical_json`'s strict RFC 8259 reader.
//!
//! **Phase 10 owns the real ingest contract.** This one exists so Phase 4 has something
//! to build `gm_build` against; it is deliberately narrow: every member is named and
//! required (a present `null` where a field may be absent, never an omitted key), an
//! unknown member refuses the whole document (so a stray `hasNote` — the oracle's own
//! camelCase — is refused loudly, not silently ignored), and `kind` strings are matched
//! by exact name (`NodeKind`/`EdgeKind::from_name`), never the lossy `edge_kind_from_type`
//! heuristic. Duplicate ids and dangling edges are refused outright (C12): the
//! alternative graph_core::index_model itself takes — first-wins, drop and forget — would
//! make ingest order diverge silently from snapshot order, which is exactly the identity
//! this ABI promises callers (`docs/contract/wasm-abi.md` "Column order").

use graph_contract::canonical_json::{JsonError, Value, parse};
use graph_core::{EdgeKind, EdgeRecord, NodeKind, NodeRecord};
use std::collections::BTreeSet;

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
    /// Too many nodes or edges to index (`u32` capacity).
    Capacity,
}

/// Parses and validates `bytes` into ingest order records, or the refusal.
pub fn read(bytes: &[u8]) -> Result<(Vec<NodeRecord>, Vec<EdgeRecord>), IngestError> {
    let text = std::str::from_utf8(bytes).map_err(|_| IngestError::Utf8)?;
    let document = parse(text).map_err(IngestError::Json)?;
    let root = object(&document, "")?;
    let version = version_number(member(root, "version", "")?)?;
    if version != VERSION {
        return Err(shape("version", &format!("unsupported version {version}")));
    }
    let nodes = array(member(root, "nodes", "")?, "nodes")?;
    let edges = array(member(root, "edges", "")?, "edges")?;
    require_only(root, &["version", "nodes", "edges"], "")?;
    let nodes: Vec<NodeRecord> = nodes
        .iter()
        .enumerate()
        .map(|(i, v)| node(v, &format!("nodes[{i}]")))
        .collect::<Result<_, _>>()?;
    let edges: Vec<EdgeRecord> = edges
        .iter()
        .enumerate()
        .map(|(i, v)| edge(v, &format!("edges[{i}]")))
        .collect::<Result<_, _>>()?;
    check_ids(&nodes, &edges)?;
    Ok((nodes, edges))
}

fn check_ids(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Result<(), IngestError> {
    let mut ids = BTreeSet::new();
    for n in nodes {
        if !ids.insert(n.id.as_str()) {
            return Err(IngestError::DuplicateId {
                what: "node",
                id: n.id.clone(),
            });
        }
    }
    let mut edge_ids = BTreeSet::new();
    for e in edges {
        if !edge_ids.insert(e.id.as_str()) {
            return Err(IngestError::DuplicateId {
                what: "edge",
                id: e.id.clone(),
            });
        }
        for (end, id) in [("source", &e.source), ("target", &e.target)] {
            if !ids.contains(id.as_str()) {
                return Err(IngestError::DanglingEndpoint {
                    edge: e.id.clone(),
                    end,
                    id: id.clone(),
                });
            }
        }
    }
    if u32::try_from(nodes.len()).is_err() || u32::try_from(edges.len()).is_err() {
        return Err(IngestError::Capacity);
    }
    Ok(())
}

fn node(value: &Value, path: &str) -> Result<NodeRecord, IngestError> {
    let fields = [
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
    let members = object(value, path)?;
    require_only(members, &fields, path)?;
    let kind_name = string(member(members, "kind", path)?, &format!("{path}.kind"))?;
    let kind = NodeKind::from_name(kind_name).ok_or_else(|| {
        shape(
            &format!("{path}.kind"),
            &format!("unknown node kind {kind_name:?}"),
        )
    })?;
    Ok(NodeRecord {
        id: string(member(members, "id", path)?, &format!("{path}.id"))?.to_owned(),
        kind,
        database_id: opt_string(
            member(members, "database_id", path)?,
            &format!("{path}.database_id"),
        )?,
        source: string(member(members, "source", path)?, &format!("{path}.source"))?.to_owned(),
        label: string(member(members, "label", path)?, &format!("{path}.label"))?.to_owned(),
        group: opt_string(member(members, "group", path)?, &format!("{path}.group"))?,
        weight: number(member(members, "weight", path)?, &format!("{path}.weight"))?,
        version: number(
            member(members, "version", path)?,
            &format!("{path}.version"),
        )?,
        has_note: boolean(
            member(members, "has_note", path)?,
            &format!("{path}.has_note"),
        )?,
        icon: opt_string(member(members, "icon", path)?, &format!("{path}.icon"))?,
    })
}

fn edge(value: &Value, path: &str) -> Result<EdgeRecord, IngestError> {
    let fields = [
        "id",
        "source",
        "target",
        "kind",
        "label",
        "strength",
        "directed",
        "record_id",
    ];
    let members = object(value, path)?;
    require_only(members, &fields, path)?;
    let kind_name = string(member(members, "kind", path)?, &format!("{path}.kind"))?;
    let kind = EdgeKind::from_name(kind_name).ok_or_else(|| {
        shape(
            &format!("{path}.kind"),
            &format!("unknown edge kind {kind_name:?}"),
        )
    })?;
    // A struct literal, no `..` (C13): p3's `child_first` field must break this build.
    Ok(EdgeRecord {
        id: string(member(members, "id", path)?, &format!("{path}.id"))?.to_owned(),
        source: string(member(members, "source", path)?, &format!("{path}.source"))?.to_owned(),
        target: string(member(members, "target", path)?, &format!("{path}.target"))?.to_owned(),
        kind,
        label: string(member(members, "label", path)?, &format!("{path}.label"))?.to_owned(),
        strength: number(
            member(members, "strength", path)?,
            &format!("{path}.strength"),
        )?,
        directed: boolean(
            member(members, "directed", path)?,
            &format!("{path}.directed"),
        )?,
        record_id: opt_string(
            member(members, "record_id", path)?,
            &format!("{path}.record_id"),
        )?,
    })
}

fn shape(path: &str, what: &str) -> IngestError {
    IngestError::Shape(format!("{path}: {what}"))
}

fn object<'a>(value: &'a Value, path: &str) -> Result<&'a [(String, Value)], IngestError> {
    match value {
        Value::Object(members) => Ok(members),
        _ => Err(shape(path, "expected an object")),
    }
}

fn array<'a>(value: &'a Value, path: &str) -> Result<&'a [Value], IngestError> {
    match value {
        Value::Array(items) => Ok(items),
        _ => Err(shape(path, "expected an array")),
    }
}

fn member<'a>(
    members: &'a [(String, Value)],
    key: &str,
    path: &str,
) -> Result<&'a Value, IngestError> {
    members
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .ok_or_else(|| shape(path, &format!("missing member `{key}`")))
}

/// Refuses a member the shape does not name — the strictness that turns a stray
/// camelCase `hasNote` into a loud refusal instead of a silently-dropped extra.
fn require_only(
    members: &[(String, Value)],
    allowed: &[&str],
    path: &str,
) -> Result<(), IngestError> {
    for (key, _) in members {
        if !allowed.contains(&key.as_str()) {
            return Err(shape(path, &format!("unknown member `{key}`")));
        }
    }
    Ok(())
}

fn string<'a>(value: &'a Value, path: &str) -> Result<&'a str, IngestError> {
    match value {
        Value::String(s) => Ok(s),
        _ => Err(shape(path, "expected a string")),
    }
}

fn opt_string(value: &Value, path: &str) -> Result<Option<String>, IngestError> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => Err(shape(path, "expected a string or null")),
    }
}

fn boolean(value: &Value, path: &str) -> Result<bool, IngestError> {
    match value {
        Value::Bool(b) => Ok(*b),
        _ => Err(shape(path, "expected a boolean")),
    }
}

/// The document's `version` member: exactly a plain non-negative integer literal, never
/// `1.0` or `1e0` read loosely as `1` — a version is compared for equality, not rounded.
fn version_number(value: &Value) -> Result<u32, IngestError> {
    let Value::Number(text) = value else {
        return Err(shape("version", "expected a number"));
    };
    text.parse()
        .map_err(|_| shape("version", "expected a plain non-negative integer"))
}

/// A JSON number, refusing one whose text does not parse to a finite `f64` (D9): the
/// grammar itself keeps out `NaN`/`Infinity` literals, but an exponent large enough to
/// overflow `f64` still parses its text and must be refused here, not on the wire later.
fn number(value: &Value, path: &str) -> Result<f64, IngestError> {
    let Value::Number(text) = value else {
        return Err(shape(path, "expected a number"));
    };
    let n: f64 = text
        .parse()
        .map_err(|_| shape(path, "not a valid number"))?;
    if !n.is_finite() {
        return Err(shape(path, "not finite"));
    }
    Ok(n)
}

#[cfg(test)]
mod tests;
