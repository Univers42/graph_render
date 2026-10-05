//! The three idempotency-key cases of §5.1: the replay, the key reused with another body, and the
//! 128-byte cap.

use axum::body::Body;

use crate::common::keyed;
use crate::support::fixtures::{hub_db, ready, upsert};

/// §5.1 step 2: a replay with the same key and the same body returns the stored response and adds
/// no seq. This is the test row `hub-idem` filters on, and the one the store's `no-idem` break
/// turns red.
#[tokio::test]
async fn idempotency_replay_returns_the_same_response_and_the_same_head_seq() {
    let hub = hub_db(&[]).await;
    ready(&hub, "idem", "task").await;
    let path = "/v1/workspaces/idem/plugins/task/batches";
    let body = upsert("task", "replayed", "note");
    let first = keyed(&hub, path, &body).await;
    assert_eq!(first.code(), 200, "{}", first.body());
    let replay = keyed(&hub, path, &body).await;
    assert_eq!(replay.code(), 200, "{}", replay.body());
    assert_eq!(
        replay.body(),
        first.body(),
        "the stored response, byte for byte"
    );
}

/// The same key with a different body is a 422 and applies nothing: §5.1 treats the key as a claim
/// about one body.
#[tokio::test]
async fn the_same_key_with_another_body_is_422() {
    let hub = hub_db(&[]).await;
    ready(&hub, "idem2", "task").await;
    let path = "/v1/workspaces/idem2/plugins/task/batches";
    let first = hub
        .send(
            hub.request("POST", path)
                .header("idempotency-key", "shared")
                .body(Body::from(upsert("task", "one", "n")))
                .expect("the request"),
        )
        .await;
    assert_eq!(first.code(), 200, "{}", first.body());
    let other = hub
        .send(
            hub.request("POST", path)
                .header("idempotency-key", "shared")
                .body(Body::from(upsert("task", "two", "n")))
                .expect("the request"),
        )
        .await;
    assert_eq!(other.code(), 422, "{}", other.body());
}

/// §5.1 caps an idempotency key at 128 **bytes**, and it is a 422 rather than a 413 because the key
/// is a claim, not a payload.
#[tokio::test]
async fn an_idempotency_key_over_128_bytes_is_422() {
    let hub = hub_db(&[]).await;
    ready(&hub, "idem3", "task").await;
    let long = "k".repeat(129);
    let refused = hub
        .send(
            hub.request("POST", "/v1/workspaces/idem3/plugins/task/batches")
                .header("idempotency-key", long)
                .body(Body::from(upsert("task", "one", "n")))
                .expect("the request"),
        )
        .await;
    assert_eq!(refused.code(), 422, "{}", refused.body());
}
