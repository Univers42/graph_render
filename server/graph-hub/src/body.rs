//! Reading a request body under two bounds at once: `GRAPH_HUB_MAX_BODY` and
//! `GRAPH_HUB_BODY_TIMEOUT_MS`.
//!
//! A declared `Content-Length` over the cap is refused **before a byte is read**, so a 5 GiB upload
//! costs the hub one header comparison rather than 5 GiB of buffer. A chunked body over the cap is
//! refused mid-stream, which is the only place a `Limited` can catch it.
//!
//! The timeout is the hub's own, and graph-server enforces an identical `GRAPH_BODY_TIMEOUT_MS` on its
//! side of the `/layout` relay, independently. Nothing orders the two: a body that is slow enough can
//! be a 408 here and a 502 `MotorBodyTimeout` there, and which one a caller sees is a race it must
//! not depend on. Caveat: that race is why the SDK never retries `/layout` (§5.3).

use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::http::HeaderMap;
use axum::http::header;
use http_body_util::{BodyExt, LengthLimitError, Limited};

use crate::error::HubApiError;

/// The whole body, or the refusal.
///
/// `413` past the cap, `408` past the budget, `400` when the body cannot be read at all — which is
/// a client that reset the connection mid-body, not a caller that sent a malformed request, and the
/// only honest class for it is the one that says the request was not usable.
pub async fn read(
    body: Body,
    headers: &HeaderMap,
    limit: u64,
    budget: Duration,
) -> Result<Bytes, HubApiError> {
    // `capped` is the `no-cap` break's one place: every cap passes through it, so the control turns
    // this 413 red along with the gates' 503s rather than leaving one cap standing.
    let limit = crate::config::capped(limit);
    let budget = crate::config::capped_millis(budget);
    if declared_over(headers, limit) {
        return Err(over_limit(limit));
    }
    // `Limited` counts `usize`, so the cap narrows here. §6's ceilings are 64 GiB at most, which
    // fits a `usize` on the 64-bit target this workspace builds for; a 32-bit build would refuse the
    // narrowing rather than wrap it.
    let capped = usize::try_from(limit).map_err(|_| over_limit(limit))?;
    let limited = Limited::new(body, capped);
    match tokio::time::timeout(budget, limited.collect()).await {
        Ok(Err(error)) if error.downcast_ref::<LengthLimitError>().is_some() => {
            Err(over_limit(limit))
        }
        Ok(Err(_)) => Err(HubApiError::BadRequest("the body could not be read")),
        Ok(Ok(collected)) => Ok(collected.to_bytes()),
        Err(_) => Err(HubApiError::BodyTimeout),
    }
}

/// Does the head declare more than `limit`?
///
/// A `Content-Length` that does not parse is **not** a 413: it is not a length the hub can compare,
/// and hyper refuses such a request before it reaches here. The check therefore only ever refuses on
/// a number it read.
fn declared_over(headers: &HeaderMap, limit: u64) -> bool {
    headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|text| text.parse::<u64>().ok())
        .is_some_and(|declared| declared > limit)
}

/// The 413, naming the limit it was over so a caller can see which knob to move.
fn over_limit(limit: u64) -> HubApiError {
    HubApiError::TooLarge {
        what: "the body",
        limit,
    }
}
