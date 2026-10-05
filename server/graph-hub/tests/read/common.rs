//! The three fixtures every read case starts from, and nothing else.
//!
//! They live here rather than in `read.rs` because each is used by more than one of the three
//! files: a cursor needs the epoch, the epoch comes from the listing, and every case needs a
//! workspace with records in it.

use axum::body::Body;

use crate::support::fixtures::{MANIFEST, batch, ready};
use crate::support::{Hub, Reply};

/// The workspace's own epoch, as `GET /v1/workspaces` publishes it.
///
/// WHY the listing and not `/graph`: an epoch is drawn by the database's clock (`hub_next_epoch`,
/// microseconds), so a fixture cannot name one and must read it. The listing answers from the
/// workspace row alone, where `/graph` reads the change log — which matters for
/// `changes_returns_410_for_a_cursor_below_what_is_kept`, which has just emptied that log.
pub async fn epoch_of(hub: &Hub, ws: &str) -> String {
    let reply = hub.get_with("/v1/workspaces").await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON list");
    let row = value["workspaces"]
        .as_array()
        .expect("a workspaces array")
        .iter()
        .find(|row| row["id"] == ws)
        .unwrap_or_else(|| panic!("the listing holds {ws}"));
    row["epoch"].as_u64().expect("an epoch").to_string()
}

/// A workspace with `count` records, which is the fixture every read case starts from.
pub async fn loaded(hub: &Hub, ws: &str, plugin: &str, count: usize) {
    ready(hub, ws, plugin).await;
    let upserts: Vec<(&str, &str, &str)> = (0..count)
        .map(|i| {
            (
                "task",
                Box::leak(format!("id{i:04}").into_boxed_str()) as &str,
                "note",
            )
        })
        .collect();
    let reply = hub
        .post(
            &format!("/v1/workspaces/{ws}/plugins/{plugin}/batches"),
            batch(&upserts, &[]),
        )
        .await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
}

/// The workspace ids of a `GET /v1/workspaces` answer, in the order the route returned them, with
/// each row's `epoch` and `head_seq` checked on the way past.
pub fn names_of(reply: &Reply) -> Vec<String> {
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON list");
    let rows: Vec<serde_json::Value> = value["workspaces"]
        .as_array()
        .expect("a workspaces array")
        .clone();
    for row in &rows {
        assert!(
            row["epoch"].as_u64().is_some(),
            "each row carries its epoch"
        );
        assert!(
            row["head_seq"].as_u64().is_some(),
            "each row carries its head_seq"
        );
    }
    rows.iter()
        .map(|row| row["id"].as_str().expect("an id").to_owned())
        .collect()
}

/// [`ready`] with `key`, which is what a case with two identities needs: the second workspace is
/// created by the key that is granted it, because authorization runs before existence.
pub async fn ready_as(hub: &Hub, key: &str, ws: &str, plugin: &str) {
    for (path, body) in [
        (format!("/v1/workspaces/{ws}"), String::new()),
        (
            format!("/v1/workspaces/{ws}/plugins/{plugin}"),
            MANIFEST.to_owned(),
        ),
    ] {
        let reply = hub
            .send(
                hub.request_as(key, "PUT", &path)
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .expect("the request"),
            )
            .await;
        assert!(reply.code() < 300, "{path}: {}", reply.body());
    }
}
