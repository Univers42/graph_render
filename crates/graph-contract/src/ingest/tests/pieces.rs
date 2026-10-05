//! The canonical writer's output, pinned byte for byte.
//!
//! The pieces API (`DOC_HEAD`, `collection_piece`, `record_piece`, `doc_tail`,
//! `frame_bytes`) exists so a store can stream a document it never holds as an
//! [`Ingest`]. The only way that refactor is safe is a pin written *before* it: the
//! bytes hashed, over fixtures large and small, so "the refactor changed the text" is a
//! test failure and not a diff nobody reads. These constants are pasted from the
//! writer's own output, never from an expectation of it — see the plan's Task 1, Step 2.
//!
//! `rows.json` and `notion.json` are pinned through [`to_json_value`], not
//! [`to_json`]: they are the two *source shapes* (`tests/fixtures.rs` refuses them as
//! ingest documents), so there is no `Ingest` to write, but their whole-value
//! canonical text is the same writer's output and a refactor that moved it would move
//! their pins too.

use super::fixtures::{EXPECTED_GRAPH, NOTION, ROWS, ingest_member};
use super::support::*;
use crate::ingest::{
    DOC_HEAD, DOC_MIDDLE, DOC_SEPARATOR, collection_piece, doc_tail, frame_bytes, record_piece,
};

/// FNV-1a 64. Any fixed 64-bit hash would do; this one is in the crate's own
/// dependency-free reach and has no table to get wrong.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// A document with neither a collection nor a record: the head-to-tail path, where a
/// separator counted once too often shows up.
fn empty() -> Ingest {
    Ingest {
        version: VERSION,
        source: "none".to_owned(),
        collections: Vec::new(),
        records: Vec::new(),
    }
}

/// `(fixture name, FNV-1a 64 of the bytes, byte length)`.
const PINS: [(&str, u64, usize); 5] = [
    ("expected-graph", 3_222_365_584_551_165_266, 1791),
    ("rows", 1_785_417_163_301_380_036, 1835),
    ("notion", 8_970_772_593_890_842_604, 3339),
    ("minimal", 5_020_957_938_646_593_147, 715),
    ("empty", 1_808_171_046_545_584_680, 60),
];

#[test]
fn to_json_bytes_are_pinned() {
    let texts = [
        (
            "expected-graph",
            to_json(&read(&ingest_member(EXPECTED_GRAPH)).unwrap()),
        ),
        ("rows", to_json_value(&read_value(ROWS).unwrap())),
        ("notion", to_json_value(&read_value(NOTION).unwrap())),
        ("minimal", to_json(&read(MINIMAL).unwrap())),
        ("empty", to_json(&empty())),
    ];
    let seen: Vec<(u64, usize)> = texts
        .iter()
        .map(|(_, text)| (fnv1a(text.as_bytes()), text.len()))
        .collect();
    let wrong: Vec<String> = texts
        .iter()
        .zip(&seen)
        .zip(PINS)
        .filter(|((_, got), pin)| **got != (pin.1, pin.2))
        .map(|(((name, _), got), pin)| format!("{name}: got {got:?}, pinned {:?}", (pin.1, pin.2)))
        .collect();
    assert!(
        wrong.is_empty(),
        "the canonical text changed:\n{}",
        wrong.join("\n")
    );
}

/// Every document the pin test above covers, in one place: the two shapes of the
/// assertion below (concatenation, and the byte count) must see the same fixtures.
fn documents() -> Vec<(&'static str, Ingest)> {
    vec![
        (
            "expected-graph",
            read(&ingest_member(EXPECTED_GRAPH)).unwrap(),
        ),
        ("minimal", read(MINIMAL).unwrap()),
        ("empty", empty()),
    ]
}

/// The pieces, concatenated in document order, are the document `to_json` writes. This
/// is the identity the pieces API exists for: a store may write head, collections,
/// middle, records and tail as five pieces and never hold the whole string.
#[test]
fn the_pieces_concatenate_to_the_whole_document() {
    for (name, doc) in documents() {
        let mut out = String::from(DOC_HEAD);
        out.push_str(
            &doc.collections
                .iter()
                .map(collection_piece)
                .collect::<Vec<_>>()
                .join(DOC_SEPARATOR),
        );
        out.push_str(DOC_MIDDLE);
        out.push_str(
            &doc.records
                .iter()
                .map(record_piece)
                .collect::<Vec<_>>()
                .join(DOC_SEPARATOR),
        );
        out.push_str(&doc_tail(&doc.source));
        assert_eq!(out, to_json(&doc), "{name}");
    }
}

/// `frame_bytes` plus the pieces' lengths is the document's length — the property
/// `doc_bytes` rests on (spec §6: an exact upper bound of `to_json().len()`). Counted,
/// not measured, because a hub's answer has to be exact or the body is refused for a
/// size that was never reached.
#[test]
fn the_frame_plus_the_pieces_is_the_document_length() {
    for (name, doc) in documents() {
        let pieces: usize = doc
            .collections
            .iter()
            .map(|c| collection_piece(c).len())
            .chain(doc.records.iter().map(|r| record_piece(r).len()))
            .sum();
        let frame = frame_bytes(
            &doc.source,
            doc.collections.len() as u64,
            doc.records.len() as u64,
        );
        assert_eq!(frame + pieces as u64, to_json(&doc).len() as u64, "{name}");
    }
}

/// The frame is a *bound*, not an estimate: it is exactly the document's bytes with no
/// collection and no record, so a store can size a buffer before reading anything.
#[test]
fn the_frame_is_the_document_with_nothing_in_it() {
    let bare = empty();
    let text = to_json(&bare);
    assert_eq!(frame_bytes(&bare.source, 0, 0), text.len() as u64);
    // Zero collections and zero records mean no separator at all between pieces: the
    // two commas in the text are member separators inside the head, middle and tail.
    assert_eq!(
        text.matches(DOC_SEPARATOR).count(),
        3,
        "the three member commas, and no separator between pieces: {text}"
    );
}
