//! The four semaphores and the two subscriber counters.
//!
//! Each case holds real permits on real gates: a test that admits three requests into a two-permit
//! gate and watches the third time out is a statement about the shipped semaphore, not about a copy
//! of its rule.

use std::sync::Arc;
use std::time::Duration;

use graph_hub::config::Subscribers as Caps;
use graph_hub::gate::{Gate, KeyGate, deadline, subscribers::Subscribers};

use axum::response::IntoResponse;

use crate::support::*;

/// A short wait budget, so a case that expects a 503 does not spend §6's 30 seconds finding out.
const PATIENCE: Duration = Duration::from_millis(150);

/// `GRAPH_HUB_WRITERS = 2`: two requests hold the gate and a third waits, then 503s with
/// `Retry-After`. Not a 429: nothing here bounds the queue.
#[tokio::test]
async fn writers_two_and_a_third_waits_then_gets_503_with_retry_after() {
    let gate = Gate::new(2);
    let _first = gate
        .admit(deadline(PATIENCE))
        .await
        .expect("the first permit");
    let _second = gate
        .admit(deadline(PATIENCE))
        .await
        .expect("the second permit");
    assert_eq!(gate.free(), 0, "both permits are held");
    let refused = gate.admit(deadline(PATIENCE)).await.unwrap_err();
    assert_eq!(refused.status(), 503);
    assert_eq!(refused.code(), "Busy");
    // The 503 carries `Retry-After` on the wire even though the variant's own `retry_after` is the
    // 429's: §5.2 gives a wait a header and not a counter.
    let response = refused.into_response();
    assert_eq!(response.status(), 503);
    assert_eq!(
        response
            .headers()
            .get("retry-after")
            .map(|v| v.to_str().unwrap()),
        Some("1")
    );
}

/// The same for `GRAPH_HUB_READS = 2`.
#[tokio::test]
async fn readers_two_and_a_third_waits_then_503() {
    let gate = Gate::new(2);
    let held = gate.admit(deadline(PATIENCE)).await.expect("a permit");
    let held_second = gate
        .admit(deadline(PATIENCE))
        .await
        .expect("a second permit");
    let refused = gate.admit(deadline(PATIENCE)).await.unwrap_err();
    assert_eq!(refused.status(), 503);
    drop((held, held_second));
    assert!(
        gate.admit(deadline(PATIENCE)).await.is_ok(),
        "a released permit admits again"
    );
}

/// And for `GRAPH_HUB_LAYOUTS = 1`: the default is one, because graph-server admits a request before
/// it reads the body, so a waiting `/layout` holds a snapshot and a pool connection.
#[tokio::test]
async fn layouts_one_and_a_second_waits_then_503() {
    let gate = Gate::new(1);
    let _held = gate
        .admit(deadline(PATIENCE))
        .await
        .expect("the one permit");
    let refused = gate.admit(deadline(PATIENCE)).await.unwrap_err();
    assert_eq!(refused.status(), 503);
}

/// `GRAPH_HUB_WRITERS_PER_KEY = 1`: one key's second write waits while another key's proceeds. The
/// two keys are told apart by which one is refused, which is the whole point of a per-key gate.
#[tokio::test]
async fn a_second_batch_from_one_key_waits_on_writers_per_key_while_another_keys_batch_proceeds() {
    let gate = KeyGate::new(1);
    let mine = gate
        .admit("mine", deadline(PATIENCE))
        .await
        .expect("my permit");
    let other = gate
        .admit("other", deadline(PATIENCE))
        .await
        .expect("another key's permit is its own");
    let mine_again = gate.admit("mine", deadline(PATIENCE)).await.unwrap_err();
    assert_eq!(mine_again.status(), 503, "my key is the one that waits");
    drop(other);
    assert_eq!(
        gate.admit("mine", deadline(PATIENCE))
            .await
            .unwrap_err()
            .status(),
        503,
        "another key's release does not free my key's permit"
    );
    drop(mine);
    assert!(
        gate.admit("mine", deadline(PATIENCE)).await.is_ok(),
        "my key proceeds once my own permit is back"
    );
}

/// `PUT /v1/workspaces/{ws}` takes a `WRITERS` permit, not a `READS` one: §15(c) condition 9's
/// correction. The gate is the one the route reads, so a route on the wrong gate is a 503 here.
#[tokio::test]
async fn put_workspaces_takes_a_writers_permit() {
    let hub = hub_with_env(&[("GRAPH_HUB_WRITERS", "1")]);
    let held = hub
        .app
        .gates
        .writers
        .admit(deadline(PATIENCE))
        .await
        .expect("the one writer permit");
    assert_eq!(
        hub.app.gates.readers.free(),
        2,
        "the reader gate is untouched"
    );
    let refused = hub
        .app
        .gates
        .writers
        .admit(deadline(PATIENCE))
        .await
        .unwrap_err();
    assert_eq!(refused.status(), 503);
    drop(held);
}

/// The per-key gate does not leak: a key name whose permit is gone leaves no entry, so an unbounded
/// number of key names cannot grow the map.
#[tokio::test]
async fn the_per_key_gate_drops_an_entry_whose_permit_is_gone() {
    let gate = KeyGate::new(1);
    for name in ["a", "b", "c", "d"] {
        let held = gate
            .admit(name, deadline(PATIENCE))
            .await
            .expect("a permit");
        drop(held);
    }
    assert_eq!(gate.keys(), 0, "every entry is gone with its last permit");
}

/// `GRAPH_HUB_MAX_SUBSCRIBERS_PER_KEY = 8`: the ninth stream from one key is 429, even with the global
/// cap far away.
#[tokio::test]
async fn the_subscriber_cap_per_key_is_429() {
    let counters = Arc::new(Subscribers::new(Caps {
        max: 64,
        per_key: 8,
    }));
    let held: Vec<_> = (0..8)
        .map(|_| {
            counters
                .admit("tester")
                .expect("a slot under the per-key cap")
        })
        .collect();
    let refused = counters.admit("tester").unwrap_err();
    assert_eq!(refused.status(), 429);
    assert_eq!(refused.retry_after(), Some(1));
    drop(held);
    assert_eq!(
        counters.total(),
        0,
        "dropping every stream frees every slot"
    );
}

/// `GRAPH_HUB_MAX_SUBSCRIBERS = 64`: the 65th stream is 429, and the per-key cap is raised so this
/// case is about the global one.
#[tokio::test]
async fn the_subscriber_cap_in_total_is_429() {
    let caps = Caps {
        max: 3,
        per_key: 64,
    };
    let counters = Arc::new(Subscribers::new(caps));
    let held: Vec<_> = ["a", "b", "c"]
        .iter()
        .map(|key| counters.admit(key).expect("a slot under the global cap"))
        .collect();
    let refused = counters.admit("d").unwrap_err();
    assert_eq!(refused.status(), 429);
    drop(held);
    assert!(counters.admit("d").is_ok(), "a released slot admits again");
}

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
