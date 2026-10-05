//! The one helper more than one write child needs: a batch sent under a fixed idempotency key.
//!
//! It lives here rather than in `write.rs` because [`idem`] owns it and a second caller may join,
//! and both arms of a replay case must send the identical request to be the same request.

use axum::body::Body;

use crate::support::{Hub, Reply};

/// One batch sent under the fixed key `key-one`, which is what makes the two arms above the same
/// request.
pub async fn keyed(hub: &Hub, path: &str, body: &str) -> Reply {
    hub.send(
        hub.request("POST", path)
            .header("idempotency-key", "key-one")
            .body(Body::from(body.to_owned()))
            .expect("the request"),
    )
    .await
}