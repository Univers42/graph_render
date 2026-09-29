//! The hierarchy fixtures (`fixtures/hierarchy/*.json`) as records, for the tests of
//! every layout that reads a [`Hierarchy`](super::Hierarchy).
//!
//! A fixture is `{"about", "nodes": [{"id", "weight"}], "edges": [{"id", "source",
//! "target", "type"}]}`: an edge keeps its raw wire `type`, so the oracle arms can apply
//! their own `child_of` flip, and here it is classified by the same two functions the
//! ingest uses, [`edge_kind_from_type`] and [`child_first_from_type`]. Every other record
//! field takes a fixed value.

use crate::columns::NodeKind;
use crate::edgekind::{child_first_from_type, edge_kind_from_type};
use crate::records::{EdgeRecord, NodeRecord};
use graph_contract::canonical_json::{Value, parse};

/// Every fixture, by name, and its text.
pub const FIXTURES: [(&str, &str); 4] = [
    (
        "tree-balanced",
        include_str!("../../../../../fixtures/hierarchy/tree-balanced.json"),
    ),
    (
        "tree-degenerate",
        include_str!("../../../../../fixtures/hierarchy/tree-degenerate.json"),
    ),
    (
        "forest",
        include_str!("../../../../../fixtures/hierarchy/forest.json"),
    ),
    (
        "cyclic",
        include_str!("../../../../../fixtures/hierarchy/cyclic.json"),
    ),
];

/// Fixture `name`'s records, in file order. Panics on a malformed fixture.
pub fn load(name: &str) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    let (_, text) = FIXTURES
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("no fixture {name}"));
    let root = parse(text).unwrap_or_else(|e| panic!("{name}: {e}"));
    let nodes = array(&root, "nodes").iter().map(node).collect();
    let edges = array(&root, "edges").iter().map(edge).collect();
    (nodes, edges)
}

fn node(value: &Value) -> NodeRecord {
    let weight = match member(value, "weight") {
        Value::Number(text) => text.parse().expect("a weight"),
        other => panic!("weight {other:?}"),
    };
    let id = text(value, "id");
    NodeRecord {
        kind: NodeKind::Record,
        database_id: None,
        source: "fixture".into(),
        label: id.clone(),
        group: None,
        weight,
        version: 0.0,
        has_note: false,
        icon: None,
        id,
    }
}

fn edge(value: &Value) -> EdgeRecord {
    let wire_type = text(value, "type");
    EdgeRecord {
        id: text(value, "id"),
        source: text(value, "source"),
        target: text(value, "target"),
        kind: edge_kind_from_type(Some(&wire_type)),
        child_first: child_first_from_type(Some(&wire_type)),
        label: wire_type,
        strength: 1.0,
        directed: true,
        record_id: None,
    }
}

fn member<'a>(value: &'a Value, key: &str) -> &'a Value {
    let Value::Object(members) = value else {
        panic!("not an object: {value:?}");
    };
    members
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .unwrap_or_else(|| panic!("no {key}"))
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    match member(value, key) {
        Value::Array(items) => items,
        other => panic!("{key}: {other:?}"),
    }
}

fn text(value: &Value, key: &str) -> String {
    match member(value, key) {
        Value::String(text) => text.clone(),
        other => panic!("{key}: {other:?}"),
    }
}
