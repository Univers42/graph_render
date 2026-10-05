//! The relay's shape: the snapshot closes before the answer, no whole document is held, a refusal
//! keeps the motor's error, and a 503 is not retried.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::body::Body;
use graph_hub::relay::body::Probe;

use super::*;

/// `Graph-Seq` on `/layout` is the position `/graph`'s `ETag` quotes, so a caller that read the
/// document at one cursor knows which state the motor laid out (H15).
#[tokio::test]
async fn the_graph_seq_header_equals_the_graph_etag() {
    let (motor, key) = real_with_key().await;
    let hub = hub_against(&motor, &key, &[]).await;
    single_plugin(&hub, "relay-same", 4).await;
    let read = graph(&hub, "relay-same").await;
    assert_eq!(read.code(), 200, "{}", read.body());
    let laid_out = lay_out(&hub, "relay-same").await;
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
    let (motor, served, key) = stub_parts(
        vec![StubReply::new(200, "", "")],
        Duration::from_millis(HOLD_MS),
    )
    .await;
    let hub = hub_alone(&motor, &key, "relay-closed").await;
    single_plugin(&hub, "relay-closed", 8).await;
    let relay = hub.spawn(
        hub.request(
            "POST",
            "/v1/workspaces/relay-closed/layout?layout=layout.grid",
        )
        .body(Body::empty())
        .expect("the relay request"),
    );
    // The window opens when the stub has drained the upload. Before that the relay is still opening
    // its connection, and the restore detector `Store::client` runs there is a transaction of its own.
    while served.load(Ordering::SeqCst) == 0 && !relay.is_finished() {
        tokio::time::sleep(Duration::from_millis(POLL_MS)).await;
    }
    let mut polls = 0;
    while !relay.is_finished() {
        assert_eq!(
            open_snapshots(&hub).await,
            Vec::<String>::new(),
            "the relay held a snapshot while the motor's answer was awaited"
        );
        polls += 1;
        tokio::time::sleep(Duration::from_millis(POLL_MS)).await;
    }
    assert!(
        polls > 0,
        "the motor answered before the relay could be observed"
    );
    let reply = relay.await.expect("the relay answer");
    assert_eq!(reply.code(), 200, "{}", reply.body());
    assert_eq!(
        open_snapshots(&hub).await,
        Vec::<String>::new(),
        "no snapshot outlives the relay"
    );
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

/// Every backend of this database sitting in an open transaction, with its last statement.
///
/// An open materialization snapshot is one of these: `materialize::open` runs
/// `BEGIN ISOLATION LEVEL REPEATABLE READ, READ ONLY` and holds it until the walk is done, so
/// `idle in transaction` on this database is the fact §5.3 says must not outlive the upload.
///
/// Nothing is excluded by statement text: the case only asks once the stub has drained the upload,
/// when no connection of the relay's is still opening. The restore detector `Store::client` runs on
/// the connection this function opens is over before the question is sent on that same connection.
/// The old exclusion of the detector's `pg_control_system()` read broke when the detector gained a
/// `pg_current_wal_flush_lsn()` read, which is what the timing now rules out instead.
///
/// Caveat: this counts by database, so the case must own the database (see [`hub_alone`]). On the
/// shared `hub` database another fixture's snapshot would be named here and the case would fail for
/// a reason that is not about the relay.
async fn open_snapshots(hub: &Hub) -> Vec<String> {
    let store = hub.app.store().await.expect("the store under test");
    let client = store.client().await.expect("a connection");
    let rows = client
        .query(
            "SELECT pid, state, left(query, 60) AS q FROM pg_stat_activity \
             WHERE datname = current_database() AND state = 'idle in transaction'",
            &[],
        )
        .await
        .expect("the open-snapshot count");
    let mut out = Vec::new();
    for row in &rows {
        out.push(format!(
            "{} {:?} {:?}",
            row.get::<_, i32>(0),
            row.get::<_, String>(1),
            row.get::<_, String>(2)
        ));
    }
    out
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
    single_plugin(&hub, "relay-streamed", 40).await;
    let store = hub.app.store().await.expect("the store under test");
    let document = graph_store::materialize::open(store, "relay-streamed")
        .await
        .expect("a snapshot of the workspace");
    let probe = Arc::new(Probe::default());
    let request = graph_hub::relay::RelayReq {
        cursor: document.cursor(),
        ws: String::from("relay-streamed"),
        layout: Some(String::from(LAYOUT)),
        post: None,
        accept: None,
    };
    let answer = graph_hub::relay::post(&hub.app, &request, document, Some(Arc::clone(&probe)))
        .await
        .expect("the relay answer");
    assert_eq!(answer.status, 200);
    let counts = probe.counts();
    assert!(
        counts.chunks > 1,
        "one chunk is a whole document: {counts:?}"
    );
    assert_eq!(
        counts.held, counts.largest,
        "the walk held more than one chunk at once: {counts:?}"
    );
    assert_eq!(counts.dropped, 0, "no break is on: {counts:?}");
    let whole = graph(&hub, "relay-streamed").await.body().len() as u64;
    assert!(
        whole > counts.largest,
        "the document is not bigger than one chunk: {counts:?} of {whole}"
    );
}

/// §5.2's relayed row for `LayoutFailed`: the caller sees the motor's own status, `error` and
/// message, because the failure is about the caller's graph and not about a hub defect.
#[tokio::test]
async fn a_layout_failure_is_relayed_with_the_motors_error() {
    let motor = stub(vec![StubReply::new(
        422,
        "LayoutFailed",
        "the layout refused",
    )])
    .await;
    let hub = hub_against(&motor, STUB_KEY, &[]).await;
    single_plugin(&hub, "relay-refused", 4).await;
    let reply = lay_out(&hub, "relay-refused").await;
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
    single_plugin(&hub, "relay-once", 4).await;
    let reply = lay_out(&hub, "relay-once").await;
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
