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
//! and a 503 is not retried.
//!
//! Caveat: the real motor is served in process on an ephemeral loopback port
//! (`support::motor::real_with_key`), so a `/layout` measured here is a floor for the relay's cost
//! and not the production figure — `docs/measurements/hub-memory.md` measures the upload across two
//! containers.
#![cfg(feature = "db-tests")]

#[path = "support/mod.rs"]
mod support;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::body::Body;
use graph_hub::relay::body::Probe;
use support::fixtures::*;
use support::*;

/// The layout every relay below asks the motor for. `layout.grid` is the first registry entry, and
/// without an `Accept` the motor answers its binary face, so both arms compare the same bytes.
const LAYOUT: &str = "layout.grid";

/// The motor key the stub fixtures use. A stub checks no credential, so this is only the shape the
/// relay's own reader insists on: one non-empty line.
const STUB_KEY: &str = "a-key-the-stub-does-not-check";

/// The three fixtures of §8's `hub-roundtrip`, in the order the loop walks them.
const FIXTURES: [&str; 3] = ["plain", "linked", "pruned"];

/// A hub whose store reads `GM_HUB_PG_URL`, whose motor is `motor`, and whose
/// `GRAPH_HUB_MOTOR_KEY_FILE` carries `key`: the whole wiring `/layout` reads.
async fn hub_against(motor: &Motor, key: &str, env: &[(&str, &str)]) -> Hub {
    db::migrated().await;
    let url = db::url();
    let dir = scratch();
    let key_file = write_private(&dir.join("motor-key"), &format!("{key}\n"));
    let motor_url = motor.url("");
    let key_path = key_file.display().to_string();
    let mut all: Vec<(&str, &str)> = vec![
        ("GRAPH_HUB_MOTOR_URL", motor_url.as_str()),
        ("GRAPH_HUB_MOTOR_KEY_FILE", key_path.as_str()),
        ("GRAPH_HUB_DB_URL", url.as_str()),
    ];
    all.extend_from_slice(env);
    hub_with_env(&all)
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
///
/// Caveat: this case needs `/graph` to be wired, which is Task 6's router entry. On a branch where
/// `/graph` still answers 501 the case is **not** skipped and not passed: it fails, because a
/// roundtrip measured against a document this test did not read proves nothing.
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
    assert_eq!(reply.code(), 200, "{}", reply.body());
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

/// §8's `hub-roundtrip`: over each of its three fixtures, the hub's answer is byte-identical to what
/// graph-server answers for `/graph`'s own document at the same cursor.
#[tokio::test]
async fn layout_bytes_equal_motor_bytes_at_the_same_cursor() {
    let (motor, key) = real_with_key().await;
    let hub = hub_against(&motor, &key, &[]).await;
    for ws in FIXTURES {
        fixture(&hub, ws).await;
        let hub_side = lay_out(&hub, ws).await;
        assert_eq!(hub_side.code(), 200, "{ws}: {}", hub_side.body());
        let read = graph(&hub, ws).await;
        assert_eq!(read.code(), 200, "{ws}: {}", read.body());
        let motor_side = at_the_motor(&motor, &key, &read.body()).await;
        assert_eq!(motor_side.code(), 200, "{ws}: {}", motor_side.body());
        assert_eq!(
            hub_side.body.len(),
            motor_side.body.len(),
            "{ws}: the two answers are different lengths"
        );
        assert!(
            hub_side.body == motor_side.body,
            "{ws}: the hub's bytes are not graph-server's bytes"
        );
    }
}

/// One fixture of §8's three: `plain` is a single plugin with no links, `linked` adds a reference
/// that resolves across plugins, and `pruned` names a record nobody stored.
async fn fixture(hub: &Hub, ws: &str) {
    match ws {
        "plain" => single_plugin(hub, ws, 6).await,
        _ => {
            single_plugin(hub, ws, 4).await;
            second_plugin(hub, ws).await;
            memo(hub, ws, if ws == "linked" { "id0000" } else { "nowhere" }).await;
        }
    }
}

/// `POST /v1/layout?layout=…&source=contract` at graph-server itself, with `doc` as the body.
///
/// `source=contract` is the same one the relay sends: H1 says the hub asks for the contract layout
/// because the document it streams *is* the ingest contract document.
async fn at_the_motor(motor: &Motor, key: &str, doc: &str) -> Reply {
    let uri = format!("{}/v1/layout?layout={LAYOUT}&source=contract", motor.url(""));
    use hyper_util::client::legacy::Client;
    use hyper_util::client::legacy::connect::HttpConnector;
    use hyper_util::rt::TokioExecutor;
    let client: Client<HttpConnector, Body> = Client::builder(TokioExecutor::new()).build_http();
    let request = hyper::Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {key}"))
        .body(Body::from(doc.to_owned()))
        .expect("the motor request");
    let response = client.request(request).await.expect("a motor answer");
    let (parts, body) = response.into_parts();
    Reply::from_parts(parts, body).await
}

/// `Graph-Seq` on `/layout` is the position `/graph`'s `ETag` quotes, so a caller that read the
/// document at one cursor knows which state the motor laid out (H15).
#[tokio::test]
async fn the_graph_seq_header_equals_the_graph_etag() {
    let (motor, key) = real_with_key().await;
    let hub = hub_against(&motor, &key, &[]).await;
    single_plugin(&hub, "same", 4).await;
    let read = graph(&hub, "same").await;
    assert_eq!(read.code(), 200, "{}", read.body());
    let laid_out = lay_out(&hub, "same").await;
    assert_eq!(laid_out.code(), 200, "{}", laid_out.body());
    assert!(!read.header("etag").is_empty(), "/graph carries an ETag");
    assert_eq!(
        laid_out.header("graph-seq"),
        read.header("etag").trim_matches('"'),
        "Graph-Seq is the position the ETag quotes"
    );
}

/// §5.3: the snapshot spans the upload only. The store's transaction commits when the last chunk is
/// written, so a slow motor holds no snapshot, no xmin horizon and no pool connection.
///
/// The fact is the hub's own backends seen from outside, read **while** the motor still holds its
/// answer: `idle in transaction` is what an open snapshot looks like in `pg_stat_activity`, and there
/// must be none of this database's at any observation before the relay answers.
#[tokio::test]
async fn the_snapshot_closes_before_the_motor_answer_is_awaited() {
    let (motor, key) = stub_after(
        vec![StubReply::new(200, "", "")],
        Duration::from_millis(HOLD_MS),
    )
    .await;
    let hub = hub_against(&motor, &key, &[]).await;
    single_plugin(&hub, "closed", 8).await;
    let relay = hub.spawn(
        hub.request("POST", "/v1/workspaces/closed/layout?layout=layout.grid")
            .body(Body::empty())
            .expect("the relay request"),
    );
    let mut polls = 0;
    while !relay.is_finished() {
        assert_eq!(
            open_snapshots(&hub).await,
            0,
            "the relay held a snapshot while the motor's answer was awaited"
        );
        polls += 1;
        tokio::time::sleep(Duration::from_millis(POLL_MS)).await;
    }
    assert!(polls > 0, "the motor answered before the relay could be observed");
    let reply = relay.await.expect("the relay answer");
    assert_eq!(reply.code(), 200, "{}", reply.body());
    assert_eq!(open_snapshots(&hub).await, 0, "no snapshot outlives the relay");
}

/// How long the stub holds its answer in
/// [`the_snapshot_closes_before_the_motor_answer_is_awaited`], in milliseconds.
///
/// Caveat: a fixture delay and not a §6 number — it only has to outlast the upload and the polls, so
/// it is the smallest value that makes the window observable rather than a bound on any real motor.
const HOLD_MS: u64 = 750;

/// The gap between two snapshot observations, in milliseconds.
///
/// Caveat: a poll interval, so it is the resolution of the assertion and not a bound on anything:
/// the count is read directly from PostgreSQL and each read costs one round trip of its own.
const POLL_MS: u64 = 25;

/// How many of this database's backends are sitting in a transaction, which is what an open snapshot
/// looks like from outside the hub.
///
/// Caveat: it counts **every** backend of the test's database, so it is a statement about this process
/// rather than about one request. That is why the assertions require zero, which is the only direction
/// a database shared with other fixtures can be read in.
async fn open_snapshots(hub: &Hub) -> i64 {
    let store = hub.app.store().await.expect("the store under test");
    let client = store.client().await.expect("a connection");
    let row = client
        .query_one(
            "SELECT count(*) FROM pg_stat_activity \
             WHERE datname = current_database() AND state = 'idle in transaction'",
            &[],
        )
        .await
        .expect("the open-snapshot count");
    row.get::<_, i64>(0)
}

/// The relay's own instrumentation rather than RSS: the store reported more than one chunk while the
/// answer was being produced, and the walk never held more than one chunk at a time.
///
/// A hub that concatenated the document would report one chunk the size of the whole document and a
/// `held` peak equal to it; this walk yields each piece before it reads the next, so `held` can only
/// ever be `largest`.
#[tokio::test]
async fn layout_never_holds_a_whole_document() {
    let (motor, key) = real_with_key().await;
    let hub = hub_against(&motor, &key, &[]).await;
    single_plugin(&hub, "streamed", 40).await;
    let store = hub.app.store().await.expect("the store under test");
    let document = graph_store::materialize::open(store, "streamed")
        .await
        .expect("a snapshot of the workspace");
    let probe = Arc::new(Probe::default());
    let request = graph_hub::relay::RelayReq {
        cursor: document.cursor(),
        ws: String::from("streamed"),
        layout: Some(String::from(LAYOUT)),
        post: None,
        accept: None,
    };
    let answer = graph_hub::relay::post(&hub.app, &request, document, Some(Arc::clone(&probe)))
        .await
        .expect("the relay answer");
    assert_eq!(answer.status, 200);
    let counts = probe.counts();
    assert!(counts.chunks > 1, "one chunk is a whole document: {counts:?}");
    assert_eq!(
        counts.held, counts.largest,
        "the walk held more than one chunk at once: {counts:?}"
    );
    assert_eq!(counts.dropped, 0, "no break is on: {counts:?}");
    let whole = graph(&hub, "streamed").await.body().len() as u64;
    assert!(
        whole > counts.largest,
        "the document is not bigger than one chunk: {counts:?} of {whole}"
    );
}

/// §5.2's relayed row for `LayoutFailed`: the caller sees the motor's own status, `error` and
/// message, because the failure is about the caller's graph and not about a hub defect.
#[tokio::test]
async fn a_layout_failure_is_relayed_with_the_motors_error() {
    let motor = stub(vec![StubReply::new(422, "LayoutFailed", "the layout refused")]).await;
    let hub = hub_against(&motor, STUB_KEY, &[]).await;
    single_plugin(&hub, "refused", 4).await;
    let reply = lay_out(&hub, "refused").await;
    assert_eq!(reply.code(), 422, "{}", reply.body());
    assert_eq!(reply.error(), "LayoutFailed");
    assert_eq!(reply.message(), "the layout refused");
}

/// §5.3: the SDK never retries `/layout`, so a motor that answers 503 is relayed once and the hub
/// does not ask again. "Never retried" is a count, so the stub counts what it served.
#[tokio::test]
async fn a_layout_is_never_retried() {
    let (motor, served) =
        stub_counting(vec![StubReply::new(503, "Timeout", "the queue is full")]).await;
    let hub = hub_against(&motor, STUB_KEY, &[]).await;
    single_plugin(&hub, "once", 4).await;
    let reply = lay_out(&hub, "once").await;
    assert_eq!(reply.code(), 503, "{}", reply.body());
    assert_eq!(reply.error(), "Timeout", "the motor's own error string");
    assert_eq!(
        reply.header("retry-after"),
        "",
        "a relayed 503 carries no Retry-After"
    );
    assert_eq!(
        served.load(Ordering::SeqCst),
        1,
        "the hub asked the motor twice"
    );
}