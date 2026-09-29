//! The scale fixtures: at or below the model's own cap it *is* the model, past it the
//! fixture is whole prefixed components rather than a truncation, and the emitted document
//! is the provisional ingest shape `graph-wasm`'s reader accepts.

use super::super::scale;
use super::super::*;

/// A scale fixture at or below the model's own cap is the model itself, byte for byte:
/// the generator is not a second implementation of the benchmark graph.
#[test]
fn a_scale_fixture_at_or_below_the_cap_is_the_synthetic_model() {
    let (nodes, edges) = scale::scale_model(0, 220, REFERENCE_DEGREE);
    let (want_nodes, want_edges) = seeded_model(0, 220, REFERENCE_DEGREE);
    assert_eq!(nodes.len(), want_nodes.len());
    assert_eq!(
        nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
        want_nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(edges.len(), want_edges.len());
    assert_eq!(edges[0].id, want_edges[0].id);
    assert_eq!(edges[0].strength, want_edges[0].strength);
}

/// Past the cap the fixture is whole components of the same model, concatenated, each
/// node id prefixed by its component so no id repeats: a 250k fixture is 100k + 100k +
/// 50k, not a truncated 250k.
#[test]
fn a_scale_fixture_past_the_cap_is_whole_prefixed_components() {
    let (nodes, edges) = scale::scale_model(0, 250_000, REFERENCE_DEGREE);
    assert_eq!(nodes.len(), 250_000);
    let mut ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), before, "a component reused a node id");
    assert!(nodes[0].id.starts_with("c0/"), "{}", nodes[0].id);
    assert!(
        nodes[100_000].id.starts_with("c1/"),
        "{}",
        nodes[100_000].id
    );
    assert!(
        nodes[200_000].id.starts_with("c2/"),
        "{}",
        nodes[200_000].id
    );
    let known: std::collections::HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    for edge in &edges {
        assert!(known.contains(edge.source.as_str()), "{}", edge.source);
        assert!(known.contains(edge.target.as_str()), "{}", edge.target);
    }
}

/// The emitted fixture is the provisional ingest document `graph-wasm`'s reader accepts:
/// version 1, every member named, no member beyond the ten and nine it requires.
#[test]
fn an_emitted_scale_fixture_is_the_provisional_ingest_shape() {
    let json = scale::fixture_json(0, 220, REFERENCE_DEGREE);
    assert!(
        json.starts_with(r#"{"version":1,"nodes":["#),
        "{}",
        &json[..40.min(json.len())]
    );
    let value = graph_contract::canonical_json::parse(&json).expect("valid JSON");
    let nodes = members(&value, "nodes");
    let edges = members(&value, "edges");
    let (_, model_edges) = seeded_model(0, 220, REFERENCE_DEGREE);
    assert_eq!((nodes.len(), edges.len()), (220, model_edges.len()));
    assert_eq!(member_names(&nodes[0]), scale::NODE_FIELDS);
    assert_eq!(member_names(&edges[0]), scale::EDGE_FIELDS);
}

/// The `member` array of a parsed document, or the test's own failure.
fn members(
    value: &graph_contract::canonical_json::Value,
    member: &str,
) -> Vec<graph_contract::canonical_json::Value> {
    let graph_contract::canonical_json::Value::Object(fields) = value else {
        panic!("the document is not an object");
    };
    match fields.iter().find(|(name, _)| name == member) {
        Some((_, graph_contract::canonical_json::Value::Array(items))) => items.clone(),
        _ => panic!("no `{member}` array"),
    }
}

/// An object's member names, sorted, which is what the ingest reader's `require_only`
/// compares against.
fn member_names(value: &graph_contract::canonical_json::Value) -> Vec<&str> {
    let graph_contract::canonical_json::Value::Object(fields) = value else {
        panic!("not an object");
    };
    let mut names: Vec<&str> = fields.iter().map(|(name, _)| name.as_str()).collect();
    names.sort_unstable();
    names
}
