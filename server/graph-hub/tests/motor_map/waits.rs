//! The two rows that are about time rather than about a status: a motor that never answers, and a
//! wait that ran out before the motor was called at all.

use std::sync::atomic::Ordering;
use std::time::Duration;

use super::*;
use axum::body::Body;
use support::StubReply;

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
    let (motor, served, _key) = stub_parts(
        vec![StubReply::new(200, "", "")],
        Duration::from_secs(HOLD_SECS),
    )
    .await;
    let hub = hub_over(
        &motor,
        &[("GRAPH_HUB_LAYOUTS", "1"), ("GRAPH_HUB_TIMEOUT_MS", "150")],
    )
    .await;
    let holder = hub.spawn(
        hub.request(
            "POST",
            &format!("/v1/workspaces/relay-mapped/layout?layout={LAYOUT}"),
        )
        .body(Body::empty())
        .expect("the relay request"),
    );
    // The holder holds the only `LAYOUTS` permit for as long as the stub holds its answer, which is
    // the whole reason the stub was given a hold here: a `LAYOUTS` permit travels with the response
    // body (§6), so an answer already written would have freed it.
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

/// How long `a_pool_wait_past_the_timeout_is_503_before_the_motor_is_called` holds the holder's answer,
/// in seconds.
///
/// Caveat: a fixture delay and not a bound the hub reads; it only has to outlast the holder's upload
/// and the 150 ms `GRAPH_HUB_TIMEOUT_MS` this case sets, and both arms are finished long before it is
/// over.
const HOLD_SECS: u64 = 3;
