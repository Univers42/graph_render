//! The generated documents, and the `n220` fixture's own records.
//!
//! Two corpora, both as *records* (see `corpus`): `fixtures/scale/n220.json`, read by the
//! reference reader exactly as it reads any other document, and a set of small documents
//! built here to hold every field class the decision record names. The fixture proves the
//! path agrees on a real corpus; the generated set proves it agrees on the awkward values a
//! real corpus does not happen to contain.

use graph_core::{EdgeKind, EdgeRecord, NodeKind, NodeRecord};

use super::corpus::Document;
use super::json::document_json;

/// The corpus: the fixture first, then the generated documents.
pub fn all() -> Vec<Document> {
    let mut out = vec![fixture()];
    out.extend(generated());
    out
}

/// `fixtures/scale/n220.json`, read by the reference path. 220 nodes, 329 edges, five edge
/// kinds, multi-byte icons and `:`-bearing ids.
fn fixture() -> Document {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/scale/n220.json"
    );
    let bytes = std::fs::read(path).expect("n220 is a committed fixture");
    let (nodes, edges) = crate::ingest::read_records(&bytes).expect("n220 is an ingest document");
    Document {
        name: "fixtures/scale/n220.json",
        json: String::from_utf8(bytes).expect("n220 is UTF-8"),
        nodes,
        edges,
    }
}

/// One node, with everything but the id defaulted.
fn node(id: &str) -> NodeRecord {
    NodeRecord {
        id: id.to_owned(),
        kind: NodeKind::Record,
        database_id: Some("db-0".to_owned()),
        source: "pg".to_owned(),
        label: format!("L{id}"),
        group: Some("Active".to_owned()),
        weight: 0.5,
        version: 0.0,
        has_note: false,
        icon: Some("🌿".to_owned()),
    }
}

/// One edge between two ids.
fn edge(id: &str, source: &str, target: &str) -> EdgeRecord {
    EdgeRecord {
        id: id.to_owned(),
        source: source.to_owned(),
        target: target.to_owned(),
        kind: EdgeKind::Relation,
        label: String::new(),
        strength: 0.5,
        directed: false,
        record_id: None,
        child_first: false,
    }
}

/// The generated set: every node kind and every edge kind, one document per axis, and one
/// document of awkward scalars. Small on purpose — a failure must be readable.
fn generated() -> Vec<Document> {
    vec![kinds(), optionals(), scalars()]
}

/// One node of each kind and one edge of each kind, wired so no edge dangles.
fn kinds() -> Document {
    let mut nodes = vec![node("k-record")];
    let names = ["note", "database", "tag"];
    for (i, kind) in NodeKind::ALL.into_iter().skip(1).enumerate() {
        let mut n = node(&format!("k-{}", kind.as_str()));
        n.kind = kind;
        n.label = names[i].to_owned();
        nodes.push(n);
    }
    let mut edges = Vec::new();
    for (i, kind) in EdgeKind::ALL.into_iter().enumerate() {
        let mut e = edge(&format!("k-e{i}"), "k-record", "k-note");
        e.kind = kind;
        e.label = kind.as_str().to_owned();
        e.child_first = i == 4;
        edges.push(e);
    }
    document("kinds", nodes, edges)
}

/// Optional present and absent on the same kind of node, and a `record_id` present.
fn optionals() -> Document {
    let mut with = node("o-with");
    with.database_id = Some("db-9".to_owned());
    with.group = Some("Epsilon".to_owned());
    with.icon = Some("📌".to_owned());
    with.has_note = true;
    let mut without = node("o-without");
    without.database_id = None;
    without.group = None;
    without.icon = None;
    without.has_note = false;
    let mut e = edge("o-e0", "o-with", "o-without");
    e.record_id = Some("rec-7".to_owned());
    let mut f = edge("o-e1", "o-without", "o-with");
    f.record_id = None;
    document("optionals", vec![with, without], vec![e, f])
}

/// The scalars a naive encoder loses: a multi-byte id, a nonzero version, a negative weight,
/// a `-0.0` weight, a subnormal strength, an undirected edge and a child-first edge.
fn scalars() -> Document {
    let mut head = node("s-éè-🌿");
    head.weight = -0.0;
    head.version = 7.0;
    head.label = String::new();
    let mut body = node("s-neg");
    body.weight = -1.5;
    body.version = -0.25;
    let mut sub = node("s-sub");
    sub.weight = f64::from_bits(1);
    let mut e = edge("s-e0", "s-éè-🌿", "s-neg");
    e.strength = f64::from_bits(1);
    e.directed = false;
    e.child_first = false;
    let mut f = edge("s-e1", "s-sub", "s-éè-🌿");
    f.child_first = true;
    f.directed = true;
    document("scalars", vec![head, body, sub], vec![e, f])
}

/// One corpus document: the records, and the JSON that spells them.
fn document(name: &'static str, nodes: Vec<NodeRecord>, edges: Vec<EdgeRecord>) -> Document {
    let json = document_json(&nodes, &edges);
    Document {
        name,
        json,
        nodes,
        edges,
    }
}
