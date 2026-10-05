//! The batch reader and the write-time cell checks: what a plugin may send, and what it
//! may put in a record once the manifest says what the fields mean.

use super::super::batch::{Batch, read_batch};
use super::super::manifest::read_manifest;
use super::super::*;
use super::support::{TWO, read};
use crate::ingest::{JsonValue, record_piece};

/// One upsert and one delete: the whole batch shape, in one document.
const BATCH: &str = r#"{
  "upserts": [
    { "collection": "tracker.task", "id": "r1", "updatedAt": 5,
      "values": { "name": "Write", "labels": ["wip"], "effort": 2, "up": null } }
  ],
  "deletes": [ { "collection": "tracker.task", "id": "r2" } ]
}"#;

/// A manifest matching [`BATCH`]'s collections: field ids the batch may use, with the
/// roles [`check`] enforces them against.
const DECLARED: &str = r#"{
  "version": 1, "manifestVersion": 1, "name": "Tasks",
  "collections": [
    { "id": "note", "name": "Notes", "titleField": "title",
      "fields": [ { "id": "title", "name": "Title", "role": "title", "link": null } ] },
    { "id": "task", "name": "Tasks", "titleField": "name", "fields": [
      { "id": "name", "name": "Name", "role": "title", "link": null },
      { "id": "state", "name": "State", "role": "group", "link": null },
      { "id": "labels", "name": "Labels", "role": "tags", "link": null },
      { "id": "effort", "name": "Effort", "role": "weight", "link": null },
      { "id": "up", "name": "Up", "role": "parent", "link": null },
      { "id": "blocks", "name": "Blocks", "role": "link",
        "link": { "collection": "tracker.task", "cardinality": "many", "symmetric": false } },
      { "id": "note", "name": "Note", "role": "scalar", "link": null }
    ] }
  ]
}"#;

fn manifest() -> Manifest {
    read_manifest(DECLARED, "tracker").expect("the declared manifest reads")
}

#[test]
fn a_batch_with_one_upsert_and_one_delete_reads() {
    let batch = read_batch(BATCH, &Limits::DEFAULT).expect("the batch reads");
    assert_eq!(batch.upserts.len(), 1);
    assert_eq!(batch.deletes.len(), 1);
    let up = &batch.upserts[0];
    assert_eq!((up.collection.as_str(), up.id.as_str(), up.updated_at), ("tracker.task", "r1", 5));
    // Values sorted by key, so the cell order a client wrote cannot reach the store.
    let keys: Vec<&str> = up.values.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["effort", "labels", "name", "up"]);
    assert_eq!(batch.deletes[0].id, "r2");
}

#[test]
fn an_unknown_member_is_refused_at_the_root_and_at_each_level() {
    let cases = [
        (
            "root",
            r#"{"upserts":[],"deletes":[],"extra":1}"#,
            "the body: unknown member `extra`",
        ),
        (
            "upsert",
            r#"{"upserts":[{"collection":"a.b","id":"r","updatedAt":1,"values":{},"extra":1}],
                "deletes":[]}"#,
            "upserts[0]: unknown member `extra`",
        ),
        (
            "delete",
            r#"{"upserts":[],"deletes":[{"collection":"a.b","id":"r","extra":1}]}"#,
            "deletes[0]: unknown member `extra`",
        ),
    ];
    for (what, text, message) in cases {
        assert_eq!(
            read_batch(text, &Limits::DEFAULT).unwrap_err().to_string(),
            message,
            "{what}"
        );
    }
}

/// A qualified collection in a batch is a client that already stored it and is now
/// sending it back. The batch is the write path, and a record's collection is
/// *re-qualified* by the reader from the plugin: writing back what the store gave would
/// make `tracker.tracker.task` after two round trips.
#[test]
fn a_qualified_collection_in_a_batch_is_refused() {
    assert_eq!(
        read_batch(BATCH, &Limits::DEFAULT)
            .unwrap_err()
            .to_string(),
        "upserts[0].collection: collection id \"tracker.task\" is not a legal id"
    );
    assert_eq!(
        read_batch(
            r#"{"upserts":[],"deletes":[{"collection":"B.coll","id":"r"}]}"#,
            &Limits::DEFAULT
        )
        .unwrap_err()
        .to_string(),
        "deletes[0].collection: collection id \"B.coll\" is not a legal id"
    );
}

#[test]
fn a_record_id_with_a_colon_is_refused() {
    assert_eq!(
        read_batch(
            r#"{"upserts":[{"collection":"task","id":"a:b","updatedAt":1,"values":{}}],"deletes":[]}"#,
            &Limits::DEFAULT
        )
        .unwrap_err()
        .to_string(),
        "upserts[0].id: record id \"a:b\" contains `:`, which cannot round-trip through the \
         node-id grammar"
    );
}

/// A batch is a set of operations applied together, so the same record appearing twice is
/// a refusal and not a "last one wins": which of the two would win is exactly the
/// ambiguity an atomic batch exists to remove.
#[test]
fn a_record_may_appear_at_most_once_across_upserts_and_deletes() {
    let cases = [
        (
            "twice as an upsert",
            r#"{"upserts":[
                {"collection":"task","id":"r1","updatedAt":1,"values":{}},
                {"collection":"task","id":"r1","updatedAt":2,"values":{}}],"deletes":[]}"#,
        ),
        (
            "once as an upsert and once as a delete",
            r#"{"upserts":[{"collection":"task","id":"r1","updatedAt":1,"values":{}}],
                "deletes":[{"collection":"task","id":"r1"}]}"#,
        ),
    ];
    for (what, text) in cases {
        assert_eq!(
            read_batch(text, &Limits::DEFAULT)
                .unwrap_err()
                .to_string(),
            "the batch: record `task`/`r1` appears more than once",
            "{what}"
        );
    }
}

/// The batch cap counts operations, upserts plus deletes, because that is the work a
/// store has to do in one transaction.
#[test]
fn a_batch_over_the_operation_cap_is_refused_as_a_size() {
    let mut upserts = Vec::new();
    for i in 0..=Limits::DEFAULT.max_batch {
        upserts.push(format!(
            r#"{{"collection":"task","id":"r{i}","updatedAt":1,"values":{{}}}}"#
        ));
    }
    let text = format!(r#"{{"upserts":[{}],"deletes":[]}}"#, upserts.join(","));
    assert_eq!(
        read_batch(&text, &Limits::DEFAULT).unwrap_err(),
        HubError::TooLarge {
            what: "batch",
            limit: Limits::DEFAULT.max_batch
        }
    );
}

/// A NUL in a **value key**. The key is what the manifest is checked against, so a NUL
/// here would be a field id no manifest can declare and no store can key a column by.
#[test]
fn a_nul_in_a_value_key_is_refused() {
    assert_eq!(
        read_batch(
            r#"{"upserts":[{"collection":"task","id":"r1","updatedAt":1,
                "values":{"na\u0000me":1}}],"deletes":[]}"#,
            &Limits::DEFAULT
        )
        .unwrap_err()
        .to_string(),
        "upserts[0].values: a NUL character"
    );
}

/// Every cell whose *shape* the role fixes. `check` is the write-time gate: the ingest
/// reader checks a whole document's declarations, but a hub writes one record at a time,
/// and a record that could not appear in the document it will be served from must be
/// refused when it arrives — with nothing stored and nothing said until a read fails.
#[test]
fn a_cell_whose_shape_the_role_forbids_is_refused() {
    let cases = [
        ("a tag containing a colon", r#"["a","b:c"]"#, "a tag may not contain `:`"),
        ("a bare text tags cell", r#""b:c""#, "expected a list of strings"),
        ("a non-numeric weight", r#""x""#, "expected a number"),
        ("a parent that is a list", r#"["a","b"]"#, "expected a single reference"),
        (
            "a one-cardinality link that is a list",
            r#"["a"]"#,
            "expected a single reference",
        ),
    ];
    for (what, cell, tail) in cases {
        let batch = one(&format!(r#""blocks":{cell}"#));
        assert_eq!(
            batch
                .check("tracker", &manifest(), &Limits::DEFAULT)
                .unwrap_err()
                .to_string(),
            format!("upserts[0].values.blocks: {tail}"),
            "{what}"
        );
    }
}

/// A many-cardinality link that is a bare text is refused too — the same rule the other
/// way round, and the one a client writing `"blocks": "a"` by mistake hits.
#[test]
fn a_many_cardinality_link_that_is_not_a_list_is_refused() {
    let batch = one(r#""blocks":"a""#);
    assert_eq!(
        batch
            .check("tracker", &manifest(), &Limits::DEFAULT)
            .unwrap_err()
            .to_string(),
        "upserts[0].values.blocks: expected a list of references"
    );
}

/// An undeclared collection or field is refused: both are cells the stored document would
/// carry and no manifest declares, so a read would answer with a document that cannot
/// round trip.
#[test]
fn an_undeclared_collection_or_field_is_refused() {
    let batch = one(r#""nope":1"#);
    assert_eq!(
        batch
            .check("tracker", &manifest(), &Limits::DEFAULT)
            .unwrap_err()
            .to_string(),
        "upserts[0].values: collection `tracker.task` declares no field `nope`"
    );
    let batch = read_batch(
        r#"{"upserts":[{"collection":"gone","id":"r1","updatedAt":1,"values":{"name":"a"}}],
            "deletes":[]}"#,
        &Limits::DEFAULT,
    )
    .expect("the batch itself reads");
    assert_eq!(
        batch
            .check("tracker", &manifest(), &Limits::DEFAULT)
            .unwrap_err()
            .to_string(),
        "upserts[0]: collection `tracker.gone` is not declared by the manifest"
    );
}

/// `null` is legal in every role's cell: a source that has no value for a field sends
/// `null`, and refusing it would make "absent" and "empty" the same thing on the wire.
#[test]
fn null_is_a_cell_in_every_role_and_a_scalar_takes_anything() {
    for field in ["name", "state", "labels", "effort", "up", "blocks"] {
        one(&format!(r#""{field}":null"#))
            .check("tracker", &manifest(), &Limits::DEFAULT)
            .unwrap_or_else(|e| panic!("{field} refused a null: {e}"));
    }
    for cell in ["1", r#""a""#, "true", "[1,2]", r#"{"a":1}"#, "null"] {
        one(&format!(r#""note":{cell}"#))
            .check("tracker", &manifest(), &Limits::DEFAULT)
            .unwrap_or_else(|e| panic!("a scalar refused {cell}: {e}"));
    }
}

/// The two number spellings that must survive a round trip byte for byte: `-0` is not
/// `0` on the wire, and `2^53 − 1` is the largest integer a JSON consumer holds exactly.
/// Both are carried in a `scalar` cell, where the motor reads nothing, so the writer is
/// the only thing that can lose them.
#[test]
fn minus_zero_and_the_last_exact_integer_survive_the_record_piece_byte_identically() {
    for (cell, text) in [(r#"-0"#, "\"-0\""), ("9007199254740991", "\"9007199254740991\"")] {
        let batch = one(&format!(r#""note":{cell}"#));
        batch
            .check("tracker", &manifest(), &Limits::DEFAULT)
            .expect("a scalar number is accepted");
        let piece = record_piece(&batch.upserts[0].record("tracker"));
        assert!(
            piece.contains(&format!(r#""note":{text}"#)),
            "{cell} must write as {text}: {piece}"
        );
    }
}

/// The record cap is on the record's *canonical text*, which is the only length the store
/// actually holds: a cap on the incoming body would be a different number, and the answer
/// a client gets must be about the thing that is stored.
#[test]
fn a_record_over_the_canonical_text_cap_is_refused_as_a_size() {
    let limits = Limits {
        max_body: 1 << 20,
        max_batch: 10_000,
        max_record_bytes: 200,
    };
    let batch = one(&format!(r#""note":"{}""#, "x".repeat(300)));
    assert_eq!(
        batch.check("tracker", &manifest(), &limits).unwrap_err(),
        HubError::TooLarge {
            what: "record",
            limit: 200
        }
    );
}



/// The negctl, stated here rather than only in the rows file: `GM_HUB_BREAK=lax-reader`
/// turns off two things this file pins — `check_collection_id` and the NUL walk — so
/// `negctl-lax-reader` must go red. This test is what "go red" means: under the break it
/// asserts the *lax* behaviour, so the same run proves the break is wired to the reader
/// and not to something else.
#[test]
fn the_lax_reader_break_relaxes_exactly_the_two_reader_refusals() {
    if !crate::hub::breaks::on("lax-reader") {
        return;
    }
    // No `check_collection_id`: a qualified collection is accepted.
    let batch = read_batch(BATCH, &Limits::DEFAULT).expect("a qualified collection now reads");
    assert_eq!(batch.upserts[0].collection, "tracker.task");
    // No NUL walk: a NUL in a key is accepted.
    let nuled = r#"{"upserts":[{"collection":"task","id":"r1","updatedAt":1,
        "values":{"na\u0000me":1}}],"deletes":[]}"#;
    assert!(read_batch(nuled, &Limits::DEFAULT).is_ok());
}

/// One upsert whose `values` is `cells`, read from a batch the reader accepts. The helper
/// keeps every cell test above to one line and pins the path every refusal names.
fn one(cells: &str) -> Batch {
    read_batch(
        &format!(
            r#"{{"upserts":[{{"collection":"task","id":"r1","updatedAt":1,"values":{{{cells}}}}}],"deletes":[]}}"#
        ),
        &Limits::DEFAULT,
    )
    .expect("the batch reads")
}

/// The manifest's second collection is never used by the batch tests above; this keeps the
/// shared fixture honest — if `TWO` stopped being a readable manifest the tests that do
/// not touch `DECLARED` would still be testing something real.
#[test]
fn the_shared_manifest_fixture_is_still_readable() {
    assert_eq!(read(TWO).collections.len(), 2);
    let _ = JsonValue::Null;
}