//! The contract path at the ABI boundary: the committed document, the refusals, and the
//! two things only this boundary can prove — that what arrives here is `graph_core`'s
//! single derivation and not a second copy of it, and that the provisional format
//! `gm_build` still reads is not this one's.
//!
//! The wasm32 half of this (the export itself) is reached by `harness/sdk-smoke.mjs
//! --contract-convergence`; everything here is what can be proved without a target.

use super::*;
use graph_contract::ingest::{IngestError, JsonValue};
use graph_core::ingest::to_canonical_json;

/// `fixtures/ingest/expected-graph.json` at compile time, so a missing or renamed
/// fixture is a build failure rather than a test that quietly checks nothing. Both
/// members are read here: `ingest` is the document a caller hands this export, `graph`
/// is what it must derive.
const EXPECTED: &str = include_str!("../../../../fixtures/ingest/expected-graph.json");

/// The smallest document that still exercises a hierarchy, tags and a weight — the
/// three roles whose derivation a swap would be visible in.
const SMALL: &str = r#"{"version":1,"source":"rows","collections":[{"id":"task","name":"Tasks","titleField":"name","fields":[{"id":"name","name":"Name","role":"title","link":null},{"id":"up","name":"Up","role":"parent","link":null},{"id":"labels","name":"Labels","role":"tags","link":null},{"id":"effort","name":"Effort","role":"weight","link":null}]}],"records":[{"id":"r1","collection":"task","deleted":false,"updatedAt":7,"values":{"name":"Write","labels":["docs"]}},{"id":"r2","collection":"task","deleted":false,"updatedAt":8,"values":{"name":"Ship","up":"r1","labels":["docs","graph"],"effort":5}}]}"#;

/// The shape `gm_build` reads and this export does not: the two formats are
/// distinguishable, which is the only reason a swapped reader is a test failure rather
/// than a silent change of meaning.
const PROVISIONAL: &str = r#"{"version":1,"nodes":[{"id":"a","kind":"record","database_id":null,"source":"s","label":"A","group":null,"weight":0.5,"version":0,"has_note":false,"icon":null}],"edges":[]}"#;

fn member(text: &str, name: &str) -> String {
    let value = graph_contract::ingest::read_value(text).expect("the fixture is JSON");
    match value {
        JsonValue::Map(members) => members
            .into_iter()
            .find(|(key, _)| key == name)
            .map(|(_, v)| graph_contract::ingest::to_json_value(&v))
            .unwrap_or_else(|| panic!("the fixture has no `{name}` member")),
        other => panic!("the fixture's root is not an object: {other:?}"),
    }
}

fn ids(derived: &Derived) -> Vec<&str> {
    derived.nodes.iter().map(|n| n.id.as_str()).collect()
}

/// `(source, target, kind, label, strength)` per edge, in derivation order.
fn edges(derived: &Derived) -> Vec<(String, String, String, String, f64)> {
    derived
        .edges
        .iter()
        .map(|e| {
            (
                e.source.clone(),
                e.target.clone(),
                e.kind.as_str().to_owned(),
                e.label.clone(),
                e.strength,
            )
        })
        .collect()
}

#[test]
fn the_committed_contract_derives_exactly_the_committed_graph() {
    let (derived, topology) =
        derive(member(EXPECTED, "ingest").as_bytes()).expect("the committed contract derives");
    assert_eq!(
        to_canonical_json(&derived),
        member(EXPECTED, "graph"),
        "the committed graph is stale; regenerate it with \
         GM_WRITE_INGEST_GRAPH=1 cargo test -p graph-core the_committed_graph"
    );
    assert_eq!(
        ids(&derived),
        [
            "lib:task:t1",
            "lib:task:t2",
            "lib:task:t3",
            "lib:person:p1",
            "lib:person:p2",
            "tag:docs",
            "tag:p0",
            "tag:graph",
        ],
        "records in document order, then one hub per distinct tag value"
    );
    assert_eq!(topology.node_count(), 8, "the indexed graph holds them all");
}

#[test]
fn the_contract_path_derives_what_graph_core_derives_and_nothing_else() {
    // The point of routing through `graph_core::ingest` rather than indexing a document
    // this crate read itself: this is where a second derivation would show up, and the
    // comparison is against the derivation itself rather than against a copy of it.
    let text = member(EXPECTED, "ingest");
    let document = graph_contract::ingest::read(&text).expect("the committed contract reads");
    let (derived, topology) = derive(text.as_bytes()).expect("the committed contract derives");
    assert_eq!(
        derived,
        graph_core::ingest::build(&document).expect("graph-core derives it"),
        "the wasm boundary derives, it does not derive differently"
    );
    assert_eq!(
        topology.node_count(),
        u32::try_from(derived.nodes.len()).expect("a derived graph fits a u32"),
        "every derived node reaches the topology"
    );
}

#[test]
fn the_two_ingest_formats_are_not_interchangeable() {
    // Each reader refuses the other's document. Without both halves, routing this export
    // to the provisional parser (or `gm_build` to the contract) would still produce a
    // plausible graph, which is the failure mode this pair exists to prevent.
    assert!(
        matches!(
            derive(PROVISIONAL.as_bytes()),
            Err(ContractError::Document(IngestError::Shape { .. }))
        ),
        "a node/edge document is not a contract document"
    );
    assert!(
        crate::ingest::read(SMALL.as_bytes()).is_err(),
        "a contract document is not a node/edge document"
    );
}

#[test]
fn a_document_the_reader_refuses_never_reaches_the_derivation() {
    for (what, text) in [
        (
            "a version the contract does not name",
            SMALL.replace(r#""version":1"#, r#""version":2"#),
        ),
        (
            "an unknown member",
            SMALL.replace(r#""source":"rows""#, r#""source":"rows","extra":1"#),
        ),
        (
            "a role outside the eight",
            SMALL.replace(r#""role":"title""#, r#""role":"Title""#),
        ),
        (
            "a source that cannot round-trip (H5)",
            SMALL.replace(r#""source":"rows""#, r#""source":"row:s""#),
        ),
        (
            "a record naming no declared collection",
            SMALL.replace(
                r#""collection":"task","deleted":false,"updatedAt":8"#,
                r#""collection":"nope","deleted":false,"updatedAt":8"#,
            ),
        ),
    ] {
        let err = derive(text.as_bytes()).expect_err(what);
        assert!(
            matches!(err, ContractError::Document(_)),
            "{what} must be refused by the contract's own reader: {err:?}"
        );
    }
}

#[test]
fn a_coordinate_that_cannot_round_trip_is_refused_where_it_is_made() {
    // `source` is the reader's check (it is a coordinate of every node id); a tag *value*
    // is the derivation's, because `tag:a:b` would parse back shifted. Both are loud,
    // and neither is silently rewritten into something that does not round-trip.
    let bad_tag = SMALL.replace(r#""docs""#, r#""do:cs""#);
    let Err(refusal) = derive(bad_tag.as_bytes()) else {
        panic!("a tag value with a colon cannot round-trip");
    };
    assert_eq!(
        refusal,
        ContractError::Derivation(BuildError::IdGrammar {
            coordinate: "tag value",
            value: "do:cs".to_owned(),
        })
    );
}

#[test]
fn not_utf8_and_not_json_are_refused_before_anything_else() {
    let Err(refusal) = derive(&[0xff, 0xfe]) else {
        panic!("not UTF-8 is refused before the reader is asked to parse it");
    };
    assert_eq!(refusal, ContractError::Utf8);
    assert!(
        matches!(
            derive(b"not json"),
            Err(ContractError::Document(IngestError::Json(_)))
        ),
        "the refusal says which step said no"
    );
}

#[test]
fn a_small_contract_derives_its_hierarchy_its_tags_and_its_weight() {
    let (derived, topology) = derive(SMALL.as_bytes()).expect("the small contract derives");
    assert_eq!(
        ids(&derived),
        ["rows:task:r1", "rows:task:r2", "tag:docs", "tag:graph"],
        "two records, then one hub per distinct tag value in first-appearance order"
    );
    assert_eq!(
        edges(&derived),
        [
            (
                "rows:task:r1".into(),
                "tag:docs".into(),
                "tag".into(),
                "docs".into(),
                0.75
            ),
            // `r2`'s `up` is `r1`, and a hierarchy edge runs **parent first** — the same
            // direction the committed fixture's `lib:task:t1 -> lib:task:t2` pins.
            (
                "rows:task:r1".into(),
                "rows:task:r2".into(),
                "hierarchy".into(),
                String::new(),
                2.0,
            ),
            (
                "rows:task:r2".into(),
                "tag:docs".into(),
                "tag".into(),
                "docs".into(),
                0.75
            ),
            (
                "rows:task:r2".into(),
                "tag:graph".into(),
                "tag".into(),
                "graph".into(),
                0.75
            ),
        ],
        "hierarchy before tags, parent first, one strength table"
    );
    let weight = derived.nodes[1].weight;
    assert_eq!(weight, 5.0, "the declared weight reaches the node");
    assert_eq!(
        derived.nodes[0].weight, 0.5,
        "and the default one is used when absent"
    );
    assert_eq!(
        derived.nodes[1].label, "Ship",
        "the title role is the label"
    );
    assert_eq!(topology.node_count(), 4);
}

#[test]
fn a_deleted_record_derives_nothing_through_this_path_too() {
    let deleted = SMALL.replace(
        r#"{"id":"r2","collection":"task","deleted":false"#,
        r#"{"id":"r2","collection":"task","deleted":true"#,
    );
    let (derived, topology) = derive(deleted.as_bytes()).expect("a document of one live record");
    assert_eq!(ids(&derived), ["rows:task:r1", "tag:docs"]);
    assert!(
        derived.edges.iter().all(|e| !e.target.contains("r2")),
        "no edge may name a deleted record: {:?}",
        edges(&derived)
    );
    assert_eq!(
        topology.node_count(),
        2,
        "deletion is honoured here, not downstream"
    );
}
