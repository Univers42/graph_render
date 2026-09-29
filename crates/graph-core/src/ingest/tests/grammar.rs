//! The node-id grammar (H5) — the constraint, enforced and pinned.
//!
//! The gate row is `cargo test -p graph-core ingest_id_grammar`, so these tests carry
//! that name in theirs.

use super::support::*;

// ------------------------------------------------------------ the id grammar

/// The gate row is `cargo test -p graph-core ingest_id_grammar`, so the grammar's own
/// tests carry that name in theirs.
#[test]
fn ingest_id_grammar_refuses_a_colon_in_source_or_a_collection_id() {
    // Enforced by the contract's own reader, so the gate row
    // `cargo test -p graph-core ingest_id_grammar` has something to name. A caller that
    // builds an `Ingest` in Rust and bypasses the reader is opting out knowingly: the
    // Ponytail on `parse_node_id` says exactly that, and names this as the escape hatch
    // for the other direction.
    let document = |source: &str| {
        format!(r#"{{"version":1,"source":"{source}","collections":[],"records":[]}}"#)
    };
    assert_eq!(
        graph_contract::ingest::read(&document("my:source"))
            .expect_err("H5")
            .to_string(),
        "source \"my:source\" contains `:` and cannot round-trip through the node-id \
         grammar (H5): the id would parse back shifted"
    );
    let with_collection = |id: &str| {
        format!(
            r#"{{"version":1,"source":"s","collections":[
                {{"id":"{id}","name":"C","titleField":"t",
                  "fields":[{{"id":"t","name":"T","role":"title","link":null}}]}}],
               "records":[]}}"#
        )
    };
    assert!(
        graph_contract::ingest::read(&with_collection("my:task"))
            .expect_err("H5")
            .to_string()
            .starts_with("collection id \"my:task\""),
        "the refusal must name the coordinate"
    );
}

#[test]
fn ingest_id_grammar_allows_a_colon_in_a_record_id() {
    let mut doc = one_of_each();
    doc.records[0].id = "r:1".into();
    let id = build(&doc).expect("derives").nodes[0].id.clone();
    assert_eq!(id, "rows:task:r:1");
    let parsed = parse_node_id(&id).expect("a record id");
    assert_eq!(
        (parsed.source, parsed.database_id, parsed.record_id),
        ("rows", "task", "r:1")
    );
}

#[test]
fn ingest_id_grammar_refuses_a_colon_in_a_tag_value() {
    let mut doc = one_of_each();
    *cell(&mut doc.records[0], "labels") = JsonValue::List(vec![JsonValue::Text("a:b".into())]);
    let refusal = build(&doc)
        .expect_err("a colon in a tag value is refused")
        .to_string();
    assert_eq!(
        refusal,
        "tag value \"a:b\" contains `:` and cannot round-trip through the node-id \
         grammar (H5): the id would parse back shifted"
    );
}

#[test]
fn every_derived_record_node_id_round_trips_through_the_parser() {
    let mut doc = one_of_each();
    doc.source = "pg".into();
    doc.records[0].id = "a:b:c".into();
    let graph = build(&doc).expect("derives");
    for node in graph.nodes.iter().filter(|n| n.kind == NodeKind::Record) {
        let parsed =
            parse_node_id(&node.id).unwrap_or_else(|| panic!("{} does not parse back", node.id));
        assert_eq!(parsed.source, "pg");
        assert_eq!(parsed.database_id, "task");
        assert_eq!(parsed.record_id, "a:b:c");
    }
    // A tag node is not a record id and must not pretend to be one.
    for node in graph.nodes.iter().filter(|n| n.kind == NodeKind::Tag) {
        assert!(parse_node_id(&node.id).is_none(), "{}", node.id);
    }
}

// ------------------------------------------------------------------ refusals

#[test]
fn a_record_naming_a_collection_that_does_not_exist_is_refused_not_dropped() {
    let mut doc = one_of_each();
    doc.records[0].collection = "nope".into();
    assert_eq!(
        build(&doc)
            .expect_err("a dangling collection is a refusal")
            .to_string(),
        "record `r1`: no collection with id `nope`"
    );
}

#[test]
fn a_link_to_a_collection_that_is_not_declared_is_refused_before_any_record_is_derived() {
    let mut doc = one_of_each();
    doc.collections[0].fields[7]
        .link
        .as_mut()
        .expect("declared")
        .collection = "nope".into();
    assert_eq!(
        build(&doc)
            .expect_err("a link to nowhere is a refusal")
            .to_string(),
        "field `blocks` of collection `task`: links to collection `nope`, which is not \
         declared"
    );
}

#[test]
fn a_record_declared_twice_is_refused_rather_than_first_wins() {
    let mut doc = one_of_each();
    doc.records.push(doc.records[0].clone());
    assert_eq!(
        build(&doc)
            .expect_err("a duplicate record is a refusal")
            .to_string(),
        "record `r1`: declared twice"
    );
}
