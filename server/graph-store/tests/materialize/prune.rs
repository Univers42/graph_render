//! Pruning, asserted on the document itself as well as against the Model: the `keep-dangling`
//! and `keep-cells` breaks switch the rule off on both sides, so only these asserts see them.

use graph_contract::ingest::{Ingest, JsonValue, Record};

use super::*;

/// Both plugins registered, no records yet.
async fn registered(name: &str) -> Twin {
    let mut twin = Twin::new(name, 2).await;
    twin.register("tracker", TRACKER).await;
    twin.register("other", OTHER).await;
    twin
}

/// The record `(qcoll, id)` of `doc`.
fn record<'a>(doc: &'a Ingest, qcoll: &str, id: &str) -> &'a Record {
    doc.records
        .iter()
        .find(|r| r.collection == qcoll && r.id == id)
        .unwrap_or_else(|| panic!("{qcoll}/{id} is in the document"))
}

/// The document, checked against the Model and read back.
async fn read_checked(twin: &Twin) -> Ingest {
    let doc = twin.read().await;
    assert_eq!(doc, twin.model.to_json());
    ingest(&doc)
}

/// Every reference to a missing record is dropped; once the targets exist they come back.
#[tokio::test]
async fn unresolved_links_are_pruned_and_the_edge_returns_when_the_target_arrives() {
    let mut twin = registered("prune_returns").await;
    let task = batch_of(
        &[("task", "a", 1, r#""name":"A","blocks":["z"],"up":"z","link":"t9""#)],
        &[],
    );
    assert!(twin.apply("tracker", task).await);
    let doc = read_checked(&twin).await;
    let a = record(&doc, "tracker.task", "a");
    for field in ["blocks", "up", "link"] {
        assert_eq!(a.value(field), None, "dangling `{field}` is pruned");
    }
    assert_eq!(a.value("name"), Some(&JsonValue::Text("A".to_owned())));
    assert!(twin.apply("tracker", batch_of(&[("task", "z", 1, "")], &[])).await);
    assert!(twin.apply("other", batch_of(&[("thing", "t9", 1, "")], &[])).await);
    let doc = read_checked(&twin).await;
    let a = record(&doc, "tracker.task", "a");
    let z = JsonValue::Text("z".to_owned());
    assert_eq!(a.value("blocks"), Some(&JsonValue::List(vec![z.clone()])));
    assert_eq!(a.value("up"), Some(&z));
    assert_eq!(a.value("link"), Some(&JsonValue::Text("t9".to_owned())));
}

/// A link into a collection no plugin declares leaves the declaration, and so do its cells.
#[tokio::test]
async fn a_link_to_an_unregistered_collection_drops_the_field_and_its_cells() {
    let mut twin = Twin::new("prune_unregistered", 2).await;
    twin.register("tracker", TRACKER).await;
    let task = batch_of(&[("task", "a", 1, r#""name":"A","link":"t1""#)], &[]);
    assert!(twin.apply("tracker", task).await);
    let doc = read_checked(&twin).await;
    let task = doc.collection("tracker.task").expect("tracker.task");
    assert!(task.field("link").is_none(), "the field is not declared");
    assert!(task.field("blocks").is_some(), "a link inside the plugin is");
    assert_eq!(record(&doc, "tracker.task", "a").value("link"), None);
}

/// `[]` as sent is kept; a list whose every reference dangled is removed.
#[tokio::test]
async fn an_originally_empty_list_stays_and_a_list_emptied_by_pruning_is_removed() {
    let mut twin = registered("prune_empty_lists").await;
    let tasks = batch_of(
        &[
            ("task", "a", 1, r#""name":"A","blocks":[]"#),
            ("task", "b", 1, r#""name":"B","blocks":["gone","gone2"]"#),
        ],
        &[],
    );
    assert!(twin.apply("tracker", tasks).await);
    let doc = read_checked(&twin).await;
    let a = record(&doc, "tracker.task", "a");
    assert_eq!(a.value("blocks"), Some(&JsonValue::List(Vec::new())));
    assert_eq!(record(&doc, "tracker.task", "b").value("blocks"), None);
}

/// A record whose references all resolve is written from its stored bytes.
#[tokio::test]
async fn a_record_with_nothing_to_prune_is_copied_verbatim() {
    let twin = super::equal::seeded("prune_verbatim", 2).await;
    let doc = twin.read().await;
    let rows = twin
        .client
        .query("SELECT text FROM records WHERE ws = 'ws'", &[])
        .await
        .expect("read the stored texts");
    assert_eq!(rows.len(), twin.model.records().count());
    for row in rows {
        let text: String = row.get(0);
        assert!(doc.contains(&text), "verbatim: {text}");
    }
}

/// A parent naming a record that is not stored is dropped; one that is stored stays.
#[tokio::test]
async fn a_dangling_parent_is_dropped() {
    let mut twin = registered("prune_parent").await;
    let tasks = batch_of(
        &[
            ("task", "a", 1, r#""name":"A","up":"x""#),
            ("task", "b", 1, r#""name":"B","up":"a""#),
        ],
        &[],
    );
    assert!(twin.apply("tracker", tasks).await);
    let doc = read_checked(&twin).await;
    assert_eq!(record(&doc, "tracker.task", "a").value("up"), None);
    let b = record(&doc, "tracker.task", "b");
    assert_eq!(b.value("up"), Some(&JsonValue::Text("a".to_owned())));
}
