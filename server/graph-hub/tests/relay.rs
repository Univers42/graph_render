//! `POST /v1/workspaces/{ws}/layout`: the hub's own document goes in as the motor's request body and
//! the motor's own snapshot comes back, byte for byte.
//!
//! §8's `hub-roundtrip` runs over its three fixtures — a workspace with one plugin and no links, one
//! with a cross-plugin link, and one whose link names a record nobody stored, so H12's pruning is in
//! the document the motor reads. A relay that changed a byte would be a relay whose answer is a
//! different graph, which is the whole subject of this file.
//!
//! The other cases are the relay's *shape*: the snapshot closes before the motor's answer is
//! awaited, no whole document is ever in memory, a refusal is relayed with the motor's own error,
//! and a 503 is not retried. They are in [`shape`], the byte equality and the `drop-record`
//! asymmetry in [`roundtrip`], and the fixtures every case shares are here.
//!
//! Caveat: the real motor is served in process on an ephemeral loopback port
//! (`support::motor::real_with_key`), so a `/layout` measured here is a floor for the relay's cost
//! and not the production figure — `docs/measurements/hub-memory.md` measures the upload across two
//! containers.
#![cfg(feature = "db-tests")]

#[path = "support/mod.rs"]
mod support;

#[path = "relay/roundtrip.rs"]
mod roundtrip;
#[path = "relay/shape.rs"]
mod shape;

use axum::body::Body;
use support::fixtures::*;
use support::*;

/// The layout every relay below asks the motor for. `layout.grid` is the first registry entry, and
/// without an `Accept` the motor answers its binary face, so both arms compare the same bytes.
const LAYOUT: &str = "layout.grid";

/// The motor key the stub fixtures use. A stub checks no credential, so this is only the shape the
/// relay's own reader insists on: one non-empty line.
const STUB_KEY: &str = "a-key-the-stub-does-not-check";

/// The three fixtures of §8's `hub-roundtrip`, in the order the loop walks them.
const FIXTURES: [&str; 3] = ["relay-plain", "relay-linked", "relay-pruned"];

/// A hub whose store reads `GM_HUB_PG_URL`, whose motor is `motor`, and whose
/// `GRAPH_HUB_MOTOR_KEY_FILE` carries `key`: the whole wiring `/layout` reads.
async fn hub_against(motor: &Motor, key: &str, env: &[(&str, &str)]) -> Hub {
    db::migrated().await;
    hub_over(&db::url(), motor, key, env).await
}

/// [`hub_against`] over a named database rather than the shared `hub` one.
async fn hub_over(url: &str, motor: &Motor, key: &str, env: &[(&str, &str)]) -> Hub {
    let dir = scratch();
    let key_file = write_private(&dir.join("motor-key"), &format!("{key}\n"));
    let motor_url = motor.url("");
    let key_path = key_file.display().to_string();
    let mut all: Vec<(&str, &str)> = vec![
        ("GRAPH_HUB_MOTOR_URL", motor_url.as_str()),
        ("GRAPH_HUB_MOTOR_KEY_FILE", key_path.as_str()),
        ("GRAPH_HUB_DB_URL", url),
    ];
    all.extend_from_slice(env);
    hub_with_env(&all)
}

/// A hub over a database of this case's own, which is what lets the snapshot count below be a
/// statement about one request.
///
/// Caveat: a database per case rather than the shared `hub` one, because `pg_stat_activity` counts
/// every backend of whichever database it reads and the cases of this binary run concurrently. On
/// the shared database the count would name another case's snapshot and this case would fail for a
/// reason that is not about the relay.
async fn hub_alone(motor: &Motor, key: &str, ws: &str) -> Hub {
    let url = db::fresh_migrated(ws).await;
    hub_over(&url, motor, key, &[]).await
}

/// `POST /layout?layout=…` with no `Accept`, so the motor answers its binary face.
async fn lay_out(hub: &Hub, ws: &str) -> Reply {
    hub.post(
        &format!("/v1/workspaces/{ws}/layout?layout={LAYOUT}"),
        Body::empty(),
    )
    .await
}

/// `GET /graph`, the document the relay streamed.
async fn graph(hub: &Hub, ws: &str) -> Reply {
    hub.get_with(&format!("/v1/workspaces/{ws}/graph")).await
}

/// A workspace with one plugin, `count` records and no links.
async fn single_plugin(hub: &Hub, ws: &str, count: usize) {
    ready(hub, ws, "task").await;
    let upserts: Vec<(&str, &str, &str)> = (0..count)
        .map(|i| ("task", leaked(&format!("id{i:04}")), "note"))
        .collect();
    let reply = hub
        .post(
            &format!("/v1/workspaces/{ws}/plugins/task/batches"),
            batch(&upserts, &[]),
        )
        .await;
    assert_eq!(reply.code(), 200, "{ws}: {}", reply.body());
}

/// A second plugin whose `relates` field is a **link** naming the first plugin's `task` collection,
/// which is what makes a resolved cross-plugin reference and a reference to nothing two documents.
async fn second_plugin(hub: &Hub, ws: &str) {
    let registered = hub
        .put(
            &format!("/v1/workspaces/{ws}/plugins/note"),
            manifest_of(MANIFEST_LINKED, 1),
        )
        .await;
    assert!(
        registered.code() == 201 || registered.code() == 200,
        "register note: {}",
        registered.body()
    );
}

/// One memo in the linked plugin. `target` is what `relates` names, so `"nowhere"` is the reference
/// H12 prunes out of the document.
async fn memo(hub: &Hub, ws: &str, target: &str) {
    let body = format!(
        r#"{{"upserts":[{{"collection":"memo","id":"memo1","updatedAt":1,"values":{{"name":"memo1","relates":"{target}"}}}}],"deletes":[]}}"#
    );
    let reply = hub
        .post(&format!("/v1/workspaces/{ws}/plugins/note/batches"), body)
        .await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
}

/// The linked plugin's manifest: a `relates` field whose `link` names another plugin's collection.
const MANIFEST_LINKED: &str = r#"{
  "version": 1,
  "manifestVersion": 1,
  "name": "Memos",
  "collections": [
    { "id": "memo", "name": "Memos", "titleField": "name", "fields": [
      { "id": "name", "name": "Name", "role": "title", "link": null },
      { "id": "relates", "name": "Relates", "role": "link",
        "link": { "cardinality": "one", "collection": "task.task", "symmetric": false } }
    ] }
  ]
}"#;

/// A record id leaked once, because `batch` takes borrowed `&str`s and the ids below are built.
fn leaked(id: &str) -> &'static str {
    Box::leak(id.to_owned().into_boxed_str())
}
