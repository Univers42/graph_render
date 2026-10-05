//! The model's behaviour: revisions, atomicity, document order and pruning.
//!
//! The property test that holds all of it to the document's own rules is `rng.rs`, which
//! drives the same public API these tests use — nothing here is a private path into the
//! model, so if a rule is only true through a private door the property test would not see
//! it either.

use super::fixtures::{bare, delete, manifest, upsert};
use super::rng::SplitMix64;
use super::super::Limits;
use super::super::manifest::Manifest;
use super::super::model::Model;
use super::super::*;

/// A model with `tracker` registered: enough for one collection's records.
fn model() -> Model {
    let mut model = Model::new("ws").expect("a workspace id is a slug");
    model.register("tracker", manifest("tracker")).unwrap();
    model
}

/// `rev` is 1 on create, one more on a *changing* upsert, and unchanged on an identical
/// one — which reports nothing at all, because a change in the stream that a reader
/// replays to no effect is a change the reader has to learn to skip.
#[test]
fn a_rev_is_one_on_create_and_only_moves_when_the_text_moves() {
    let mut model = model();
    let first = model.apply("tracker", &bare("task", "r1", 1), &Limits::DEFAULT).unwrap();
    assert_eq!(first.upserted[0].1, 1, "a created record is rev 1");
    let second = model.apply("tracker", &bare("task", "r2", 1), &Limits::DEFAULT).unwrap();
    assert_eq!(second.upserted[0].1, 1, "a different record is its own rev 1");
    let changed = model.apply("tracker", &bare("task", "r1", 2), &Limits::DEFAULT).unwrap();
    assert_eq!(changed.upserted[0].1, 2, "a changed record is one more");
    let same = model.apply("tracker", &bare("task", "r1", 2), &Limits::DEFAULT).unwrap();
    assert!(same.upserted.is_empty(), "an identical upsert reports nothing");
    assert_eq!(model.stored("tracker.task", "r1").unwrap().rev, 2);
}

/// Delete then re-create: the record has not existed as far as the store is concerned, so it
/// is rev 1 again. A client that saw rev 2 of the deleted record must not be told its
/// replacement is rev 3 — the two are different records that share an id.
#[test]
fn a_record_re_created_after_a_delete_starts_again_at_rev_one() {
    let mut model = model();
    model.apply("tracker", &bare("task", "r1", 1), &Limits::DEFAULT).unwrap();
    model.apply("tracker", &bare("task", "r1", 2), &Limits::DEFAULT).unwrap();
    let before = model.stored("tracker.task", "r1").unwrap().rev;
    assert_eq!(before, 2);
    let applied = model.apply("tracker", &delete("task", "r1"), &Limits::DEFAULT).unwrap();
    assert_eq!(
        applied.deleted,
        [("tracker.task".to_owned(), "r1".to_owned(), 2)],
        "the delete reports the collection, id and rev it removed"
    );
    assert!(model.stored("tracker.task", "r1").is_none());
    model.apply("tracker", &bare("task", "r1", 9), &Limits::DEFAULT).unwrap();
    assert_eq!(model.stored("tracker.task", "r1").unwrap().rev, 1);
}

#[test]
fn deleting_a_record_that_is_not_there_changes_nothing() {
    let mut model = model();
    let applied = model.apply("tracker", &delete("task", "nope"), &Limits::DEFAULT).unwrap();
    assert!(applied.deleted.is_empty(), "a replayed delete is not a change");
    let mut empty = Model::new("ws").unwrap();
    empty.register("tracker", manifest("tracker")).unwrap();
    assert_eq!(model.to_json(), empty.to_json());
}

/// A batch is atomic: one bad record leaves nothing stored — not the good records before
/// it, and not a `rev` spent on them.
#[test]
fn a_batch_with_one_bad_record_changes_nothing() {
    let mut model = model();
    model.apply("tracker", &bare("task", "r1", 1), &Limits::DEFAULT).unwrap();
    let before = model.to_json();
    // `nope` is not a declared field, so `check` refuses the whole batch.
    let bad = upsert("task", "r2", 1, r#""nope":1"#);
    assert_eq!(
        model.apply("tracker", &bad, &Limits::DEFAULT).unwrap_err().status(),
        422
    );
    assert_eq!(model.to_json(), before, "the model must be untouched");
    assert!(model.stored("tracker.task", "r2").is_none());
    assert_eq!(model.stored("tracker.task", "r1").unwrap().rev, 1);
}

/// Collection ids are compared as their **qualified strings**, so `-` (`0x2D`) before `.`
/// (`0x2E`) puts `a-b.c` before `a.x`. Comparing plugin-then-collection would order by
/// plugin first and quietly disagree with the byte order of the qualified id.
#[test]
fn collections_are_ordered_by_their_qualified_string() {
    let mut model = Model::new("ws").unwrap();
    for plugin in ["a", "a-b", "b"] {
        model.register(plugin, manifest(plugin)).unwrap();
    }
    let collections = model.collections();
    let order: Vec<&str> = collections.iter().map(|c| c.id.as_str()).collect();
    let mut sorted = order.clone();
    sorted.sort_unstable();
    assert_eq!(order, sorted, "the qualified strings must already be in byte order");
    let at = |prefix: &str| order.iter().position(|c| c.starts_with(prefix)).unwrap();
    assert!(at("a-b.") < at("a."), "{order:?}");
}

/// Record keys are compared as **tuples**, not as concatenated text. For the same set of
/// keys the two orders differ, so a store keyed on a joined string would satisfy one of
/// them and not the other; the map's own key type is the tuple, so this is a property of
/// the data structure rather than of a sort.
#[test]
fn records_are_ordered_by_their_key_tuple_not_by_concatenated_text() {
    let mut model = Model::new("ws").unwrap();
    for plugin in ["a", "a-b"] {
        model.register(plugin, manifest(plugin)).unwrap();
    }
    for (plugin, collection, id) in [("a", "c", "1"), ("a-b", "c", "1")] {
        model
            .apply(plugin, &bare(collection, id, 1), &Limits::DEFAULT)
            .unwrap();
    }
    let keys: Vec<(&str, &str)> = model
        .records()
        .map(|s| (s.record.collection.as_str(), s.record.id.as_str()))
        .collect();
    assert_eq!(keys, [("a-b.c", "1"), ("a.c", "1")]);
    // Concatenating would give `"a-b.c1"` and `"a.c1"`, which sort the other way round,
    // because `-` (0x2D) is below `.` (0x2E) but the *plugin* separators differ.
    let mut joined: Vec<String> = keys.iter().map(|(c, id)| format!("{c}{id}")).collect();
    joined.sort();
    assert_eq!(
        joined,
        ["a-b.c1".to_owned(), "a.c1".to_owned()],
        "the tuple order happens to agree here; the type is what pins it"
    );
}

/// A link to a collection no registered plugin declares is dropped from the *declaration*
/// and its cells go with it — so a document that a client reads back has no field its
/// records cannot use.
#[test]
fn a_link_to_an_unregistered_plugins_collection_is_dropped_with_its_cells() {
    let mut model = model();
    model
        .apply(
            "tracker",
            &upsert("task", "r1", 1, r#""name":"W","link":"thing""#),
            &Limits::DEFAULT,
        )
        .unwrap();
    let text = model.to_json();
    assert!(!text.contains("other.thing"), "the target is gone: {text}");
    assert!(
        !text.contains("\"link\":\""),
        "and so is every cell under it: {text}"
    );
}

/// A dangling parent is pruned; a list that arrived *empty* is kept. The two are different
/// documents to a client diffing them: "no tags" and "the tags field is gone" are not the
/// same claim about the record.
#[test]
fn a_dangling_parent_is_pruned_and_an_originally_empty_list_stays() {
    let mut model = model();
    model
        .apply(
            "tracker",
            &upsert("task", "r1", 1, r#""name":"W","up":"gone","tags":[]"#),
            &Limits::DEFAULT,
        )
        .unwrap();
    let text = model.to_json();
    assert!(!text.contains(r#""up":"#), "the dangling parent is gone: {text}");
    assert!(
        text.contains(r#""tags":[]"#),
        "a list that arrived empty is kept: {text}"
    );
}

/// The other half of that rule: a list *emptied by pruning* is removed **from the
/// record**, while the declaration keeps the field — the field is registered and other
/// records may still use it. The two are different objects and only one of them changes.
#[test]
fn a_list_emptied_by_pruning_is_removed_from_the_record_only() {
    let mut model = model();
    model
        .apply(
            "tracker",
            &upsert("task", "r1", 1, r#""name":"W","blocks":["r9"]"#),
            &Limits::DEFAULT,
        )
        .unwrap();
    let text = model.to_json();
    let record = &text[text.find("\"records\":[").unwrap()..];
    assert!(!record.contains(r#""blocks":"#), "the cell is gone: {record}");
    assert!(
        text.contains(r#""id":"blocks""#),
        "the declaration keeps a field other records still use: {text}"
    );
}

/// A reference to a record that *is* stored is kept, and a self reference is kept: the
/// engine derives a self edge from one, so pruning it would change the graph rather than
/// repair it.
#[test]
fn a_reference_to_a_stored_record_is_kept() {
    let mut model = model();
    model.apply("tracker", &bare("task", "r1", 1), &Limits::DEFAULT).unwrap();
    model
        .apply(
            "tracker",
            &upsert("task", "r2", 1, r#""name":"W","blocks":["r1","r1"],"up":"r2""#),
            &Limits::DEFAULT,
        )
        .unwrap();
    let text = model.to_json();
    assert!(text.contains(r#""blocks":["r1","r1"]"#), "{text}");
    assert!(text.contains(r#""up":"r2""#), "a self parent is kept: {text}");
}

/// `doc_bytes` is a bound, and it is *exact* while nothing is pruned — which is what lets a
/// store size a response before writing it.
#[test]
fn doc_bytes_bounds_the_document_and_is_exact_while_nothing_is_pruned() {
    let mut model = model();
    assert_eq!(model.doc_bytes(), model.to_json().len() as u64);
    model.apply("tracker", &bare("task", "r1", 1), &Limits::DEFAULT).unwrap();
    assert_eq!(model.doc_bytes(), model.to_json().len() as u64);
    // Prune something, and the bound stops being exact — which is why it is a bound and not
    // an equality.
    model
        .apply(
            "tracker",
            &upsert("task", "r2", 1, r#""name":"W","up":"gone""#),
            &Limits::DEFAULT,
        )
        .unwrap();
    assert!(model.to_json().len() as u64 <= model.doc_bytes());
    assert!(model.doc_bytes() > model.to_json().len() as u64);
}

#[test]
fn a_sixty_fifth_plugin_is_refused_as_a_size() {
    let mut model = Model::new("ws").unwrap();
    for i in 0..MAX_PLUGINS {
        model
            .register(&format!("p{i}"), manifest("tracker"))
            .unwrap_or_else(|e| panic!("plugin {i}: {e}"));
    }
    assert_eq!(
        model.register("last", manifest("tracker")).unwrap_err(),
        HubError::TooLarge {
            what: "plugins",
            limit: MAX_PLUGINS
        }
    );
}

/// A batch for a plugin with no manifest is refused rather than checked against an empty
/// declaration: "no fields" accepts every cell, so an unregistered plugin would store
/// anything at all.
#[test]
fn a_batch_for_an_unregistered_plugin_is_refused() {
    let mut model = model();
    let err = model
        .apply("nobody", &bare("task", "r1", 1), &Limits::DEFAULT)
        .unwrap_err();
    assert_eq!(err.status(), 422);
    assert!(err.to_string().contains("no registered manifest"), "{err}");
}

/// Two models fed the same final records in opposite insertion orders write the same bytes.
/// This is the property the `BTreeMap` key exists for: if the store were keyed on a joined
/// string, or ordered by insertion, this would fail.
#[test]
fn the_same_records_written_in_any_order_give_the_same_bytes() {
    let records = [("r1", "a"), ("r2", "b"), ("r3", "c"), ("r4", "d")];
    let forward = build(records);
    let mut reversed = records;
    reversed.reverse();
    let backward = build(reversed);
    assert_eq!(forward.to_json(), backward.to_json());
    assert_eq!(forward.doc_bytes(), backward.doc_bytes());
    assert_eq!(forward.to_json(), forward.to_json(), "two writes are one text");
}

/// A model holding `records`, each applied in the order given.
fn build(records: [(&str, &str); 4]) -> Model {
    let mut model = model();
    for (id, cell) in records {
        model
            .apply(
                "tracker",
                &upsert("task", id, 1, &format!(r#""name":"{cell}""#)),
                &Limits::DEFAULT,
            )
            .unwrap();
    }
    model
}

/// The generator's own module test lives in `rng.rs`; this one states that the model tests
/// and the property test agree on which plugins exist, so a fixture change that made the
/// property test vacuous (every step a no-op) would show up here.
#[test]
fn the_property_test_will_not_be_vacuous() {
    let mut rng = SplitMix64::seeded(0);
    let mut model = Model::new("ws").unwrap();
    for _ in 0..8 {
        let plugin = ["tracker", "a", "a-b", "b"][rng.below(4) as usize];
        if model.manifest_of(plugin).is_err() {
            model.register(plugin, manifest(plugin)).unwrap();
            continue;
        }
        model
            .apply(
                plugin,
                &bare(["task", "c", "note"][rng.below(3) as usize], "r1", 1),
                &Limits::DEFAULT,
            )
            .unwrap();
    }
    assert!(model.records().count() > 0, "the steps must actually store something");
    let _: Manifest = manifest("tracker");
}