//! The canonical writer: one text per document, and its round trip.

use super::support::*;

// ---------------------------------------------------------------- the writer

#[test]
fn the_writer_is_canonical_compact_with_keys_sorted_by_bytes() {
    let text = to_json(&minimal());
    assert!(text.ends_with('\n'), "one trailing newline");
    assert_eq!(text.lines().count(), 1, "one line: {text}");
    let top: Vec<&str> = [
        "\"collections\"",
        "\"records\"",
        "\"source\"",
        "\"version\"",
    ]
    .into();
    let mut at = 0;
    for key in top {
        let found = text[at..]
            .find(key)
            .unwrap_or_else(|| panic!("{key} after byte {at}"));
        at += found;
    }
    // The document's own keys come out in byte order, not declaration order.
    let collections = text.find("\"collections\"").unwrap();
    let records = text.find("\"records\"").unwrap();
    let source = text.find("\"source\"").unwrap();
    let version = text.find("\"version\"").unwrap();
    assert!(
        collections < records && records < source && source < version,
        "{text}"
    );
    // A record's `values` map is sorted by key too, whatever order it was read in.
    // Scoped to the `values` object: the field *declarations* above also spell these
    // ids, and a whole-text search would find those instead.
    let values = &text[text.find("\"values\":").unwrap()..];
    let at = |key: &str| {
        values
            .find(key)
            .unwrap_or_else(|| panic!("{key} in {values}"))
    };
    assert!(at("\"effort\"") < at("\"labels\""), "{values}");
    assert!(at("\"labels\"") < at("\"name\""), "{values}");
    assert!(at("\"name\"") < at("\"state\""), "{values}");
}

#[test]
fn write_then_read_gives_back_the_same_document() {
    let first = minimal();
    let second = read(&to_json(&first)).expect("the writer's own output reads");
    assert_eq!(first, second);
    assert_eq!(to_json(&first), to_json(&second));
}

#[test]
fn the_writer_escapes_exactly_what_json_requires_and_nothing_more() {
    let value = JsonValue::Text("a\"b\\c\nd\te\u{1}f\u{7f}g/h é😀".into());
    let text = to_json_value(&value);
    assert_eq!(text, "\"a\\\"b\\\\c\\nd\\te\\u0001f\u{7f}g/h é😀\"");
    assert_eq!(read_value(&text).expect("round trip"), value);
}

#[test]
fn every_json_value_kind_writes_and_reads_back() {
    let cases = [
        JsonValue::Null,
        JsonValue::Bool(true),
        JsonValue::Bool(false),
        JsonValue::Number(0.0),
        JsonValue::Number(-1.5),
        JsonValue::Number(1e21),
        JsonValue::Text(String::new()),
        JsonValue::List(vec![]),
        JsonValue::Map(vec![]),
        JsonValue::List(vec![JsonValue::Null, JsonValue::Number(2.0)]),
    ];
    for value in cases {
        let text = to_json_value(&value);
        assert_eq!(
            read_value(&text).unwrap_or_else(|e| panic!("{text}: {e}")),
            value
        );
    }
}

#[test]
fn a_number_writes_in_the_shortest_form_that_reads_back_as_the_same_f64() {
    for number in [0.0, -0.5, 1.0, 1.5, 0.1, 1e21, -2.25e-7, 1234567890.0] {
        let text = to_json_value(&JsonValue::Number(number));
        let back = read_value(&text).expect("a number reads back");
        assert_eq!(back, JsonValue::Number(number), "{number} wrote as {text}");
    }
    // A whole number writes without a fractional part, so a reader cannot tell 2.0
    // from 2 and the byte comparison between two adapters stays exact.
    assert_eq!(to_json_value(&JsonValue::Number(2.0)), "2");
    assert_eq!(to_json_value(&JsonValue::Number(-0.0)), "-0");
}

#[test]
fn a_value_member_is_a_map_so_a_duplicate_key_is_refused_not_last_wins() {
    assert_eq!(
        read_value(r#"{"a":1,"a":2}"#).unwrap_err().to_string(),
        "not JSON at byte 7: a key repeated in one object"
    );
}

#[test]
fn a_values_map_writes_the_same_text_whatever_order_it_was_read_in() {
    // The writer sorts a `values` map by key and, for two equal keys, by the value's
    // own text — the same total order in both places, so there is no second rule to
    // drift. H6 in one test: a record whose cells arrived in a different member order
    // writes the same bytes.
    let mut doc = minimal();
    let record = &mut doc.records[0];
    record.values.reverse();
    assert_eq!(to_json(&doc), to_json(&minimal()));
}

#[test]
fn a_record_with_a_repeated_key_writes_both_cells_because_the_reader_would_refuse_it() {
    // Unreachable through `read` (a repeated key is refused there), so it is pinned on
    // a hand-built document instead: the writer must not silently drop one.
    let mut doc = minimal();
    doc.records[0]
        .values
        .push(("name".to_string(), JsonValue::Text("Other".into())));
    let text = to_json(&doc);
    let values = &text[text.find("\"values\":").unwrap()..];
    assert_eq!(
        values.matches("\"name\":").count(),
        2,
        "one cell was dropped: {values}"
    );
    // Sorted by value, so the shorter text comes first: a total order, not insertion.
    let other = values.find(r#""name":"Other""#).unwrap();
    let write = values.find(r#""name":"Write""#).unwrap();
    assert!(other < write, "{values}");
    assert!(read(&text).is_err(), "the reader must refuse the repeat");
}
