//! The committed documents of `fixtures/ingest`, checked against the reader.
//!
//! Split out of `reader.rs` by the house's 300-line limit, and because it is a different
//! question from "what does the reader refuse": the refusals in `reader.rs` pin the reader's
//! own messages, and this one asks whether the *documents the repository ships* still read
//! at all — the gate every refusal added to the reader has to clear.

use super::support::*;

// --------------------------------------------------- the committed documents

/// The three committed fixtures of `fixtures/ingest`, compiled in: a missing or renamed
/// one is a build failure rather than a test that quietly checks nothing.
const EXPECTED_GRAPH: &str = include_str!("../../../../../fixtures/ingest/expected-graph.json");
const ROWS: &str = include_str!("../../../../../fixtures/ingest/rows.json");
const NOTION: &str = include_str!("../../../../../fixtures/ingest/notion.json");

/// The gate every refusal above has to clear. `expected-graph.json`'s `ingest` member is
/// the document `graph-core`'s convergence test, `graph-wasm`'s contract test and
/// `graph-cli ingest` all hand this reader, so a refusal here is a committed document the
/// motor can no longer accept — a failure, not a fixture to loosen. `rows.json` and
/// `notion.json` are the source shapes the TypeScript adapters map (`tables`/`columns`,
/// `databases`/`properties`); they are *not* ingest documents, and a reader that began
/// accepting them would change what the contract means.
#[test]
fn every_committed_ingest_fixture_still_reads() {
    let doc = read(&ingest_member(EXPECTED_GRAPH)).expect("the committed document reads");
    let counts = (
        doc.source.as_str(),
        doc.collections.len(),
        doc.records.len(),
    );
    assert_eq!(counts, ("lib", 2, 6));
    for (name, text) in [("rows.json", ROWS), ("notion.json", NOTION)] {
        assert_eq!(
            read(text).unwrap_err().to_string(),
            "the document: unknown member `_comment`",
            "{name} is a source shape, not an ingest document"
        );
    }
}

/// The named member of a two-member fixture, back as wire text: `graph-core` and
/// `graph-wasm` do the same round trip through `read_value`/`to_json_value`.
fn ingest_member(text: &str) -> String {
    let value = read_value(text).expect("the fixture is JSON");
    let JsonValue::Map(members) = value else {
        panic!("the fixture's root is not an object");
    };
    let member = members.iter().find(|(key, _)| key == "ingest");
    to_json_value(member.map(|(_, v)| v).expect("no `ingest` member"))
}
