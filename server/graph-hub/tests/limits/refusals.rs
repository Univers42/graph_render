//! The 429s, and the shape every refusal answers with: a status, `Retry-After` where §5.2 gives
//! one, and a bounded one-line JSON body.

use std::sync::Arc;

use axum::response::IntoResponse;
use graph_hub::config::Subscribers as Caps;
use graph_hub::gate::subscribers::Subscribers;
use graph_hub::{HubApiError, MotorFault};

use crate::support::*;

/// The subscriber counters move together: one slot, one total, and a key whose streams all end has no
/// entry at all.
#[tokio::test]
async fn the_subscriber_counters_move_together() {
    let counters = Arc::new(Subscribers::new(Caps {
        max: 64,
        per_key: 8,
    }));
    let one = counters.admit("tester").expect("a slot");
    assert_eq!(counters.total(), 1);
    assert_eq!(counters.of("tester"), 1);
    assert_eq!(counters.keys().len(), 1);
    drop(one);
    assert_eq!(counters.total(), 0, "the total follows the slot");
    assert!(
        counters.keys().is_empty(),
        "the per-key entry follows the slot"
    );
}

/// A permit wait past `GRAPH_HUB_TIMEOUT_MS` is a 503 and never a 500: a busy hub is not a broken
/// one, and a client that retries a 503 is doing the right thing.
#[tokio::test]
async fn a_permit_wait_past_the_timeout_is_503_not_500() {
    let gate = Gate::new(1);
    let _held = gate
        .admit(deadline(PATIENCE))
        .await
        .expect("the one permit");
    let refused = gate.admit(deadline(PATIENCE)).await.unwrap_err();
    assert_eq!(refused.status(), 503);
    assert_ne!(refused.status(), 500);
    assert_eq!(refused.code(), "Busy");
}

/// A pool wait past the timeout is the same 503: the permit and the pool are the same kind of wait,
/// and §5.2 gives both the same status.
#[tokio::test]
async fn a_pool_wait_past_the_timeout_is_503_with_retry_after() {
    let hub = hub_with_env(&[("GRAPH_HUB_TIMEOUT_MS", "100")]);
    let deadline = deadline(hub.app.settings.limits.timeout);
    let gate = Gate::new(1);
    let _held = gate.admit(deadline).await.expect("the one permit");
    let refused = gate.admit(deadline).await.unwrap_err();
    assert_eq!(refused.status(), 503);
    let response = refused.into_response();
    assert_eq!(
        response
            .headers()
            .get("retry-after")
            .map(|v| v.to_str().unwrap()),
        Some("1"),
        "§5.2's 503 carries Retry-After"
    );
}

/// The response a 503 carries: `Retry-After: 1`, and the JSON body every refusal uses.
#[tokio::test]
async fn a_503_carries_retry_after_and_the_json_shape() {
    let gate = Gate::new(1);
    let _held = gate
        .admit(deadline(PATIENCE))
        .await
        .expect("the one permit");
    let response = gate
        .admit(deadline(PATIENCE))
        .await
        .unwrap_err()
        .into_response();
    assert_eq!(response.status(), 503);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .map(|v| v.to_str().unwrap()),
        Some("application/json")
    );
    assert_eq!(
        response
            .headers()
            .get("retry-after")
            .map(|v| v.to_str().unwrap()),
        Some("1")
    );
}

/// A 401 carries `WWW-Authenticate: Bearer`, so a client knows what to send — and the 503 above does
/// not carry it, because a busy hub is not a credential problem.
#[tokio::test]
async fn a_401_carries_www_authenticate_and_a_503_does_not() {
    use graph_hub::HubApiError;
    let unauthorized = HubApiError::Unauthorized("missing or unknown API key").into_response();
    assert_eq!(unauthorized.status(), 401);
    assert_eq!(
        unauthorized
            .headers()
            .get("www-authenticate")
            .map(|v| v.to_str().unwrap()),
        Some("Bearer")
    );
    let busy = HubApiError::busy_wait().into_response();
    assert_eq!(busy.status(), 503);
    assert!(busy.headers().get("www-authenticate").is_none());
}

/// The `no-cap` break's subject, read here so the control has something to turn red even before the
/// routes of Task 5 exist: a gate built under it has 64 permits whatever §6 says.
#[test]
fn the_no_cap_break_reads_every_limit_as_no_cap_permits() {
    let gate = Gate::new(2);
    let expected = if graph_hub::breaks::on("no-cap") {
        graph_hub::gate::NO_CAP_PERMITS
    } else {
        2
    };
    assert_eq!(gate.free(), expected);
}

/// Every refusal is one line and one JSON object, whatever produced it: a `Caveat` that a 256-byte
/// message is a prefix is not a shape change.
#[tokio::test]
async fn every_refusal_is_a_bounded_one_line_json_body() {
    use graph_hub::HubApiError;
    let long = "x".repeat(4096);
    let response = HubApiError::Internal(long).into_response();
    let body = read_body(response).await;
    assert!(
        body.len() < 512,
        "the body is bounded: {} bytes",
        body.len()
    );
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("a JSON body");
    assert_eq!(parsed["error"], "internal");
    assert!(
        parsed["message"].as_str().unwrap().ends_with("..."),
        "a message past the bound is a prefix and says so"
    );
}

/// The refusal header table, read from the responses rather than from the enum: 429 and 503 carry
/// `Retry-After`, 401 carries `WWW-Authenticate`, and the relayed motor answer carries neither.
#[tokio::test]
async fn the_refusal_headers_come_from_the_status_and_not_the_variant() {
    use graph_hub::{HubApiError, MotorFault};
    let cases = [
        (
            HubApiError::BadRequest("the path id is empty"),
            400u16,
            false,
        ),
        (HubApiError::Forbidden("no grant"), 403, false),
        (HubApiError::NotFound("no such route"), 404, false),
        (HubApiError::busy_subscriber(1), 429, true),
        (HubApiError::busy_wait(), 503, true),
        (HubApiError::Motor(MotorFault::Unavailable), 502, false),
    ];
    for (error, status, retry) in cases {
        let response = error.into_response();
        assert_eq!(response.status().as_u16(), status, "{status}");
        assert_eq!(
            response.headers().get("retry-after").is_some(),
            retry,
            "status {status} carries Retry-After: {retry}"
        );
    }
}

/// Read a response body as text.
pub(crate) async fn read_body(response: axum::response::Response) -> String {
    use http_body_util::BodyExt;
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("the body")
        .to_bytes();
    String::from_utf8_lossy(&bytes).into_owned()
}
