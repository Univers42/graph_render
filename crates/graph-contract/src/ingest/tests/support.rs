//! The one document every ingest-contract test shares, and the vocabulary they import.
//!
//! `MINIMAL` carries all eight roles, both cardinalities, a `link` member and a `null`
//! one, so a test about any of them can be written against a single document rather
//! than each carrying its own JSON. The re-exports below are what let every test
//! module be one `use super::support::*;`: one place says which names they share, and
//! a glob import never warns about an unused member.

pub(super) use crate::ingest::{
    Cardinality, Ingest, JsonValue, Role, VERSION, read, read_value, to_json, to_json_value,
};

/// A small but complete document: one collection, all eight roles, one record.
pub(super) const MINIMAL: &str = r#"{
  "version": 1,
  "source": "rows",
  "collections": [
    {
      "id": "task",
      "name": "Tasks",
      "titleField": "name",
      "fields": [
        { "id": "name", "name": "Name", "role": "title", "link": null },
        { "id": "state", "name": "State", "role": "group", "link": null },
        { "id": "labels", "name": "Labels", "role": "tags", "link": null },
        { "id": "effort", "name": "Effort", "role": "weight", "link": null },
        { "id": "up", "name": "Parent", "role": "parent", "link": null },
        { "id": "blocks", "name": "Blocks", "role": "link",
          "link": { "collection": "task", "cardinality": "many", "symmetric": false } },
        { "id": "note", "name": "Note", "role": "scalar", "link": null }
      ]
    }
  ],
  "records": [
    { "id": "r1", "collection": "task", "deleted": false, "updatedAt": 1700000000,
      "values": { "name": "Write", "state": "doing", "labels": ["wip"], "effort": 2 } }
  ]
}"#;

pub(super) fn minimal() -> Ingest {
    read(MINIMAL).expect("the minimal document reads")
}

/// The refusal `text` draws, as its message. Panics on acceptance, so a test that
/// expected a refusal and got a document fails at the call rather than at an assert.
pub(super) fn err(text: &str) -> String {
    match read(text) {
        Ok(doc) => panic!("expected a refusal, read {doc:?}"),
        Err(e) => e.to_string(),
    }
}
