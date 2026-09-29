//! The strict reader: what it accepts, and the message each refusal carries.

use super::support::*;

// ---------------------------------------------------------------- the reader

#[test]
fn the_minimal_document_reads_to_exactly_these_collections_and_records() {
    let doc = minimal();
    assert_eq!(doc.version, VERSION);
    assert_eq!(doc.source, "rows");
    assert_eq!(doc.collections.len(), 1);
    let collection = &doc.collections[0];
    assert_eq!(
        (
            collection.id.as_str(),
            collection.name.as_str(),
            collection.title_field.as_str()
        ),
        ("task", "Tasks", "name")
    );
    let fields: Vec<(&str, Role)> = collection
        .fields
        .iter()
        .map(|f| (f.id.as_str(), f.role))
        .collect();
    // Canonical order — sorted by field id, whatever order the document listed them in.
    assert_eq!(
        fields,
        [
            ("blocks", Role::Link),
            ("effort", Role::Weight),
            ("labels", Role::Tags),
            ("name", Role::Title),
            ("note", Role::Scalar),
            ("state", Role::Group),
            ("up", Role::Parent),
        ]
    );
    let link = collection
        .field("blocks")
        .expect("declared")
        .link
        .as_ref()
        .unwrap();
    assert_eq!(
        (link.collection.as_str(), link.cardinality, link.symmetric),
        ("task", Cardinality::Many, false)
    );
    assert_eq!(doc.records.len(), 1);
    let record = &doc.records[0];
    assert_eq!(
        (
            record.id.as_str(),
            record.collection.as_str(),
            record.deleted,
            record.updated_at
        ),
        ("r1", "task", false, 1_700_000_000)
    );
    assert_eq!(record.value("name"), Some(&JsonValue::Text("Write".into())));
    assert_eq!(
        record.value("state"),
        Some(&JsonValue::Text("doing".into()))
    );
    assert_eq!(
        record.value("labels"),
        Some(&JsonValue::List(vec![JsonValue::Text("wip".into())]))
    );
    assert_eq!(record.value("effort"), Some(&JsonValue::Number(2.0)));
    assert_eq!(record.value("absent"), None);
}

#[test]
fn a_field_without_the_link_member_reads_with_link_none() {
    let doc = minimal();
    assert!(
        doc.collections[0]
            .field("name")
            .expect("declared")
            .link
            .is_none()
    );
}

#[test]
fn a_documents_field_order_is_canonical_whatever_order_it_was_written_in() {
    // The reader sorts a collection's fields by id, so two documents declaring the same
    // roles in a different order are the *same* document — which is what lets two
    // adapters, writing their declarations in whatever order their source listed them,
    // produce byte-identical text.
    let reversed = MINIMAL
        .replace(
            r#"{ "id": "name", "name": "Name", "role": "title", "link": null },
        { "id": "state", "name": "State", "role": "group", "link": null },"#,
            r#"{ "id": "state", "name": "State", "role": "group", "link": null },
        { "id": "name", "name": "Name", "role": "title", "link": null },"#,
        )
        .to_owned();
    assert_ne!(
        MINIMAL, reversed,
        "the two documents are written differently"
    );
    let read_reversed = read(&reversed).expect("the reversed document reads");
    assert_eq!(minimal(), read_reversed);
    assert_eq!(to_json(&minimal()), to_json(&read_reversed));
}

#[test]
fn a_duplicate_field_id_is_refused_with_the_documents_own_index() {
    // The sort happens *after* this check, so the path names where the field actually is
    // in the file the reader has open, not where it would be once sorted.
    let text = MINIMAL.replacen(
        r#"{ "id": "state", "name": "State", "role": "group", "link": null }"#,
        r#"{ "id": "name", "name": "State", "role": "group", "link": null }"#,
        1,
    );
    assert_eq!(
        err(&text),
        "collections[0].fields[1]: duplicate field id `name`"
    );
}

#[test]
fn an_empty_document_reads_with_no_collections_and_no_records() {
    let doc = read(r#"{"version":1,"source":"s","collections":[],"records":[]}"#)
        .expect("an empty document is a document");
    assert!(doc.collections.is_empty());
    assert!(doc.records.is_empty());
}

#[test]
fn the_reader_refuses_every_fault_with_its_own_message() {
    // Wrong version.
    assert_eq!(
        err(&MINIMAL.replace(r#""version": 1"#, r#""version": 2"#)),
        "version: unsupported version 2"
    );
    // A version that is not a plain integer literal.
    assert_eq!(
        err(&MINIMAL.replace(r#""version": 1"#, r#""version": 1.0"#)),
        "version: expected a plain non-negative integer"
    );
    // A missing member, named.
    assert_eq!(
        err(&MINIMAL.replace(r#""source": "rows","#, "")),
        "the document: missing member `source`"
    );
    // A member the shape does not name: a stray camelCase is refused, never dropped.
    assert_eq!(
        err(&MINIMAL.replace(r#""version": 1,"#, r#""version": 1, "hasNote": true,"#)),
        "the document: unknown member `hasNote`"
    );
    // A role outside the eight.
    assert_eq!(
        err(&MINIMAL.replace(r#""role": "group""#, r#""role": "multi_select""#)),
        "collections[0].fields[1].role: unknown role \"multi_select\""
    );
    // A cardinality outside the two.
    assert_eq!(
        err(&MINIMAL.replace(r#""cardinality": "many""#, r#""cardinality": "some""#)),
        "collections[0].fields[5].link.cardinality: unknown cardinality \"some\""
    );
    // A `link` role with no link member at all.
    assert_eq!(
        err(&MINIMAL.replace(
            r#"{ "id": "blocks", "name": "Blocks", "role": "link",
          "link": { "collection": "task", "cardinality": "many", "symmetric": false } }"#,
            r#"{ "id": "blocks", "name": "Blocks", "role": "link" }"#
        )),
        "collections[0].fields[5]: a `link` field must declare its `link` member"
    );
    // A non-link role carrying a link member: refused rather than silently ignored.
    assert_eq!(
        err(&MINIMAL.replace(
            r#"{ "id": "note", "name": "Note", "role": "scalar", "link": null }"#,
            r#"{ "id": "note", "name": "Note", "role": "scalar",
               "link": { "collection": "task", "cardinality": "one", "symmetric": true } }"#
        )),
        "collections[0].fields[6]: only a `link` field may declare a `link` member"
    );
    // A duplicate collection id.
    assert_eq!(
        err(&MINIMAL.replacen(
            r#""collections": ["#,
            r#""collections": [{"id": "task", "name": "Dup", "titleField": "name", "fields": []}, "#,
            1,
        )),
        "collections[1]: duplicate collection id `task`"
    );
    // A duplicate field id inside one collection.
    assert_eq!(
        err(&MINIMAL.replace(
            r#"{ "id": "state", "name": "State", "role": "group", "link": null }"#,
            r#"{ "id": "name", "name": "State", "role": "group", "link": null }"#
        )),
        "collections[0].fields[1]: duplicate field id `name`"
    );
    // A duplicate record id.
    assert_eq!(
        err(&MINIMAL.replace(
            r#""records": [
    { "id": "r1""#,
            r#""records": [
    { "id": "r1", "collection": "task", "deleted": false, "updatedAt": 1, "values": {} },
    { "id": "r1""#
        )),
        "records[1]: duplicate record id `r1`"
    );
    // A wrong type where a string is named.
    assert_eq!(
        err(&MINIMAL.replace(r#""id": "r1""#, r#""id": 1"#)),
        "records[0].id: expected a string"
    );
    assert_eq!(
        err(&MINIMAL.replace(r#""deleted": false"#, r#""deleted": "no""#)),
        "records[0].deleted: expected a boolean"
    );
    // A number that is not a plain integer where an integer is named.
    assert_eq!(
        err(&MINIMAL.replace(r#""updatedAt": 1700000000"#, r#""updatedAt": 1.5"#)),
        "records[0].updatedAt: expected a plain non-negative integer"
    );
    // A value that is not JSON at all: the contract's own strict RFC 8259 reader,
    // `canonical_json::parse`, byte and all — one parser, not a second one.
    assert_eq!(
        read("{").unwrap_err().to_string(),
        "not JSON at byte 1: expected a key"
    );
}
