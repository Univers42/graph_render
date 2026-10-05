//! The fixtures the model tests build states from: one manifest that declares every role
//! the pruning rules care about, and the one-line batch builder they use.
//!
//! Its own file because `batch.rs` has a manifest for the *write-time cell* rules and this
//! one has a manifest for the *pruning* rules, and the two declare different fields on
//! purpose: `batch.rs` needs a many-link, this needs a link to a collection no plugin
//! registers and a tags list. Merging them would give one manifest that satisfies neither
//! test's intent as directly.

use super::super::batch::{Batch, read_batch};
use super::super::{Limits, Manifest, read_manifest};

/// One collection per plugin, declaring every role the pruning and ordering rules mention:
///
/// | Field | Role | Why it is here |
/// |---|---|---|
/// | `name` | title | the record must have one to read |
/// | `state` | group | a second scalar role, to prove the order test is not about one field |
/// | `tags` | tags | the *list* case: emptied by pruning vs originally empty |
/// | `link` | link → `tracker.other` | the unregistered-target case; no plugin declares it |
/// | `blocks` | link → same collection, many | the registered-target case, so pruning keeps it |
/// | `up` | parent | the dangling-reference case |
/// | `note` | scalar | the number case, where `-0` and `2^53 − 1` live |
pub(super) const MODEL: &str = r#"{
  "version": 1,
  "manifestVersion": 1,
  "name": "Tasks",
  "collections": [
    { "id": "task", "name": "Tasks", "titleField": "name", "fields": [
      { "id": "name", "name": "Name", "role": "title", "link": null },
      { "id": "state", "name": "State", "role": "group", "link": null },
      { "id": "tags", "name": "Tags", "role": "tags", "link": null },
      { "id": "link", "name": "Link", "role": "link",
        "link": { "collection": "other.thing", "cardinality": "one", "symmetric": false } },
      { "id": "blocks", "name": "Blocks", "role": "link",
        "link": { "collection": "task", "cardinality": "many", "symmetric": false } },
      { "id": "up", "name": "Up", "role": "parent", "link": null },
      { "id": "note", "name": "Note", "role": "scalar", "link": null }
    ] },
    { "id": "c", "name": "C", "titleField": "name", "fields": [
      { "id": "name", "name": "Name", "role": "title", "link": null }
    ] },
    { "id": "note", "name": "Note", "titleField": "name", "fields": [
      { "id": "name", "name": "Name", "role": "title", "link": null }
    ] }
  ]
}"#;

/// [`MODEL`] read for `plugin`: the declaration every model test registers.
pub(super) fn manifest(plugin: &str) -> Manifest {
    read_manifest(MODEL, plugin).unwrap_or_else(|e| panic!("the model manifest reads: {e}"))
}

/// One upsert in `collection`, with `cells` as its whole `values` map. The one-line batch
/// builder every model test uses, so a test states the *state* it wants rather than the
/// JSON that spells it.
pub(super) fn upsert(collection: &str, id: &str, updated_at: u32, cells: &str) -> Batch {
    read_batch(
        &format!(
            r#"{{"upserts":[{{"collection":"{collection}","id":"{id}","updatedAt":{updated_at},
              "values":{{{cells}}}}}],"deletes":[]}}"#
        ),
        &Limits::DEFAULT,
    )
    .expect("the batch reads")
}

/// The same, with no cells: a record that is nothing but its identity, which is what a
/// delete-then-re-create test needs so the two records' texts differ for one reason only.
pub(super) fn bare(collection: &str, id: &str, updated_at: u32) -> Batch {
    upsert(collection, id, updated_at, r#""name":"Write""#)
}

/// One delete in `collection`.
pub(super) fn delete(collection: &str, id: &str) -> Batch {
    read_batch(
        &format!(r#"{{"upserts":[],"deletes":[{{"collection":"{collection}","id":"{id}"}}]}}"#),
        &Limits::DEFAULT,
    )
    .expect("the delete reads")
}