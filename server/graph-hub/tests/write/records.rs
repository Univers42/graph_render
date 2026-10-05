//! The read-after-write routes: the records page and its cursor, one record, and the plugin
//! listing.
//!
//! The 403 case lives here too, because it is a claim about what the one-record route exposes.

use crate::support::add_key;
use crate::support::fixtures::{batch, hub_db, hub_db_grants, ready, upsert};

/// A records page is in byte order, carries `plugin_seq` as `<epoch>.<seq>` on every page, and stops
/// at `limit`.
#[tokio::test]
async fn a_record_page_is_in_byte_order_and_carries_plugin_seq() {
    let hub = hub_db(&[]).await;
    ready(&hub, "paged", "task").await;
    let path = "/v1/workspaces/paged/plugins/task/batches";
    hub.post(
        path,
        batch(
            &[("task", "b", "n"), ("task", "a", "n"), ("task", "c", "n")],
            &[],
        ),
    )
    .await;
    let reply = hub
        .get_with("/v1/workspaces/paged/plugins/task/records?limit=2")
        .await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON page");
    let ids: Vec<&str> = value["records"]
        .as_array()
        .expect("a records array")
        .iter()
        .map(|row| row["id"].as_str().expect("an id"))
        .collect();
    assert_eq!(ids, ["a", "b"], "byte order, and the page stops at limit=2");
    let plugin_seq = value["plugin_seq"].as_str().expect("a plugin_seq");
    assert_eq!(plugin_seq.split('.').count(), 2, "{plugin_seq:?}");
}

/// The opaque `next` walks the whole collection and then stops: the last page carries no `next`, so
/// a client never asks past the end.
#[tokio::test]
async fn a_records_page_next_cursor_terminates() {
    let hub = hub_db(&[]).await;
    ready(&hub, "walk", "task").await;
    let path = "/v1/workspaces/walk/plugins/task/batches";
    hub.post(
        path,
        batch(
            &[("task", "a", "n"), ("task", "b", "n"), ("task", "c", "n")],
            &[],
        ),
    )
    .await;
    let mut next = String::new();
    let mut seen: Vec<String> = Vec::new();
    for _ in 0..8 {
        let cursor = if next.is_empty() {
            String::new()
        } else {
            format!("&cursor={next}")
        };
        let reply = hub
            .get_with(&format!(
                "/v1/workspaces/walk/plugins/task/records?limit=1{cursor}"
            ))
            .await;
        assert_eq!(reply.code(), 200, "{}", reply.body());
        let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON page");
        for row in value["records"].as_array().expect("a records array") {
            seen.push(row["id"].as_str().expect("an id").to_owned());
        }
        match value["next"].as_str() {
            Some(cursor) => next = cursor.to_owned(),
            None => {
                next.clear();
                break;
            }
        }
    }
    assert_eq!(seen, ["a", "b", "c"], "every id exactly once, in order");
    assert!(next.is_empty(), "the last page carries no next cursor");
}

/// One record is 200 with its `rev` and its `values`, read from the store's own canonical text.
#[tokio::test]
async fn one_record_is_200_with_its_rev() {
    let hub = hub_db(&[]).await;
    ready(&hub, "one", "task").await;
    hub.post(
        "/v1/workspaces/one/plugins/task/batches",
        upsert("task", "solo", "the note"),
    )
    .await;
    let reply = hub
        .get_with("/v1/workspaces/one/records/task/task/solo")
        .await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON record");
    assert_eq!(value["id"], "solo");
    assert_eq!(value["collection"], "task");
    assert!(
        value["rev"].as_u64().expect("a rev") > 0,
        "the store's own rev"
    );
    assert_eq!(
        value["values"]["note"], "the note",
        "the record's own values"
    );
}

/// The one-record route of **another** plugin is a 403, not a 404: authorization runs before
/// existence, so a key with no grant learns nothing about whether the record is there. This is
/// Review Focus 1's second case on a different route.
#[tokio::test]
async fn one_record_of_another_plugin_is_403_not_404() {
    let hub = hub_db_grants("tester * admin\nstranger * write:other\n").await;
    let stranger = add_key(&hub, "stranger");
    // The keyring read both files when the hub was built, so a key minted after that is unknown
    // until a reload; the same `SIGHUP` path `reload.rs` covers, reached directly here.
    hub.app
        .keys
        .reload()
        .expect("the second key is in the pair");
    assert_eq!(hub.put("/v1/workspaces/guarded", "").await.code(), 201);
    ready(&hub, "guarded", "task").await;
    hub.post(
        "/v1/workspaces/guarded/plugins/task/batches",
        upsert("task", "solo", "n"),
    )
    .await;
    // The record exists, and the second key has no grant covering `task` on this workspace: the
    // refusal is the 403, never the 404 a lookup-then-refuse order would give.
    let refused = hub
        .get_as(&stranger, "/v1/workspaces/guarded/records/task/task/solo")
        .await;
    assert_eq!(refused.code(), 403, "{}", refused.body());
    let missing = hub
        .get_as(&stranger, "/v1/workspaces/guarded/records/task/task/absent")
        .await;
    assert_eq!(missing.code(), 403, "an absent record is the same bytes");
    assert_eq!(missing.body(), refused.body(), "byte for byte");
}

/// `GET .../plugins` answers every registered manifest, one per plugin, in plugin order.
#[tokio::test]
async fn get_plugins_returns_every_manifest() {
    let hub = hub_db(&[]).await;
    ready(&hub, "listed", "alpha").await;
    ready(&hub, "listed", "beta").await;
    let reply = hub.get_with("/v1/workspaces/listed/plugins").await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON list");
    let names: Vec<&str> = value["manifests"]
        .as_array()
        .expect("a manifests array")
        .iter()
        .map(|m| m["plugin"].as_str().expect("a plugin name"))
        .collect();
    assert_eq!(names, ["alpha", "beta"], "both plugins, in plugin order");
}
