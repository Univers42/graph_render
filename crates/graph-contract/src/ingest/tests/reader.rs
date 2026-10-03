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
fn a_field_without_the_link_member_is_refused_naming_it() {
    // `link` is required on every field, as every member is: a present `null` is how a
    // non-`link` field says it has no target. The schema says so too
    // (`docs/contract/ingest-schema.json` requires `link`, and `required_link` in
    // `ingest/schema.rs` puts it there deliberately), so a reader that accepted a
    // missing one would read a document its own schema refuses.
    assert_eq!(
        err(&MINIMAL.replace(
            r#"{ "id": "name", "name": "Name", "role": "title", "link": null }"#,
            r#"{ "id": "name", "name": "Name", "role": "title" }"#
        )),
        "collections[0].fields[0]: missing member `link`"
    );
}

#[test]
fn an_integer_past_the_exact_range_is_refused_rather_than_rounded() {
    // `JsonValue::Number` is an `f64`, so an integer the `f64` cannot hold lands on a
    // neighbour: reading `9007199254740993` would make the cell `9007199254740992`, and
    // the document's own bytes would be rewritten with nothing said.
    let with_effort = |n: &str| MINIMAL.replace(r#""effort": 2"#, &format!(r#""effort": {n}"#));
    let path = "records[0].values.effort: an integer past 9007199254740992 cannot be read exactly";
    assert_eq!(err(&with_effort("9007199254740993")), path);
    assert_eq!(err(&with_effort("-9007199254740993")), path);
    // One past the writer's own spelling of `1e21`, which is a bare integer no `f64` holds
    // exactly: refused too, because it is not the text the writer writes for that number.
    assert_eq!(err(&with_effort("1000000000000000000001")), path);
    // 2^53 itself is exact, so it reads as itself.
    assert_eq!(
        read(&with_effort("9007199254740992"))
            .expect("2^53 is exact")
            .records[0]
            .value("effort"),
        Some(&JsonValue::Number(9007199254740992.0))
    );
    // The writer's own text for a whole `f64` past 2^53 stays readable — it is the only
    // spelling of the number the writer meant, and refusing it would break
    // write-then-read for `1e21` (`ingest::tests::writer`).
    assert_eq!(
        read(&with_effort("1000000000000000000000"))
            .expect("the writer's own spelling reads")
            .records[0]
            .value("effort"),
        Some(&JsonValue::Number(1e21))
    );
    // A large *float* is a different fault from a large integer — it says its own
    // precision in an exponent — and it is read, not refused.
    assert_eq!(
        read(&with_effort("1e30")).expect("a float reads").records[0].value("effort"),
        Some(&JsonValue::Number(1e30))
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
    // A `link` role with no target. The member is *present* and `null`: omitting it is a
    // different fault, refused earlier as a missing member (the test above), because a
    // `link` role that has to say nothing at all is a different mistake from one that
    // says `null`.
    assert_eq!(
        err(&MINIMAL.replace(
            r#"{ "id": "blocks", "name": "Blocks", "role": "link",
          "link": { "collection": "task", "cardinality": "many", "symmetric": false } }"#,
            r#"{ "id": "blocks", "name": "Blocks", "role": "link", "link": null }"#
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
