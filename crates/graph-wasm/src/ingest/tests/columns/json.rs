//! The JSON writer for the generated documents. `corpus` holds the records and what they
//! cover; this turns them into the exact provisional-ingest text `read_records` speaks, so
//! the reference path and the columnar path start from one spelling of each graph.

use graph_core::{EdgeRecord, NodeRecord};

/// A `f64` as the JSON the reader parses: `-0.0` and a subnormal must survive the round
/// trip, and neither has a shorter spelling that a formatter might "helpfully" round.
fn number(value: f64) -> String {
    debug_assert!(value.is_finite(), "the reader refuses a non-finite number");
    // `{value}` is Rust's shortest round-tripping form, and it keeps `-0` as `-0` and a
    // subnormal in exponent form — both of which `f64::from_str` reads back exactly.
    value.to_string()
}

/// A JSON string, escaped. Only the characters this corpus can produce are escaped; the
/// ids are ASCII except the one multi-byte id, which needs no escape at all.
fn text(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// A string or `null`.
fn optional(value: Option<&str>) -> String {
    value.map_or_else(|| "null".to_owned(), text)
}

/// The node, in the member order `ingest::record::NODE_FIELDS` requires.
pub fn node_json(node: &NodeRecord) -> String {
    let members = [
        ("id", text(&node.id)),
        ("kind", text(node.kind.as_str())),
        ("database_id", optional(node.database_id.as_deref())),
        ("source", text(&node.source)),
        ("label", text(&node.label)),
        ("group", optional(node.group.as_deref())),
        ("weight", number(node.weight)),
        ("version", number(node.version)),
        ("has_note", node.has_note.to_string()),
        ("icon", optional(node.icon.as_deref())),
    ];
    format!("{{{}}}", join(&members))
}

/// The edge, in the member order `ingest::record::EDGE_FIELDS` requires.
pub fn edge_json(edge: &EdgeRecord) -> String {
    let members = [
        ("id", text(&edge.id)),
        ("source", text(&edge.source)),
        ("target", text(&edge.target)),
        ("kind", text(edge.kind.as_str())),
        ("label", text(&edge.label)),
        ("strength", number(edge.strength)),
        ("directed", edge.directed.to_string()),
        ("record_id", optional(edge.record_id.as_deref())),
        ("child_first", edge.child_first.to_string()),
    ];
    format!("{{{}}}", join(&members))
}

/// A whole document, version 1.
pub fn document_json(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> String {
    let all: Vec<String> = nodes
        .iter()
        .map(node_json)
        .chain(edges.iter().map(edge_json))
        .collect();
    format!(
        "{{\"version\":1,\"nodes\":[{}],\"edges\":[{}]}}",
        all[..nodes.len()].join(","),
        all[nodes.len()..].join(",")
    )
}

/// `key: value` for each member, comma separated — the one place the two writers differ
/// from each other, so both call it.
fn join(members: &[(&str, String)]) -> String {
    members
        .iter()
        .map(|(key, value)| format!("\"{key}\":{value}"))
        .collect::<Vec<_>>()
        .join(",")
}
