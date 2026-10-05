//! §5.2's motor-status map, one case per row, against a scripted stub.
//!
//! Every case here drives `/layout` end to end and reads the hub's own answer: a status, an `error`
//! string, and the headers §5.2's row says it carries. The subject is the **table**, so the motor is
//! a stub rather than graph-server: the point is which row a given motor answer takes, not that the
//! real motor produces those answers.
//!
//! Three cases are about something other than a status: the motor that never answers (§5.2's
//! `GRAPH_HUB_MOTOR_TIMEOUT_MS` row), the wait that ran out before the motor was called at all, and
//! the relayed body keeping the motor's own `error` string.
//!
//! Caveat: a stub is a script, not a motor. It cannot fail the way graph-server fails — it never
//! resets a connection mid-upload and never queues — so what these cases prove is the mapping, and
//! row `hub-roundtrip` is what proves the bytes.
#![cfg(feature = "db-tests")]

#[path = "support/mod.rs"]
mod support;

use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::body::Body;
use support::fixtures::*;
use support::*;

/// The layout every case asks for; none of them reaches the motor's own reader, because the stub
/// answers whatever it was scripted with.
const LAYOUT: &str = "layout.grid";

/// The motor key a stub fixture's hub presents: the stub checks no credential, so this is only the
/// shape the relay's own reader insists on, one non-empty line.
const STUB_KEY: &str = "a-key-the-stub-does-not-check";

/// A hub over `motor` and `env`: the store on `GM_HUB_PG_URL`, one workspace with `COUNT` records,
/// and the motor the case built.
async fn hub_over(motor: &Motor, env: &[(&str, &str)]) -> Hub {
    db::migrated().await;
    let url = db::url();
    let dir = scratch();
    let key_file = write_private(&dir.join("motor-key"), &format!("{STUB_KEY}\n"));
    let motor_url = motor.url("");
    let key_path = key_file.display().to_string();
    let mut all: Vec<(&str, &str)> = vec![
        ("GRAPH_HUB_MOTOR_URL", motor_url.as_str()),
        ("GRAPH_HUB_MOTOR_KEY_FILE", key_path.as_str()),
        ("GRAPH_HUB_DB_URL", url.as_str()),
    ];
    all.extend_from_slice(env);
    let hub = hub_with_env(&all);
    loaded(&hub, COUNT).await;
    hub
}

/// A hub over a stub scripted with `answers`.
///
/// The stub is **returned with the hub**: dropping a `Motor` stops its listener, so a fixture that
/// kept it in a local would leave the hub pointing at a port nothing answers, and every row of
/// §5.2's table would read as `MotorUnavailable`.
async fn hub_with_script(answers: Vec<StubReply>, env: &[(&str, &str)]) -> (Motor, Hub) {
    let (motor, _served, _key) = stub_parts(answers, Duration::ZERO).await;
    let hub = hub_over(&motor, env).await;
    (motor, hub)
}

/// How many records the workspace holds. Any positive number works; three is the smallest that still
/// makes the document more than a head and a tail.
const COUNT: usize = 3;

/// `POST /layout?layout=…`, the one request every case below makes.
async fn relay(hub: &Hub) -> Reply {
    hub.post(
        &format!("/v1/workspaces/mapped/layout?layout={LAYOUT}"),
        Body::empty(),
    )
    .await
}

/// A workspace with `count` records, so `/layout` has a document to stream.
async fn loaded(hub: &Hub, count: usize) {
    ready(hub, "mapped", "task").await;
    let upserts: Vec<(&str, &str, &str)> = (0..count)
        .map(|i| ("task", leaked(&format!("id{i:04}")), "note"))
        .collect();
    let reply = hub
        .post(
            "/v1/workspaces/mapped/plugins/task/batches",
            batch(&upserts, &[]),
        )
        .await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
}

/// A record id leaked once, because `batch` takes borrowed `&str`s and these are built per case.
fn leaked(id: &str) -> &'static str {
    Box::leak(id.to_owned().into_boxed_str())
}

/// The 400 row: the caller's own `layout`/`post` was refused, so the status and the `error` string
/// are graph-server's.
#[tokio::test]
async fn motor_400_is_relayed_as_400() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(400, "BadRequest", "`layout=<id>` is required")],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 400, "{}", reply.body());
    assert_eq!(reply.error(), "BadRequest");
    assert_eq!(reply.message(), "`layout=<id>` is required");
}

/// The 401 row: the hub's **own** motor key was refused, which is a hub defect, so it is a 502 with
/// §5.2's own name and a line in the log.
#[tokio::test]
async fn motor_401_is_502_motor_auth_and_logged() {
    let (_motor, hub) = hub_with_script(vec![StubReply::new(401, "Unauthorized", "no such key")], &[]).await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 502, "{}", reply.body());
    assert_eq!(reply.error(), "MotorAuth");
    assert!(
        logged(&hub, "MotorAuth"),
        "the refusal is in the log: {:?}",
        hub.lines()
    );
}

/// The 406 row: the caller's own `Accept` was refused, so the answer relays.
#[tokio::test]
async fn motor_406_is_relayed_as_406() {
    let (_motor, hub) = hub_with_script(vec![StubReply::new(406, "NotAcceptable", "no such face")], &[]).await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 406, "{}", reply.body());
    assert_eq!(reply.error(), "NotAcceptable");
    assert_eq!(reply.message(), "no such face");
}

/// The 408 row: the upload missed graph-server's own body timeout, so it is a 502 and carries **no**
/// `Retry-After`, because the same request would miss it again.
#[tokio::test]
async fn motor_408_is_502_motor_body_timeout_with_no_retry_after() {
    let (_motor, hub) = hub_with_script(vec![StubReply::new(408, "Timeout", "the body did not arrive")], &[]).await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 502, "{}", reply.body());
    assert_eq!(reply.error(), "MotorBodyTimeout");
    assert_eq!(reply.header("retry-after"), "", "no Retry-After on a 408");
}

/// The 413 row: the motor's own message is kept, so the caller learns which cap it passed.
#[tokio::test]
async fn motor_413_is_413_graph_too_large_with_the_motors_message() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(
            413,
            "IngestTooLarge",
            "the document is past the motor's limits",
        )],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 413, "{}", reply.body());
    assert_eq!(reply.error(), "GraphTooLarge");
    assert_eq!(
        reply.message(),
        "the document is past the motor's limits",
        "the motor's own message is kept"
    );
}

/// The first half of N3's 422 split: the document did not read at the motor, which `hub-materialize`
/// proves cannot happen, so it is a hub defect and a 502.
#[tokio::test]
async fn motor_422_ingest_invalid_is_502_materialize_invalid() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(422, "IngestInvalid", "the document is not valid")],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 502, "{}", reply.body());
    assert_eq!(reply.error(), "MaterializeInvalid");
}

/// The second half of the same split, and the other 502 of N3.
#[tokio::test]
async fn motor_422_contract_invalid_is_502_materialize_invalid() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(422, "ContractInvalid", "the document is not a contract")],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 502, "{}", reply.body());
    assert_eq!(reply.error(), "MaterializeInvalid");
}

/// The relayed half of N3's split: the caller's layout failed, which is a 422 relayed whole.
///
/// This and `motor_422_post_failed_is_relayed_as_422` are the two cases row
/// `negctl-layoutfailed-as-502` turns red: its break maps this arm to a 502 instead.
#[tokio::test]
async fn motor_422_layout_failed_is_relayed_as_422() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(422, "LayoutFailed", "the layout refused this graph")],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 422, "{}", reply.body());
    assert_eq!(reply.error(), "LayoutFailed");
    assert_eq!(reply.message(), "the layout refused this graph");
}

/// The other relayed half of the 422 split: a POST pass refused, which is the caller's geometry.
#[tokio::test]
async fn motor_422_post_failed_is_relayed_as_422() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(422, "PostFailed", "the POST pass refused this geometry")],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 422, "{}", reply.body());
    assert_eq!(reply.error(), "PostFailed");
    assert_eq!(reply.message(), "the POST pass refused this geometry");
}

/// The 429 row: the motor's queue was full, so the caller waits in the hub's queue and gets a 503
/// **with** a `Retry-After`, the opposite of the 503 row below.
#[tokio::test]
async fn motor_429_is_503_with_retry_after() {
    let (_motor, hub) = hub_with_script(vec![StubReply::new(429, "Busy", "the queue is full")], &[]).await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 503, "{}", reply.body());
    assert_eq!(reply.header("retry-after"), "1");
}

/// The 503 row: relayed whole, with graph-server's own `error` string and **no** `Retry-After`.
///
/// Caveat: §5.3 says the hub cannot tell an admission wait from an overrun here, so it relays the 503
/// with no `Retry-After` rather than guessing; a distinct overrun code is graph-render-4f's call.
#[tokio::test]
async fn motor_503_is_relayed_as_503_with_no_retry_after() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(503, "Timeout", "the layout ran past its deadline")],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 503, "{}", reply.body());
    assert_eq!(reply.error(), "Timeout", "the motor's own error string");
    assert_eq!(reply.message(), "the layout ran past its deadline");
    assert_eq!(
        reply.header("retry-after"),
        "",
        "a relayed 503 must not invite a retry"
    );
}

/// The 500 row: a hub defect, so a 502 with §5.2's name and a line in the log.
#[tokio::test]
async fn motor_500_is_502_motor_error() {
    let (_motor, hub) = hub_with_script(vec![StubReply::new(500, "Internal", "the motor failed")], &[]).await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 502, "{}", reply.body());
    assert_eq!(reply.error(), "MotorError");
}

/// §5.2's default row: a status the table has no row for is a 502 `MotorError`, never a relayed
/// answer, and it is logged because it is a hub defect.
#[tokio::test]
async fn motor_404_is_502_motor_error_and_logged() {
    let (_motor, hub) = hub_with_script(vec![StubReply::new(404, "NotFound", "no such route")], &[]).await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 502, "{}", reply.body());
    assert_eq!(reply.error(), "MotorError");
    assert!(
        logged(&hub, "MotorError"),
        "the default row is logged: {:?}",
        hub.lines()
    );
}

/// §5.2's last row: a motor that never answers is a 502 `MotorUnavailable` once
/// `GRAPH_HUB_MOTOR_TIMEOUT_MS` is out.
///
/// The stub holds its answer for far longer than the hub's own budget, and the budget is set low here
/// because §6 refuses a motor timeout at or below 40 000 **at start** — and this hub is built and
/// never started.
#[tokio::test]
async fn a_motor_that_never_answers_is_502_motor_unavailable() {
    let (motor, _served, _key) = stub_parts(
        vec![StubReply::new(200, "", "")],
        Duration::from_secs(SILENCE_SECS),
    )
    .await;
    let hub = hub_over(&motor, &[("GRAPH_HUB_MOTOR_TIMEOUT_MS", "150")]).await;
    let reply = relay(&hub).await;
    assert_eq!(reply.code(), 502, "{}", reply.body());
    assert_eq!(reply.error(), "MotorUnavailable");
}

/// How long `a_motor_that_never_answers_is_502_motor_unavailable` holds the stub's answer, in seconds.
///
/// Caveat: a fixture delay and not a bound the hub reads; it only has to outlast the 150 ms motor
/// timeout this case sets, and the hub has already answered by the time it is over.
const SILENCE_SECS: u64 = 30;

/// A wait that ran out is a 503 **before** the motor is called, so the motor's own request count must
/// not move: the hub never reached a peer, and a caller that sees this has not spent the motor a
/// connection.
#[tokio::test]
async fn a_pool_wait_past_the_timeout_is_503_before_the_motor_is_called() {
    let (motor, served, _key) =
        stub_parts(vec![StubReply::new(200, "", "")], Duration::ZERO).await;
    let hub = hub_over(
        &motor,
        &[("GRAPH_HUB_LAYOUTS", "1"), ("GRAPH_HUB_TIMEOUT_MS", "150")],
    )
    .await;
    let holder = hub.spawn(
        hub.request("POST", "/v1/workspaces/mapped/layout?layout=layout.grid")
            .body(Body::empty())
            .expect("the relay request"),
    );
    // The holder has the only `LAYOUTS` permit; the second relay waits on it and runs out.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let refused = relay(&hub).await;
    assert_eq!(refused.code(), 503, "{}", refused.body());
    assert_eq!(refused.header("retry-after"), "1");
    assert_eq!(
        refused.error(),
        "Busy",
        "a wait that ran out is the hub's own Busy"
    );
    let answered = holder.await.expect("the holder's relay answer");
    assert_eq!(answered.code(), 200, "{}", answered.body());
    assert_eq!(
        served.load(Ordering::SeqCst),
        1,
        "the refused relay never reached the motor"
    );
}

/// §5.2's relayed rows keep graph-server's `error` string byte for byte, so a client parses the same
/// name whichever service answered it.
///
/// The body is compared as text and not field by field, because the claim is about the bytes: a hub
/// that re-derived the class would produce the same `error` here and a different one on a message it
/// chose to rewrite.
#[tokio::test]
async fn a_relayed_body_keeps_the_motors_error_string() {
    let (_motor, hub) = hub_with_script(
        vec![StubReply::new(400, "IndexOutOfRange", "no POST pass has this id")],
        &[],
    )
    .await;
    let reply = relay(&hub).await;
    assert_eq!(
        reply.body(),
        r#"{"error":"IndexOutOfRange","message":"no POST pass has this id"}"#,
        "the relayed body is graph-server's own"
    );
}

/// Is a `motor-fault` line naming `fault` in the log?
///
/// Caveat: it matches on the `event` and the `fault` member and not on the whole line, because the
/// line also carries the workspace and the status, which §5.2 says nothing about here.
fn logged(hub: &Hub, fault: &str) -> bool {
    hub.lines()
        .iter()
        .any(|line| line["event"] == "motor-fault" && line["fault"] == fault)
}

