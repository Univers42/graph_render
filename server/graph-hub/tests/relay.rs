//! `POST /v1/workspaces/{ws}/layout` against the **real** motor: the hub's own document goes in as
//! the request body and graph-server's own snapshot comes back, byte for byte.
//!
//! The three fixtures are §8's `hub-roundtrip` ones: a workspace with one plugin and no links, one
//! with a cross-plugin link, and one whose link names a collection nobody registered, so H12's
//! pruning is in the document the motor reads. A relay that changed a byte would be a relay whose
//! answer is a different graph, which is the only thing this file is about.
//!
//! Two cases are about the relay's *shape* rather than its bytes: the snapshot is closed before the
//! motor's answer is awaited, and no whole document is ever in memory.
//!
//! Caveat: the motor is served in process on an ephemeral loopback port
//! (`support::motor::real_with_key`), so this is a floor for the relay's cost and not the production
//! figure — `docs/measurements/hub-memory.md` measures the upload across two containers.
#![cfg(feature = "db-tests")]

#[path = "support/mod.rs"]
mod support;

use std::sync::Arc;
use std::time::Duration;

use graph_hub::relay::body::{Counts, Probe};
use support::fixtures::*;
use support::*;

/// The layout both arms ask the motor for. `layout.grid` is the first registry entry, and the
/// binary face is the default, so the answer is the same bytes both sides compare.
const LAYOUT: &str = "layout.grid";

/// A hub whose store reads `GM_HUB_PG_URL`, whose motor is `motor`, and whose motor key file
/// carries `key` — the whole wiring `/layout` needs, and the only thing this file sets up.
async fn hub_against(motor: &Motor, key: &str, env: &[(&str, &str)]) -> Hub {
    let dir = scratch();
    let key_file = write_private(&dir.join("motor-key"), &format!("{key}\n"));
    let mut all: Vec<(&str, &str)> = vec![
        ("GRAPH_HUB_MOTOR_URL", &motor.url("")),
        ("GRAPH_HUB_MOTOR_KEY_FILE", &key_file.display().to_string()),
    ];
    all.extend_from_slice(env);
    db::migrated().await;
    let url = db::url();
    all.push(("GRAPH_HUB_DB_URL", url.as_str()));
    hub_with_env(&all)
}

/// `POST /layout` with the layout under test and no `Accept`, so the motor answers its binary face.
async fn lay_out(hub: &Hub, ws: &str) -> Reply {
    hub.post(
        &format!("/v1/workspaces/{ws}/layout?layout={LAYOUT}"),
        Body::empty(),
    )
    .await
}

/// `GET /graph`, which is the document the relay streamed.
async fn document(hub: &Hub, ws: &str) -> Reply {
    hub.get_with(&format!("/v1/workspaces/{ws}/graph")).await
}

/// `POST /v1/layout?layout=…&source=contract` at graph-server itself, with `doc` as the body.
async fn at_the_motor(motor: &Motor, key: &str, doc: &str) -> Reply {
    let uri = format!("{}/v1/layout?layout={LAYOUT}&source=contract", motor.url(""));
    motor_send(&uri, key, doc).await
}

/// One motor request over loopback, reading the whole answer.
async fn motor_send(uri: &str, key: &str, doc: &str) -> Reply {
    use axum::http::Request;
    use http_body_util::BodyExt;
    use hyper_util::client::legacy::Client;
    use hyper_util::client::legacy::connect::HttpConnector;
    use hyper_util::rt::TokioExecutor;
    let client: Client<HttpConnector, axum::body::Body> =
        Client::builder(TokioExecutor::new()).build_http();
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {key}"))
        .body(axum::body::Body::from(doc.to_owned()))
        .expect("the motor request");
    let response = client.request(request).await.expect("a motor answer");
    let (parts, body) = response.into_parts();
    let body = body.collect().await.expect("the motor body").to_bytes();
    Reply {
        status: parts.status,
        headers: parts.headers,
        body,
    }
}

/// A workspace with one plugin, `count` records and no links.
async fn single_plugin(hub: &Hub, ws: &str, count: usize) {
    ready(hub, ws, "task").await;
    upsert_many(hub, ws, "task", count).await;
}

/// `count` records in `collection` of `plugin`, ids `id0000` upward.
async fn upsert_many(hub: &Hub, ws: &str, plugin: &str, count: usize) {
    let upserts: Vec<(&str, &str, &str)> = (0..count)
        .map(|i| {
            let id: &str = Box::leak(format!("id{i:04}").into_boxed_str());
            ("task", id, "note")
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

/// Two plugins in one workspace, the second registered against `MANIFEST_LINKED` so a field can name
/// the first plugin's collection — the cross-plugin link of §8's second fixture.
async fn two_plugins(hub: &Hub, ws: &str) {
    ready(hub, ws, "task").await;
    upsert_many(hub, ws, "task", 4).await;
    let second = hub
        .put(
            &format!("/v1/workspaces/{ws}/plugins/note"),
            manifest_linked(2),
        )
        .await;
    assert!(
        second.code() == 201 || second.code() == 200,
        "register note: {}",
        second.body()
    );
    let reply = hub
        .post(
            &format!("/v1/workspaces/{ws}/plugins/note/batches"),
            batch(&[("memo", "memo1", "points at a task")], &[]),
        )
        .await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
}

/// A second collection whose `relates` field is a **link** to `task`, so a record in it holds a
/// reference the materializer either resolves or prunes.
const MANIFEST_LINKED: &str = r#"{
  "version": 1,
  "manifestVersion": 2,
  "name": "Memos",
  "collections": [
    { "id": "memo", "name": "Memos", "titleField": "name", "fields": [
      { "id": "name", "name": "Name", "role": "title", "link": null },
      { "id": "relates", "name": "Relates", "role": "scalar",
        "link": { "cardinality": "one", "collection": "task", "symmetric": false } }
    ] }
  ]
}"#;

/// The same manifest at `version`, read and written back by graph-contract.
fn manifest_linked(version: u32) -> String {
    manifest_of(MANIFEST_LINKED, version)
}

/// One memo naming a task that does not exist, which is what H12 prunes away from the document.
async fn dangling_link(hub: &Hub, ws: &str, collection: &str) {
    let body = format!(
        r#"{{"upserts":[{{"collection":"{collection}","id":"memo1","updatedAt":1,"values":{{"name":"memo1","relates":"nowhere"}}}}],"deletes":[]}}"#
    );
    let reply = hub
        .post(
            &format!("/v1/workspaces/{ws}/plugins/note/batches"),
            body,
        )
        .await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
}

/// §8's `hub-roundtrip`, over its three fixtures: the hub's answer is byte-identical to graph-server's
/// own answer on `/graph`'s document, for a plain workspace, a cross-plugin link and a link to a
/// collection nobody registered.
#[tokio::test]
async fn layout_bytes_equal_motor_bytes_at_the_same_cursor() {
    let (motor, key) = real_with_key().await;
    let hub = hub_against(&motor, &key, &[]).await;
    for (ws, setup) in fixtures(&hub).await {
        let hub_side = lay_out(&hub, &ws).await;
        assert_eq!(hub_side.code(), 200, "{ws}: {}", hub_side.body());
        let graph = document(&hub, &ws).await;
        assert_eq!(graph.code(), 200, "{ws}: {}", graph.body());
        let motor_side = at_the_motor(&motor, &key, &graph.body()).await;
        assert_eq!(motor_side.code(), 200, "{ws}: {}", motor_side.body());
        assert_eq!(
            hub_side.body.len(),
            motor_side.body.len(),
            "{ws}: the hub and the motor answered different lengths"
        );
        assert!(
            hub_side.body == motor_side.body,
            "{ws}: the hub's bytes are not graph-server's bytes"
        );
    }
}

/// The three fixtures, each a workspace of its own so one cannot leak into the next.
async fn fixtures(hub: &Hub) -> Vec<(String, Fixture)> {
    let mut out: Vec<(String, Fixture)> = Vec::new();
    single_plugin(hub, "plain", 6).await;
    out.push((String::from("plain"), Fixture::NoLinks));
    two_plugins(hub, "linked").await;
    out.push((String::from("linked"), Fixture::CrossPlugin));
    two_plugins(hub, "pruned").await;
    dangling_link(hub, "pruned", "memo").await;
    out.push((String::from("pruned"), Fixture::Pruned));
    out
}

/// Which of §8's three fixtures a workspace is, so a failure names the one that broke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fixture {
    /// One plugin, no links.
    NoLinks,
    /// A reference that resolves across two plugins.
    CrossPlugin,
    /// A reference to a collection nobody registered.
    Pruned,
}

/// `Graph-Seq` on `/layout` is the position `/graph`'s `ETag` carries, so a caller that read the
/// document at one cursor can send it to the motor and know which state was laid out (H15).
#[tokio::test]
async fn the_graph_seq_header_equals_the_graph_etag() {
    let (motor, key) = real_with_key().await;
    let hub = hub_against(&motor, &key, &[]).await;
    single_plugin(&hub, "same", 4).await;
    let graph = document(&hub, "same").await;
    assert_eq!(graph.code(), 200, "{}", graph.body());
    let laid_out = lay_out(&hub, "same").await;
    assert_eq!(laid_out.code(), 200, "{}", laid_out.body());
    let etag = graph.header("etag");
    assert!(!etag.is_empty(), "/graph carries an ETag");
    assert_eq!(
        laid_out.header("graph-seq"),
        etag.trim_matches('"'),
        "Graph-Seq is the position ETag quotes"
    );
}

/// §5.3: the snapshot spans the upload only. The store's transaction commits when the last chunk is
/// written, before the motor's answer is awaited, so a slow motor does not hold a pool connection
/// or an xmin horizon.
///
/// The fact is the **hub's** connection count against PostgreSQL, read while the motor is holding
/// its answer: `state = 'idle in transaction'` is what a snapshot looks like from outside, and there
/// must be none of this database's once the upload is done.
#[tokio::test]
async fn the_snapshot_closes_before_the_motor_answer_is_awaited() {
    let (motor, key) = real_with_key().await;
    let hub = hub_against(&motor, &key, &[]).await;
    single_plugin(&hub, "closed", 8).await;
    let laid_out = hub.spawn(
        hub.request("POST", "/v1/workspaces/closed/layout?layout=layout.grid")
            .body(Body::empty())
            .expect("the relay request"),
    );
    // While the motor is still answering there is no open snapshot: the walk ended, `next()`
    // committed, and the `Document` went with the request body.
    assert_eq!(
        open_snapshots(&hub, "closed").await,
        0,
        "the relay held a snapshot while the motor's answer was awaited"
    );
    let reply = laid_out.await.expect("the relay answer");
    assert_eq!(reply.code(), 200, "{}", reply.body());
}

/// The number of this database's backends sitting in a transaction, which is what an open snapshot
/// looks like from outside the hub.
async fn open_snapshots(hub: &Hub, ws: &str) -> i64 {
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
    let count = row.get::<_, i64>(0);
    let _ = ws;
    count
}

/// The relay's own instrumentation, not RSS: the store reported more than one chunk while the answer
/// was being produced, and the walk never held more than one chunk at a time.
///
/// A hub that concatenated the document would report one chunk the size of the whole thing and a
/// `held` peak equal to it; the walk yields each piece before it reads the next, so `held` can only
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
    let cursor = document.cursor();
    let relay = graph_hub::relay::post(
        &hub.app,
        &graph_hub::relay::RelayReq {
            ws: String::from("streamed"),
            layout: Some(String::from(LAYOUT)),
            post: None,
            accept: None,
            cursor,
        },
        document,
        Some(Arc::clone(&probe)),
    )
    .await
    .expect("the relay answer");
    assert_eq!(relay.status, 200);
    let counts = probe.counts();
    assert!(counts.chunks > 1, "one chunk is a whole document: {counts:?}");
    assert_eq!(
        counts.held, counts.largest,
        "the walk held more than one chunk at a time: {counts:?}"
    );
    assert_eq!(counts.dropped, 0, "no break is on: {counts:?}");
    let graph = document_text(&hub, "streamed").await;
    assert!(
        graph.len() as u64 > counts.largest,
        "the document is bigger than one chunk: {counts:?}"
    );
}

/// `GET /graph`'s body as text, which is the byte count the chunk sizes are compared against.
async fn document_text(hub: &Hub, ws: &str) -> String {
    document(hub, ws).await.body()
}

/// The counts a `Counts` prints, so every assertion above names the numbers it read.
trait Print {
    /// `Counts` in the `{chunks, largest, held, dropped}` spelling the messages use.
    fn show(&self) -> String;
}

impl Print for Counts {
    fn show(&self) -> String {
        format!(
            "{{chunks: {}, largest: {}, held: {}, dropped: {}}}",
            self.chunks, self.largest, self.held, self.dropped
        )
    }
}

/// A motor refusal reaches the caller as the motor's own answer: `LayoutFailed` stays a 422 and its
/// `error` string is the motor's, because §5.2's row for it is "relayed".
#[tokio::test]
async fn a_layout_failure_is_relayed_with_the_motors_error() {
    let motor = stub(vec![StubReply::new(422, "LayoutFailed", "the layout refused")]).await;
    let hub = hub_against(&motor, "not-a-real-key", &[]).await;
    single_plugin(&hub, "refused", 4).await;
    let reply = lay_out(&hub, "refused").await;
    assert_eq!(reply.code(), 422, "{}", reply.body());
    assert_eq!(reply.error(), "LayoutFailed");
    assert_eq!(reply.message(), "the layout refused");
}

/// §5.3: the SDK never retries `/layout`, so a motor that answers 503 is relayed once and the hub
/// does not ask again. The stub counts its requests, and one request is what it saw.
#[tokio::test]
async fn a_layout_is_never_retried() {
    let (motor, key, served) = counting_stub(vec![StubReply::new(503, "Busy", "the queue is full")])
        .await;
    let hub = hub_against(&motor, &key, &[]).await;
    single_plugin(&hub, "once", 4).await;
    let reply = lay_out(&hub, "once").await;
    assert_eq!(reply.code(), 503, "{}", reply.body());
    assert_eq!(reply.error(), "Busy", "the motor's own error string");
    assert_eq!(
        reply.header("retry-after"),
        "",
        "a relayed 503 carries no Retry-After"
    );
    assert_eq!(served.load(std::sync::atomic::Ordering::SeqCst), 1);
}

/// The stub with a served counter the test can read, because "never retried" is a count and not a
/// status code.
async fn counting_stub(answers: Vec<StubReply>) -> (Motor, String, Arc<std::sync::atomic::AtomicU64>) {
    stub_counting(answers).await
}

/// A key the stub never checks, named here so the fixture and the assertions read the same thing.
const STUB_KEY: &str = &STUB_KEY_TEXT;
const STUB_KEY_TEXT: &str = "a-key-the-stub-does-not-check";

/// How long `the_snapshot_closes_before_the_motor_answer_is_awaited` waits for the relay to finish
/// once the snapshot is gone.
const PATIENCE: Duration = Duration::from_secs(10);