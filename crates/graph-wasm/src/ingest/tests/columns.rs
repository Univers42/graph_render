//! The differential: the whole `Topology` from the JSON path against the whole `Topology`
//! from the columnar path, over a corpus that covers every field class.
//!
//! `format!("{:?}")` of the `Topology`, not a field-by-field comparison. A per-field
//! comparison says *which* field differs and so would need a rule for each; the whole-value
//! comparison says *that* the two paths disagree, which is the only question this format
//! exists to answer, and it cannot be satisfied by comparing fewer things than
//! `index_model` actually reads — the arena, the group column, `by_database`, all three
//! CSRs and the note count included.
//!
//! The negative control at the bottom is the same comparison with one bit flipped, and it
//! asserts the strings **differ**. Without it, a differential that compared nothing would
//! pass.

mod corpus;
mod encode;
mod generated;
mod json;

use crate::errors::Code;
use crate::ingest::MAX_INGEST_BYTES;
use crate::ingest::columns::{self, ColumnsError};
use corpus::Coverage;
use graph_core::Topology;

/// The reference path's answer: `read_records` then `ingest::index`, byte for byte what
/// `gm_build` does.
fn via_json(text: &str) -> Result<Topology, String> {
    let (nodes, edges) =
        crate::ingest::read_records(text.as_bytes()).map_err(|e| format!("{e:?}"))?;
    crate::ingest::index(&nodes, &edges).map_err(|e| format!("{e:?}"))
}

/// The columnar path's answer, from the records the *same* reader produced.
fn via_columns(
    records: &corpus::Document,
) -> Result<Topology, crate::ingest::columns::ColumnsError> {
    crate::ingest::columns::index(&encode::encode(&records.nodes, &records.edges).bytes)
}

#[test]
fn both_paths_build_the_same_topology_for_every_document_in_the_corpus() {
    let corpus = generated::all();
    for records in &corpus {
        let expected = via_json(&records.json)
            .unwrap_or_else(|e| panic!("{}: the reference path refused it: {e}", records.name));
        let actual = via_columns(records)
            .unwrap_or_else(|e| panic!("{}: the columnar path refused it: {e:?}", records.name));
        assert_eq!(
            format!("{expected:?}"),
            format!("{actual:?}"),
            "{}: the two paths disagree",
            records.name
        );
    }
}

#[test]
fn the_columnar_path_is_the_same_reader_on_the_same_records() {
    // The documents in the corpus are spelled as JSON and read back by `read_records`
    // before either path indexes them, so this test can assert that the records the encoder
    // was handed are literally the ones the reference reader produced — otherwise the
    // differential above would be comparing a graph against a different graph.
    for records in generated::all() {
        let (nodes, edges) = crate::ingest::read_records(records.json.as_bytes())
            .unwrap_or_else(|e| panic!("{}: {e:?}", records.name));
        assert_eq!(nodes, records.nodes, "{}", records.name);
        assert_eq!(edges, records.edges, "{}", records.name);
    }
}

#[test]
fn the_corpus_covers_every_field_class() {
    let mut coverage = Coverage::default();
    for records in generated::all() {
        coverage.add(&records);
    }
    assert!(coverage.complete(), "{coverage:?}");
}

#[test]
fn a_negative_control_mutation_makes_the_differential_red() {
    // Criterion 9. Flipping one edge's `child_first` is a mutation that stays legal — the
    // contract reads a boolean as 0 or 1 — so nothing refuses it and only a field-by-field
    // comparison can see it. The assert is that the two `Debug` strings now *differ*: this
    // test passes when the differential above would have caught the mutation, and fails the
    // moment it would stop catching it.
    let records = &generated::all()[0];
    let expected = format!("{:?}", via_json(&records.json).expect("valid"));
    let encoded = encode::encode(&records.nodes, &records.edges);
    let mut mutated = encoded.bytes.clone();
    let first = &encoded.bytes[encoded.edge_child_first..][..4];
    let flipped = u32::from(first != [1, 0, 0, 0]);
    mutated[encoded.edge_child_first..][..4].copy_from_slice(&flipped.to_le_bytes());
    let actual = crate::ingest::columns::index(&mutated).expect("a boolean is still a boolean");
    assert_ne!(expected, format!("{actual:?}"), "the mutation went unseen");
}

#[test]
fn rotating_an_endpoint_row_makes_the_differential_red() {
    // The other half of the control: an endpoint row is the one field with no id to look up,
    // so a mutation there must move the graph or the differential is not reading it.
    let records = &generated::all()[2];
    let expected = format!("{:?}", via_json(&records.json).expect("valid"));
    let encoded = encode::encode(&records.nodes, &records.edges);
    let mut mutated = encoded.bytes.clone();
    let last = 4 * (records.edges.len() - 1);
    let at = encoded.edge_source + last;
    let row = u32::from_le_bytes(mutated[at..at + 4].try_into().expect("4 bytes"));
    let other = u32::from(row == 0);
    mutated[at..at + 4].copy_from_slice(&other.to_le_bytes());
    let actual = crate::ingest::columns::index(&mutated).expect("row 1 exists");
    assert_ne!(expected, format!("{actual:?}"), "the rotation went unseen");
}

#[test]
fn a_mutation_that_does_not_stay_legal_is_refused_never_a_panic() {
    // The other direction from the controls above: a mutated document the contract forbids
    // must come back as a refusal, because the export turns that into `ColumnsInvalid`. A
    // trap here would be an ABI failure a host cannot name.
    let records = &generated::all()[2];
    let encoded = encode::encode(&records.nodes, &records.edges);
    let last = 4 * (records.edges.len() - 1);
    let node_count = read_u32(&encoded.bytes, HEADER + 8);
    let cases = [
        (
            "an endpoint past the last node",
            (encoded.edge_source + last, node_count),
        ),
        ("a boolean that is 2", (encoded.edge_child_first + last, 2)),
        ("a node kind naming no kind", (encoded.node_kind, 0)),
    ];
    for (what, (at, value)) in cases {
        let mut bytes = encoded.bytes.clone();
        put(&mut bytes, at, value);
        assert_eq!(
            columns::index(&bytes).err(),
            Some(ColumnsError::Invalid),
            "{what}"
        );
    }
    for (what, bytes) in [
        ("a short buffer", truncated(&encoded.bytes)),
        ("a bad magic", with_header(&encoded.bytes, 0, 0x1234_5678)),
    ] {
        assert_eq!(
            columns::index(&bytes).err(),
            Some(ColumnsError::Invalid),
            "{what}"
        );
    }
}

/// The same bytes, one byte short.
fn truncated(bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    out.truncate(out.len() - 1);
    out
}

/// The same bytes, with header word `word` replaced.
fn with_header(bytes: &[u8], word: usize, value: u32) -> Vec<u8> {
    let mut out = bytes.to_vec();
    put(&mut out, 4 * word, value);
    out
}

const HEADER: usize = 32;

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("4 bytes"))
}

#[test]
fn every_columns_refusal_names_its_wire_code() {
    // Criterion 6 from the host's side: an oversized document is `IngestTooLarge` (F-16, and
    // it is not `ColumnsInvalid` — the document is not malformed, it is too big to hold) and
    // everything else this path can refuse is `ColumnsInvalid`.
    assert_eq!(ColumnsError::TooLarge.code(), Code::IngestTooLarge);
    assert_eq!(ColumnsError::Invalid.code(), Code::ColumnsInvalid);
    assert_eq!(
        columns::index(&[0u8; MAX_INGEST_BYTES + 1]).err(),
        Some(ColumnsError::TooLarge)
    );
}
