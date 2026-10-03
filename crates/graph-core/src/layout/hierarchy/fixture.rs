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

/// One fixture node's `"weight"`, parsed and **refused when non-finite**.
///
/// The reachable non-finite weight is an overflowing exponent, not a `NaN` literal: the
/// canonical JSON number grammar admits no `NaN`/`Infinity` token, so `"weight": 1e999`
/// parses to `Number("1e999")` and `f64::from_str` turns that into `+inf`. That value would
/// flow into every layout's aggregate and partition (`treemap::rows::node_values` sums it,
/// `hierarchy::depth` never sees it), where `+inf` collapses a comparison or a ratio. The
/// layouts' own weight clamps are not the boundary: `WEIGHT_EPSILON` and friends would
/// hide a bad fixture behind a plausible picture, so the fixture boundary refuses it and
/// the message names the node, which is the only place a reader can act.
///
/// Refused by panic like every other malformed field here — this module is `#[cfg(test)]`
/// and test data has no error channel. `load`'s signature stays as it is; a fallible
/// fixture loader is a separate decision, not this finding.
fn weight(value: &Value, id: &str) -> f64 {
    let text = match member(value, "weight") {
        Value::Number(text) => text,
        other => panic!("{id}: weight {other:?}"),
    };
    let parsed: f64 = text
        .parse()
        .unwrap_or_else(|_| panic!("{id}: weight {text:?}"));
    assert!(parsed.is_finite(), "{id}: weight {text:?} is not finite");
    parsed
}

fn node(value: &Value) -> NodeRecord {
    let id = text(value, "id");
    let weight = weight(value, &id);
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

#[cfg(test)]
mod tests;
