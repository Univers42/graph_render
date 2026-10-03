//! The happy path: what a decoded document hands out, and the properties every accessor's
//! safety argument rests on.

use super::*;

#[test]
fn an_empty_document_decodes_to_an_empty_doc() {
    let encoded = Doc::new().encode();
    assert_eq!(
        encoded.bytes.len(),
        40,
        "header, one offset, then pad to an eight-byte boundary"
    );
    let doc = decode(&encoded.bytes).expect("an empty document is a document");
    assert_eq!(
        (doc.node_count(), doc.edge_count(), doc.string_count()),
        (0, 0, 0)
    );
    assert_eq!(doc.blob(), "");
    assert_eq!(doc.node(0), None);
    assert_eq!(doc.edge(0), None);
}

#[test]
fn a_full_document_hands_out_every_row_it_declares() {
    let encoded = full().encode();
    let doc = decode(&encoded.bytes).expect("the fixture is valid");
    assert_eq!(encoded.bytes.len(), encoded.marks.child_first + 4);
    assert_eq!(doc.node_count(), 2);
    assert_eq!(doc.edge_count(), 1);
    let first = doc.node(0).expect("row 0");
    let second = doc.node(1).expect("row 1");
    assert_eq!(
        (
            first.id,
            first.kind,
            first.database_id,
            first.source,
            first.label,
            first.group,
            first.icon,
            first.has_note
        ),
        (
            "n-0",
            "record",
            Some("db-0"),
            "studio",
            "Graph notes",
            Some("Epsilon"),
            Some("\u{1f33f}"),
            true
        )
    );
    assert_eq!(
        (
            second.id,
            second.kind,
            second.database_id,
            second.group,
            second.icon
        ),
        ("n-1", "note", None, None, None)
    );
    assert!(!second.has_note);
    let edge = doc.edge(0).expect("row 0");
    assert_eq!(
        (
            edge.id,
            edge.source_row,
            edge.target_row,
            edge.kind,
            edge.label,
            edge.record_id
        ),
        ("e-0", 0, 1, "relation", "relation", Some("rec-7"))
    );
    assert!(edge.directed && edge.child_first && edge.strength == 0.5);
}

#[test]
fn a_row_past_the_counts_is_none_rather_than_a_trap() {
    let encoded = full().encode();
    let doc = decode(&encoded.bytes).expect("valid");
    assert_eq!(doc.node(doc.node_count()), None);
    assert_eq!(doc.edge(doc.edge_count()), None);
}

#[test]
fn text_is_none_past_the_table_and_the_blob_is_one_borrow() {
    let encoded = full().encode();
    let doc = decode(&encoded.bytes).expect("valid");
    let last = doc.string_count();
    assert_eq!(doc.text(last - 1), Some("rec-7"));
    assert_eq!(doc.text(last), None, "the closing offset is not an entry");
    assert_eq!(
        doc.text(u32::MAX),
        None,
        "and nothing is read one past it either"
    );
    assert_eq!(
        doc.blob(),
        "n-0n-1db-0studioGraph notesEpsilon\u{1f33f}recordnoterelatione-0rec-7"
    );
}

#[test]
fn a_string_table_may_hold_the_same_bytes_twice() {
    // Legal, and the arena collapses it: two entries, one value. The offsets still tile the
    // blob, so nothing here is a special case — it is what an un-deduped encoder produces.
    let mut doc = Doc::new();
    let a = doc.s("dup");
    let b = doc.duplicate("dup");
    let kind = doc.s("record");
    assert_ne!(a, b);
    let node = doc.node();
    node.id = a;
    node.kind = kind;
    node.source = kind;
    node.label = b;
    let encoded = doc.encode();
    let decoded = decode(&encoded.bytes).expect("two entries, one value");
    assert_eq!(decoded.node(0).expect("row 0").label, "dup");
    assert_eq!(
        decoded.blob(),
        "dupduprecord",
        "two entries, one value, plus the kind"
    );
}

#[test]
fn the_sections_land_where_the_contract_says_they_do() {
    let encoded = full().encode();
    let marks = encoded.marks;
    assert_eq!(marks.offsets, 32);
    assert_eq!(marks.blob, 32 + 4 * (full().string_count() as usize + 1));
    assert_eq!(
        marks.weight - marks.blob - full().blob_len() as usize,
        (8 - marks.blob % 8) % 8
    );
    assert_eq!(
        marks.weight,
        marks.blob + full().blob_len() as usize + (8 - marks.blob % 8) % 8
    );
    assert_eq!(marks.version - marks.weight, 8 * 2);
    assert_eq!(marks.strength - marks.version, 8 * 2);
    assert_eq!(marks.node_id - marks.strength, 8);
    assert_eq!(marks.node_kind - marks.node_id, 4 * 2);
}
