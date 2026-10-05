//! §5.2's motor-status map, one case per row, against a scripted stub.
//!
//! Every case here drives `/layout` end to end and reads the hub's own answer: a status, an `error`
//! string, and the headers §5.2's row says it carries. The subject is the **table**, so the motor is
//! a stub rather than graph-server: the point is which row a given motor answer takes, not that the
//! real motor produces those answers.
//!
//! Three cases are about something other than a status: the relayed body keeping the motor's own
//! `error` string, the motor that never answers (§5.2's `GRAPH_HUB_MOTOR_TIMEOUT_MS` row), and the
//! wait that ran out before the motor was called at all — the last two are in [`waits`], the
//! thirteen table rows in [`rows`], and the fixtures every case shares are here.
//!
//! Caveat: a stub is a script, not a motor. It cannot fail the way graph-server fails — it never
//! resets a connection mid-upload and never queues — so what these cases prove is the mapping, and
//! row `hub-roundtrip` is what proves the bytes.
#![cfg(feature = "db-tests")]

#[path = "support/mod.rs"]
mod support;

#[path = "motor_map/rows.rs"]
mod rows;
#[path = "motor_map/waits.rs"]
mod waits;

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
        &format!("/v1/workspaces/relay-mapped/layout?layout={LAYOUT}"),
        Body::empty(),
    )
    .await
}

/// A workspace with `count` records, so `/layout` has a document to stream.
async fn loaded(hub: &Hub, count: usize) {
    ready(hub, "relay-mapped", "task").await;
    let upserts: Vec<(&str, &str, &str)> = (0..count)
        .map(|i| ("task", leaked(&format!("id{i:04}")), "note"))
        .collect();
    let reply = hub
        .post(
            "/v1/workspaces/relay-mapped/plugins/task/batches",
            batch(&upserts, &[]),
        )
        .await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
}

/// A record id leaked once, because `batch` takes borrowed `&str`s and these are built per case.
fn leaked(id: &str) -> &'static str {
    Box::leak(id.to_owned().into_boxed_str())
}
