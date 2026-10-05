//! Document order and pruning: the two rules that decide *what a materialized hub writes*.
//!
//! A child module of `materialize.rs` and not a sibling, for one reason that has nothing
//! to do with the 300-line limit: the gate rows filter on `hub::tests::materialize`, so
//! every test about the model has to live under that name or the row would run a subset of
//! them and pass green while the rest rotted.
//!
//! It is a different question as well: `materialize.rs` asks how the model *transitions*,
//! this asks what the state it holds is *written* as. Between them they are the whole
//! model, and the property test in `property.rs` holds both to the document's own reader.

use super::super::fixtures::{bare, manifest, upsert};
use super::model;
use crate::hub::Limits;
use crate::hub::model::Model;

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
    assert_eq!(
        order, sorted,
        "the qualified strings must already be in byte order"
    );
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
    assert!(
        !text.contains(r#""up":"#),
        "the dangling parent is gone: {text}"
    );
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
    assert!(
        !record.contains(r#""blocks":"#),
        "the cell is gone: {record}"
    );
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
    model
        .apply("tracker", &bare("task", "r1", 1), &Limits::DEFAULT)
        .unwrap();
    model
        .apply(
            "tracker",
            &upsert(
                "task",
                "r2",
                1,
                r#""name":"W","blocks":["r1","r1"],"up":"r2""#,
            ),
            &Limits::DEFAULT,
        )
        .unwrap();
    let text = model.to_json();
    assert!(text.contains(r#""blocks":["r1","r1"]"#), "{text}");
    assert!(
        text.contains(r#""up":"r2""#),
        "a self parent is kept: {text}"
    );
}
