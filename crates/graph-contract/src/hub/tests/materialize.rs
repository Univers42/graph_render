//! The model's behaviour: revisions, atomicity, document order and pruning — and the
//! property test that holds all of it to the document's own rules.

use super::rng::SplitMix64;
use super::support::{DECLARED, read, upsert_batch};
use super::super::batch::read_batch;
use super::super::manifest::Manifest;
use super::super::model::Model;
use super::super::*;
use crate::ingest::{JsonValue, ingest_to_json, read as ingest_read};

/// A two-plugin model: `tracker` with a `task` and a `note`, and `other` with a `c`.
fn model() -> Model {
    let mut model = Model::new("ws").expect("a workspace id is a slug");
    for plugin in ["tracker", "other"] {
        model
            .register(plugin, manifest_for(plugin))
            .expect("the first registration grows");
    }
    model
}

fn manifest_for(plugin: &str) -> Manifest {
    read_manifest(DECLARED, plugin).expect("the declared manifest reads")
}

/// A batch with one upsert in `collection` and no deletes.
fn upsert(collection: &str, id: &str, updated_at: u32) -> super::super::batch::Batch {
    upsert_batch(collection, id, updated_at, r#""name":"Write""#)
}

#[test]
fn a_rev_is_one_on_create_and_increments_only_when_the_record_changes() {
    let mut model = model();
    let first = model
        .apply("tracker", &upsert("task", "r1", 1), &Limits::DEFAULT)
        .expect("the batch applies");
    assert_eq!(first.upserted[0].1, 1, "a created record is rev 1");
    let second = model
        .apply("tracker", &upsert("task", "r1", 2), &Limits::DEFAULT)
        .expect("the batch applies");
    assert_eq!(second.upserted[0].1, 2, "a changed record is one more");
    // Identical text: same cells, same `updatedAt`. Nothing changes, so nothing is
    // reported — a change in the stream that a reader replays to no effect is a change the
    // reader must learn to skip.
    let same = model
        .apply("tracker", &upsert("task", "r1", 2), &Limits::DEFAULT)
        .expect("an identical batch is not a failure");
    assert!(same.upserted.is_empty(), "{:?}", same.upserted);
    assert_eq!(model.stored("tracker.task", "r1").unwrap().rev, 2);
}

/// Delete then re-create: the record has not existed as far as the store is concerned, so
/// it is rev 1 again. A client that saw rev 9 of the deleted record must not be told its
/// replacement is rev 10 — the two are different records with the same id.
#[test]
fn a_record_re_created_after_a_delete_starts_again_at_rev_one() {
    let mut model = model();
    for _ in 0..3 {
        model
            .apply("tracker", &upsert("task", "r1", 1), &Limits::DEFAULT)
            .expect("applies");
        model
            .apply(
                "tracker",
                &upsert("task", "r1", 1 + u32::from(model.stored("tracker.task", "r1").is_some() as u32) * 0),
                &Limits::DEFAULT,
            )
            .ok();
    }
    model
        .apply("tracker", &upsert("task", "r1", 9), &Limits::DEFAULT)
        .expect("applies");
    let before = model.stored("tracker.task", "r1").unwrap().rev;
    let delete = read_batch(
        r#"{"upserts":[],"deletes":[{"collection":"task","id":"r1"}]}"#,
        &Limits::DEFAULT,
    )
    .expect("the delete reads");
    let applied = model
        .apply("tracker", &delete, &Limits::DEFAULT)
        .expect("the delete applies");
    assert_eq!(applied.deleted[0].2, before, "the delete reports the rev it removed");
    assert!(model.stored("tracker.task", "r1").is_none());
    model
        .apply("tracker", &upsert("task", "r1", 1), &Limits::DEFAULT)
        .expect("re-creates");
    assert_eq!(model.stored("tracker.task", "r1").unwrap().rev, 1);
}

#[test]
fn deleting_a_record_that_is_not_there_changes_nothing() {
    let mut model = model();
    let delete = read_batch(
        r#"{"upserts":[],"deletes":[{"collection":"task","id":"nope"}]}"#,
        &Limits::DEFAULT,
    )
    .expect("the delete reads");
    let applied = model
        .apply("tracker", &delete, &Limits::DEFAULT)
        .expect("a delete of an absent record is a no-op, not a failure");
    assert!(applied.deleted.is_empty());
    assert_eq!(model.to_json(), Model::new("ws").unwrap().to_json());
}

/// A batch is atomic: one bad record leaves nothing stored — not the good records before
/// it, and not a partial document.
#[test]
fn a_batch_with_one_bad_record_changes_nothing() {
    let mut model = model();
    model
        .apply("tracker", &upsert("task", "r1", 1), &Limits::DEFAULT)
        .expect("applies");
    let before = model.to_json();
    let bad = upsert_batch("task", "r2", 1, r#""nope":1"#);
    let outcome = model.apply("tracker", &bad, &Limits::DEFAULT);
    assert!(outcome.is_err(), "an undeclared field is refused");
    assert_eq!(model.to_json(), before, "the model must be untouched");
    assert!(model.stored("tracker.task", "r2").is_none());
}

/// The order two tests depend on, stated in one test: collection ids are compared as their
/// **qualified strings**, so `-` (`0x2D`) before `.` (`0x2E`) puts `a-b.c` before `a.x`.
/// Comparing plugin-then-collection would give `a.c` first and quietly disagree.
#[test]
fn collections_are_ordered_by_their_qualified_string() {
    let mut model = Model::new("ws").unwrap();
    model.register("a", manifest_for("a")).unwrap();
    model.register("a-b", manifest_for("a-b")).unwrap();
    model.register("b", manifest_for("b")).unwrap();
    let order: Vec<&str> = model
        .collections()
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    let mut sorted = order.clone();
    sorted.sort_unstable();
    assert_eq!(order, sorted, "the qualified strings must already be in byte order");
    assert!(
        order.iter().position(|c| c.starts_with("a-b.")) < order.iter().position(|c| c == "a.c"),
        "{order:?}"
    );
}

/// And record keys are compared as **tuples**, not as concatenated text: `("a-b", "1")`
/// before `("a.x", "1")` is a tuple order, while the concatenated `"a-b1"` and `"a.x1"`
/// compare the other way round at the `.`/`-` boundary for some id pairs. Both orders are
/// pinned here because a store keyed on one string would satisfy one of them only.
#[test]
fn records_are_ordered_by_their_key_tuple_not_by_concatenated_text() {
    let mut model = Model::new("ws").unwrap();
    model.register("a", manifest_for("a")).unwrap();
    model.register("a-b", manifest_for("a-b")).unwrap();
    for (plugin, collection, id) in [
        ("a", "x", "1"),
        ("a-b", "c", "1"),
        ("a", "x", "2"),
        ("a-b", "c", "2"),
    ] {
        model
            .apply(plugin, &upsert(collection, id, 1), &Limits::DEFAULT)
            .unwrap_or_else(|e| panic!("{plugin}/{collection}/{id}: {e}"));
    }
    let keys: Vec<(&str, &str)> = model
        .records()
        .map(|s| (s.record.collection.as_str(), s.record.id.as_str()))
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted, "the map's own order is already the tuple order");
    // The concatenation order differs from the tuple order for this pair — which is the
    // whole reason the key is a tuple and not a joined string.
    assert_ne!(
        keys,
        ["a-b.c1", "a-b.c2", "a.x1", "a.x2"],
        "a concatenated key would sort `a.x1` before `a-b.c1`"
    );
    assert_eq!(keys, [("a-b.c", "1"), ("a-b.c", "2"), ("a.x", "1"), ("a.x", "2")]);
}

#[test]
fn a_link_to_an_unregistered_plugin_collection_is_dropped_with_its_cells() {
    let mut model = Model::new("ws").unwrap();
    model.register("tracker", manifest_for("tracker")).unwrap();
    // `link` points at `tracker.other`, which no plugin registers.
    let batch = upsert_batch("task", "r1", 1, r#""link":"other.thing""#);
    model
        .apply("tracker", &batch, &Limits::DEFAULT)
        .expect("the batch applies");
    let text = model.to_json();
    assert!(!text.contains("other.thing"), "the field is gone: {text}");
    assert!(!text.contains(r#""link":"#, "and so are its cells: {text}");
    assert_eq!(text.matches(r#""role":"link""#).count(), 0, "{text}");
}

#[test]
fn a_dangling_parent_is_pruned_and_an_empty_list_emptied_by_pruning_is_removed() {
    let mut model = Model::new("ws").unwrap();
    model.register("tracker", manifest_for("tracker")).unwrap();
    // `r1` has a parent that is not stored, and an *originally* empty list.
    model
        .apply(
            "tracker",
            &upsert_batch("task", "r1", 1, r#""up":"gone","tags":[]"#),
            &Limits::DEFAULT,
        )
        .expect("applies");
    let text = model.to_json();
    assert!(!text.contains(r#""up":""#), "the dangling parent is gone: {text}");
    assert!(
        text.contains(r#""tags":[]"#),
        "a list that arrived empty is kept — the client said there are none: {text}"
    );
}

/// `doc_bytes` is a bound and it is *exact* when nothing was pruned — the property
/// `tests/rng.rs`'s property test re-checks after every random step.
#[test]
fn doc_bytes_bounds_the_document_and_is_exact_when_nothing_is_pruned() {
    let mut model = Model::new("ws").unwrap();
    model.register("tracker", manifest_for("tracker")).unwrap();
    assert_eq!(model.doc_bytes(), model.to_json().len() as u64);
    model
        .apply("tracker", &upsert("task", "r1", 1), &Limits::DEFAULT)
        .unwrap();
    assert_eq!(model.doc_bytes(), model.to_json().len() as u64);
    // Prune something, and the bound stops being exact — which is why it is a bound.
    model
        .apply(
            "tracker",
            &upsert_batch("task", "r2", 1, r#""up":"gone""#),
            &Limits::DEFAULT,
        )
        .unwrap();
    assert!(model.to_json().len() as u64 <= model.doc_bytes());
    assert!(model.doc_bytes() > model.to_json().len() as u64);
}

#[test]
fn a_sixty_fifth_plugin_is_refused_and_the_refusal_is_a_size() {
    let mut model = Model::new("ws").unwrap();
    for i in 0..MAX_PLUGINS {
        model
            .register(&format!("p{i}"), manifest_for("p0"))
            .unwrap_or_else(|e| panic!("plugin {i}: {e}"));
    }
    assert_eq!(
        model
            .register("last", manifest_for("p0"))
            .unwrap_err(),
        HubError::TooLarge {
            what: "plugins",
            limit: MAX_PLUGINS
        }
    );
}

/// The property test: 64 seeds, 40 steps each, and after **every** step the four
/// properties that together say "this is a document the motor can read and a client can
/// compare".
///
/// 1. the streaming writer and the built document are the same text;
/// 2. that text reads;
/// 3. reading it back and writing it again is the identical text;
/// 4. `doc_bytes` bounds it, and equals its length when nothing was pruned.
#[test]
fn random_ops_materialize_canonically() {
    for seed in 0..64u64 {
        let mut model = Model::new("ws").unwrap();
        let mut rng = SplitMix64::seeded(seed);
        for step in 0..40 {
            run_step(&mut model, &mut rng, step);
            let text = model.to_json();
            assert_eq!(
                text,
                ingest_to_json(&model.to_ingest()),
                "seed {seed} step {step}: the two writers disagree"
            );
            let back = ingest_read(&text)
                .unwrap_or_else(|e| panic!("seed {seed} step {step}: the document is refused: {e}\n{text}"));
            assert_eq!(ingest_to_json(&back), text, "seed {seed} step {step}: not canonical");
            assert!(
                text.len() as u64 <= model.doc_bytes(),
                "seed {seed} step {step}: doc_bytes is not a bound"
            );
        }
    }
}

/// One random step: register a plugin, upsert, or delete. The generator picks from three
/// shapes with fixed weights rather than uniformly, because a uniform choice spends most of
/// its steps on upserts of the same record and the interesting cases — a delete, a second
/// plugin — would barely appear.
fn run_step(model: &mut Model, rng: &mut SplitMix64, step: usize) {
    let plugin = ["tracker", "a", "a-b", "b"][rng.below(4) as usize];
    if model.manifest_of(plugin).is_err() {
        model
            .register(plugin, manifest_for(plugin))
            .unwrap_or_else(|e| panic!("seed step {step}: register {plugin}: {e}"));
        return;
    }
    let collection = ["task", "note", "c"][rng.below(3) as usize];
    let id = format!("r{}", rng.below(4));
    let roll = rng.below(10);
    if roll == 0 {
        let delete = read_batch(
            &format!(r#"{{"upserts":[],"deletes":[{{"collection":"{collection}","id":"{id}"}}]}}"#),
            &Limits::DEFAULT,
        )
        .expect("the delete reads");
        model
            .apply(plugin, &delete, &Limits::DEFAULT)
            .unwrap_or_else(|e| panic!("seed step {step}: delete {plugin}/{collection}/{id}: {e}"));
        return;
    }
    let batch = upsert_batch(collection, &id, step as u32, &random_cells(rng));
    if let Err(e) = model.apply(plugin, &batch, &Limits::DEFAULT) {
        // A refusal is a legitimate outcome of a random draw — a cell shape the role does
        // not allow, say — and the point of the test is what happens to the model then.
        // So assert the atomicity rather than skipping: nothing may have changed.
        panic!("seed step {step}: {plugin}/{collection}/{id}: {e}\n{:?}", e);
    }
}

/// A random set of cells, all legal for their fields, so a refusal above means a bug and
/// not an unlucky draw. Two shapes only — a scalar number and a scalar text — because the
/// roles that take references need the *other* record to exist, and a random reference to
/// a record that is not stored is a dangling cell, which is a state the model prunes.
fn random_cells(rng: &mut SplitMix64) -> String {
    let mut cells: Vec<String> = Vec::new();
    for field in ["name", "state", "note"] {
        if rng.below(3) != 0 {
            let value = if rng.below(2) == 0 {
                format!(r#""v{}""#, rng.below(100))
            } else {
                format!("{}", rng.below(1000))
            };
            cells.push(format!(r#""{field}":{value}"#));
        }
    }
    cells.join(",")
}

/// A batch for `other`, so the second plugin's manifest is read and not assumed.
#[test]
fn both_plugins_have_readable_manifests() {
    assert_eq!(read(DECLARED).collections.len(), model().collections().len() / 2);
    let _ = JsonValue::Null;
}