//! The phase's proof: two source shapes, one contract document, one graph.

use super::fixture_write;
use super::support::*;

// ------------------------------------------------------- the convergence pair

/// `fixtures/ingest/expected-graph.json`, read at compile time: a missing or renamed
/// fixture is a build failure, not a test that quietly finds nothing to check.
///
/// Two members, because the phase's proof has two halves and they run in two runtimes:
/// `ingest` is the contract document both adapters must produce from their own source
/// shape (checked by `harness/sdk-smoke.mjs --adapter-convergence`, the only runtime
/// that can run TypeScript), and `graph` is what this crate's one derivation makes of
/// it (checked here). One committed file, so the two halves cannot drift apart.
const EXPECTED: &str = include_str!("../../../../../fixtures/ingest/expected-graph.json");

fn member(text: &str, name: &str) -> String {
    let value = graph_contract::ingest::read_value(text).expect("the fixture is JSON");
    match &value {
        JsonValue::Map(members) => members
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, v)| graph_contract::ingest::to_json_value(v))
            .unwrap_or_else(|| panic!("the fixture has no `{name}` member")),
        other => panic!("the fixture's root is not an object: {other:?}"),
    }
}

fn member_of<'a>(value: &'a JsonValue, name: &str) -> &'a JsonValue {
    match value {
        JsonValue::Map(members) => members
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, v)| v)
            .unwrap_or_else(|| panic!("no member `{name}`")),
        other => panic!("not an object: {other:?}"),
    }
}

fn list_of<'a>(value: &'a JsonValue, name: &str) -> &'a [JsonValue] {
    match member_of(value, name) {
        JsonValue::List(items) => items,
        other => panic!("`{name}` is not a list: {other:?}"),
    }
}

fn texts_of(value: &JsonValue, list: &str, field: &str) -> Vec<String> {
    list_of(value, list)
        .iter()
        .map(|item| match member_of(item, field) {
            JsonValue::Text(text) => text.clone(),
            other => panic!("`{list}.{field}` is not a string: {other:?}"),
        })
        .collect()
}

#[test]
fn the_committed_graph_is_exactly_what_the_derivation_produces() {
    let document = graph_contract::ingest::read(&member(EXPECTED, "ingest"))
        .expect("the committed ingest document reads");
    let derived = build(&document).expect("the committed document derives");
    let produced = to_canonical_json(&derived);
    // The same test is the gate row and the generator, so a regeneration cannot be a
    // hand-edit: `GM_WRITE_INGEST_GRAPH=1` rewrites the fixture's `graph` member from the
    // derivation and prints it, and this assertion is what a stale fixture fails.
    if fixture_write::requested() {
        let written = fixture_write::write_graph(&produced).expect("the fixture is writable");
        assert_eq!(
            member(&written, "graph"),
            produced,
            "the generator wrote something the reader cannot read back as the derivation"
        );
        println!(
            "GM_WRITE_INGEST_GRAPH: rewrote fixtures/ingest/expected-graph.json's graph \
             member; commit it"
        );
        return;
    }
    let committed = member(EXPECTED, "graph");
    assert_eq!(
        committed, produced,
        "the committed graph is stale; regenerate with GM_WRITE_INGEST_GRAPH=1 \
         cargo test -p graph-core the_committed_graph, then commit the result"
    );
}

#[test]
fn the_committed_graph_is_internally_consistent() {
    // Read from the committed bytes, not from the derivation: a hand-edited fixture
    // that names a node that does not exist must fail here even if the comparison above
    // is the one a reader runs.
    let graph = graph_contract::ingest::read_value(&member(EXPECTED, "graph"))
        .expect("the committed graph is JSON");
    let ids = texts_of(&graph, "nodes", "id");
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), ids.len(), "a node id is repeated");
    assert!(!ids.is_empty() && !list_of(&graph, "edges").is_empty());
    for end in ["source", "target"] {
        for id in texts_of(&graph, "edges", end) {
            assert!(ids.contains(&id), "edge {end} `{id}` names no node");
        }
    }
    // Every edge id is unique too, and every strength is the table's own: the fixture
    // cannot quietly become a second, divergent strength table.
    let edge_ids = texts_of(&graph, "edges", "id");
    let mut unique_edges = edge_ids.clone();
    unique_edges.sort_unstable();
    unique_edges.dedup();
    assert_eq!(unique_edges.len(), edge_ids.len(), "an edge id is repeated");
    for edge in list_of(&graph, "edges") {
        let kind = match member_of(edge, "kind") {
            JsonValue::Text(name) => EdgeKind::from_name(name).unwrap_or_else(|| panic!("{name}")),
            other => panic!("kind is not a string: {other:?}"),
        };
        let strength = match member_of(edge, "strength") {
            JsonValue::Number(n) => *n,
            other => panic!("strength is not a number: {other:?}"),
        };
        assert_eq!(strength, edge_strength(kind), "{}", "the committed graph");
    }
}

#[test]
fn both_source_fixtures_describe_the_same_dataset_in_two_shapes() {
    // The adapters' half, restated where the derivation lives. What the two files do
    // *not* carry is a role: a role is what an adapter declares, which is exactly why
    // the vendor's `multi_select` and a plain SQL column's name both become `tags` in
    // one place and nowhere else.
    let rows = include_str!("../../../../../fixtures/ingest/rows.json");
    let notion = include_str!("../../../../../fixtures/ingest/notion.json");
    assert!(rows.contains("\"role\""), "rows.json declares roles");
    assert!(
        notion.contains("\"type\""),
        "notion.json declares property types"
    );
    assert!(
        !notion.contains("\"role\""),
        "notion.json declares no roles of its own"
    );
    // The dataset: the same six record ids, in the same order, in both shapes. Counted
    // as row ids in the rows file and page ids in the Notion one, because `"id": "`
    // also names the two tables and the sixteen properties.
    let rows_ids = rows.matches("\"id\": \"").count() - 2;
    let notion_ids = notion.matches("\"id\": \"").count() - 16;
    assert_eq!(rows_ids, 6, "rows.json's six records");
    assert_eq!(notion_ids, rows_ids, "both shapes carry the same records");
    // And the retired record is present in both, deleted: a derivation that ignored
    // `deleted` would draw it, so its presence in both shapes is load-bearing.
    assert!(rows.contains("\"t4\"") && notion.contains("\"t4\""));
    assert!(rows.contains("\"deleted\": true") && notion.contains("\"deleted\": true"));
}
