use super::*;

fn node_json(id: &str) -> String {
    format!(
        r#"{{"id":"{id}","kind":"record","database_id":null,"source":"pg","label":"L","group":null,"weight":0.5,"version":0.0,"has_note":false,"icon":null}}"#
    )
}

fn edge_json(id: &str, source: &str, target: &str) -> String {
    format!(
        r#"{{"id":"{id}","source":"{source}","target":"{target}","kind":"relation","label":"","strength":0.5,"directed":false,"record_id":null}}"#
    )
}

fn doc(nodes: &[String], edges: &[String]) -> String {
    format!(
        r#"{{"version":1,"nodes":[{}],"edges":[{}]}}"#,
        nodes.join(","),
        edges.join(",")
    )
}

#[test]
fn a_minimal_document_round_trips_into_records_in_ingest_order() {
    let text = doc(
        &[node_json("a"), node_json("b")],
        &[edge_json("e", "a", "b")],
    );
    let (nodes, edges) = read(text.as_bytes()).expect("valid");
    assert_eq!(
        nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(edges.len(), 1);
    assert_eq!(
        (edges[0].source.as_str(), edges[0].target.as_str()),
        ("a", "b")
    );
    assert_eq!(nodes[0].kind, NodeKind::Record);
    assert_eq!(edges[0].kind, EdgeKind::Relation);
}

#[test]
fn empty_nodes_and_edges_is_a_valid_n_equals_zero_document() {
    let (nodes, edges) = read(doc(&[], &[]).as_bytes()).expect("valid, empty");
    assert!(nodes.is_empty() && edges.is_empty());
}

#[test]
fn not_utf8_is_refused_before_json_parsing() {
    assert_eq!(read(&[0xff, 0xfe]), Err(IngestError::Utf8));
}

#[test]
fn not_json_is_refused_as_json_not_shape() {
    assert!(matches!(read(b"not json"), Err(IngestError::Json(_))));
}

#[test]
fn an_unsupported_version_is_refused_by_name() {
    let text = r#"{"version":2,"nodes":[],"edges":[]}"#;
    let err = read(text.as_bytes()).expect_err("v2 unsupported");
    assert!(
        matches!(&err, IngestError::Shape(m) if m.contains("unsupported version 2")),
        "{err:?}"
    );
}

#[test]
fn a_loose_version_number_is_refused_not_rounded() {
    let text = r#"{"version":1.0,"nodes":[],"edges":[]}"#;
    assert!(
        matches!(read(text.as_bytes()), Err(IngestError::Shape(_))),
        "1.0 is not the plain integer 1"
    );
}

#[test]
fn a_missing_required_member_is_refused_by_name() {
    let text = r#"{"version":1,"nodes":[],"edges":[]"#; // truncated, also bad JSON
    assert!(read(text.as_bytes()).is_err());
    let missing_edges = r#"{"version":1,"nodes":[]}"#;
    let err = read(missing_edges.as_bytes()).expect_err("no edges member");
    assert!(
        matches!(&err, IngestError::Shape(m) if m.contains("missing member `edges`")),
        "{err:?}"
    );
}

#[test]
fn camelcase_has_note_is_refused_loudly_not_silently_defaulted() {
    let bad_node = r#"{"id":"a","kind":"record","database_id":null,"source":"pg","label":"L","group":null,"weight":0.5,"version":0.0,"hasNote":false,"icon":null}"#;
    let text = doc(&[bad_node.to_owned()], &[]);
    let err = read(text.as_bytes()).expect_err("hasNote is unknown, has_note is missing");
    // require_only sees `hasNote` before member() would report `has_note` missing.
    assert!(
        matches!(&err, IngestError::Shape(m) if m.contains("unknown member `hasNote`")),
        "{err:?}"
    );
}

#[test]
fn an_unknown_member_anywhere_is_refused() {
    let bad_node = node_json("a").replace('}', r#","extra":1}"#);
    let text = doc(&[bad_node], &[]);
    let err = read(text.as_bytes()).expect_err("extra member");
    assert!(
        matches!(&err, IngestError::Shape(m) if m.contains("unknown member `extra`")),
        "{err:?}"
    );
}

#[test]
fn node_and_edge_kinds_are_matched_by_strict_name_not_the_wire_heuristic() {
    let bad = node_json("a").replace("record", "Record"); // wrong case: strict, not lowered
    let text = doc(&[bad], &[]);
    let err = read(text.as_bytes()).expect_err("Record with capital R is not a known name");
    assert!(
        matches!(&err, IngestError::Shape(m) if m.contains("unknown node kind")),
        "{err:?}"
    );

    let heuristic_only = doc(
        &[node_json("a"), node_json("b")],
        &[edge_json("e", "a", "b").replace("relation", "child-of")],
    );
    let err = read(heuristic_only.as_bytes()).expect_err("child-of is not a strict EdgeKind name");
    assert!(
        matches!(&err, IngestError::Shape(m) if m.contains("unknown edge kind")),
        "{err:?}"
    );
}

#[test]
fn a_duplicate_node_id_is_refused_not_first_wins() {
    let text = doc(&[node_json("a"), node_json("a")], &[]);
    assert_eq!(
        read(text.as_bytes()),
        Err(IngestError::DuplicateId {
            what: "node",
            id: "a".into()
        })
    );
}

#[test]
fn a_duplicate_edge_id_is_refused() {
    let text = doc(
        &[node_json("a"), node_json("b")],
        &[edge_json("e", "a", "b"), edge_json("e", "b", "a")],
    );
    assert_eq!(
        read(text.as_bytes()),
        Err(IngestError::DuplicateId {
            what: "edge",
            id: "e".into()
        })
    );
}

#[test]
fn a_dangling_endpoint_is_refused_not_dropped() {
    let text = doc(&[node_json("a")], &[edge_json("e", "a", "ghost")]);
    assert_eq!(
        read(text.as_bytes()),
        Err(IngestError::DanglingEndpoint {
            edge: "e".into(),
            end: "target",
            id: "ghost".into()
        })
    );
}

#[test]
fn a_non_finite_number_is_refused() {
    let text = doc(&[node_json("a").replace("0.5", "1e400")], &[]);
    let err = read(text.as_bytes()).expect_err("overflows f64 to infinity");
    assert!(
        matches!(&err, IngestError::Shape(m) if m.contains("not finite")),
        "{err:?}"
    );
}

#[test]
fn null_is_accepted_for_optional_string_members_and_a_number_is_refused_there() {
    let with_group = node_json("a").replace(r#""group":null"#, r#""group":"g1""#);
    let (nodes, _) = read(doc(&[with_group], &[]).as_bytes()).expect("valid");
    assert_eq!(nodes[0].group.as_deref(), Some("g1"));

    let bad = node_json("a").replace(r#""group":null"#, r#""group":1"#);
    assert!(matches!(
        read(doc(&[bad], &[]).as_bytes()),
        Err(IngestError::Shape(_))
    ));
}

mod ceiling;
mod child_first;
mod extremes;
mod index;
