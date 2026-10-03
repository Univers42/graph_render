//! Today's `ingest::read`, frozen before the tree was consumed by value, kept whole and
//! borrowing: the differential test's oracle. It must not be "improved" — if the new
//! reader and this disagree, one of them is wrong and the test says which input.
//!
//! Copied from `ingest.rs` as it stood: same order (root, then version, then `nodes`,
//! then `edges`, then the strict member check, then every node, then every edge, then
//! `check_ids`), same error text, same borrowing accessors.

use crate::ingest::at::At;
use crate::ingest::ids::check_ids;
use crate::ingest::{IngestError, VERSION};
use graph_contract::canonical_json::{Value, parse};
use graph_core::{EdgeKind, EdgeRecord, NodeKind, NodeRecord};

/// The reference reader: bytes to the same `Result` the shipped reader produced.
pub(super) fn read(bytes: &[u8]) -> Result<(Vec<NodeRecord>, Vec<EdgeRecord>), IngestError> {
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
    Ok(EdgeRecord {
        id: string(member(members, "id", at)?, at.field("id"))?.to_owned(),
        source: string(member(members, "source", at)?, at.field("source"))?.to_owned(),
        target: string(member(members, "target", at)?, at.field("target"))?.to_owned(),
        kind,
        label: string(member(members, "label", at)?, at.field("label"))?.to_owned(),
        strength: number(member(members, "strength", at)?, at.field("strength"))?,
        directed: boolean(member(members, "directed", at)?, at.field("directed"))?,
        record_id: opt_string(member(members, "record_id", at)?, at.field("record_id"))?,
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

fn version_number(value: &Value) -> Result<u32, IngestError> {
    let Value::Number(text) = value else {
        return Err(shape(At::list("version"), "expected a number"));
    };
    text.parse()
        .map_err(|_| shape(At::list("version"), "expected a plain non-negative integer"))
}

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
