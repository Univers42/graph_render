//! The motor's answer: its status mapped by §5.2's table, its body relayed unchanged.
//!
//! The two paths through this module are deliberately different. A 200 is **streamed**: the motor's
//! snapshot is the largest thing the hub relays, so it is never buffered and never counted, and it
//! is cut at `GRAPH_HUB_STREAM_DEADLINE_MS` like every other streamed body in the hub. Any other
//! status is the hub's own small JSON refusal, so its body is read whole — that is where the
//! `error` and `message` members §5.2's table keys on come from.

use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use futures_util::stream;
use graph_contract::hub::Cursor;
use hyper::body::Incoming;
use http_body_util::BodyExt;

use crate::error::HubApiError;
use crate::relay::RelayAnswer;
use crate::relay::map::{self, Mapped};

/// The refusal body §5.2's relayed rows are built from: graph-server's `error` and `message`.
struct Refusal {
    error: String,
    message: String,
}

/// How long a refusal body is read for.
///
/// Caveat: a fixed guess and not a §6 number, because this body is graph-server's own JSON refusal
/// and is written in one piece before the status line leaves it. Five seconds is above any loopback
/// write of a few hundred bytes and below `GRAPH_HUB_MOTOR_TIMEOUT_MS`, so a motor that sends a
/// status and then stalls costs this budget and not the relay's.
const REFUSAL_BUDGET: Duration = Duration::from_secs(5);

/// One motor answer, as the hub's own.
///
/// `status` is 200 and only 200: every other status is an `Err`, because a relayed answer is
/// rebuilt from graph-server's own `error` and `message` rather than passed through as bytes.
pub async fn read(
    parts: hyper::http::response::Parts,
    body: Incoming,
    deadline: tokio::time::Instant,
) -> Result<RelayAnswer, HubApiError> {
    let status = parts.status.as_u16();
    if status == 200 {
        return Ok(RelayAnswer {
            status,
            content_type: header_of(&parts.headers, header::CONTENT_TYPE),
            vary: header_of(&parts.headers, header::VARY),
            body: streaming(body, deadline),
        });
    }
    let refusal = refusal(body).await;
    let Mapped::Refuse(error) = map::fault(status, &refusal.error, &refusal.message) else {
        // `map::fault` reaches `Pass` only for a 200, which returned above; a 200 that arrives
        // twice is not a thing, so this arm is the hub saying so rather than guessing.
        return Err(HubApiError::Internal(String::from(
            "the motor mapped a refusal as a pass",
        )));
    };
    Err(error)
}

/// The motor's answer body as the hub's, cut at `deadline`.
///
/// Caveat: the cut ends the stream rather than failing it, so a caller reading a short body sees a
/// short body and re-reads its cursor — the same outcome §6's stream deadline gives every other
/// route. Trailing headers are not relayed: graph-server sends none on `/v1/layout`, and a frame
/// that carries them is skipped rather than turned into an error in the middle of a snapshot.
fn streaming(body: Incoming, deadline: tokio::time::Instant) -> Body {
    Body::from_stream(stream::unfold((body, deadline), |(mut body, deadline)| async move {
        let data = async {
            match tokio::time::timeout_at(deadline, body.frame()).await {
                Ok(Some(Ok(frame))) => match frame.into_data() {
                    Ok(data) => Some(Ok::<Bytes, std::io::Error>(data)),
                    // A trailer frame carries no bytes this hub relays and ends the stream:
                    // graph-server sends none on `/v1/layout`, and a wrong answer about the end
                    // of a snapshot is worse than a short one the caller re-reads at its cursor.
                    Err(_trailers) => None,
                },
                // The end of the body, a transport error and the deadline all end the stream the
                // same way: the caller sees a short body and re-reads at its cursor.
                Ok(Some(Err(error))) => Some(Err(std::io::Error::other(error))),
                Ok(None) | Err(_) => None,
            }
        }
        .await;
        data.map(|item| (item, (body, deadline)))
    }))
}

/// The `error` and `message` of a refusal body, whole.
///
/// A body that is not the JSON shape this map keys on reads as an empty `error`, which sends
/// `map::fault` to its default arm and makes the refusal a 502 `MotorError`. That is the right
/// direction: an answer the hub cannot read is an answer it cannot relay.
async fn refusal(body: Incoming) -> Refusal {
    let bytes = tokio::time::timeout(REFUSAL_BUDGET, body.collect())
        .await
        .ok()
        .and_then(|collected| collected.ok())
        .map(|collected| collected.to_bytes())
        .unwrap_or_default();
    let parsed: Result<serde_json::Value, _> = serde_json::from_slice(&bytes);
    let Ok(value) = parsed else {
        return Refusal {
            error: String::new(),
            message: String::new(),
        };
    };
    Refusal {
        error: member(&value, "error"),
        message: member(&value, "message"),
    }
}

/// One string member of a JSON object, or the empty string.
fn member(value: &serde_json::Value, name: &str) -> String {
    value
        .get(name)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// One header as text, `None` when it is absent or is not text.
fn header_of(headers: &HeaderMap, name: header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

/// The hub's response for a relay answer: graph-server's status, its `Content-Type` and `Vary`,
/// and `Graph-Seq` for the cursor the snapshot was read at.
///
/// Caveat: `Graph-Seq` is the snapshot's own cursor, so it is the same position `/graph`'s `ETag`
/// carries at that seq (H15) and not the workspace's head read after the upload: a write committed
/// while the relay ran must not move the header off the bytes the caller received.
pub fn respond(
    answer: RelayAnswer,
    permit: tokio::sync::OwnedSemaphorePermit,
    cursor: &Cursor,
) -> Response {
    use axum::http::HeaderValue;
    let seq = HeaderValue::from_str(&cursor.to_string())
        .unwrap_or_else(|_| HeaderValue::from_static("0.0"));
    let mut built = Response::builder()
        .status(StatusCode::from_u16(answer.status).unwrap_or(StatusCode::BAD_GATEWAY))
        .header("graph-seq", seq)
        .body(crate::relay::body::held(answer.body, permit))
        .unwrap_or_else(|_| Response::new(Body::empty()));
    if let Some(value) = header_value(&answer.content_type) {
        built.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    if let Some(value) = header_value(&answer.vary) {
        built.headers_mut().insert(header::VARY, value);
    }
    built
}

/// One relayed header value, or `None` when the motor sent nothing usable.
///
/// Caveat: a header that is not text, or is text a header value cannot carry, is dropped rather
/// than replaced: the snapshot below it is still the motor's own bytes, and a wrong
/// `Content-Type` on them would be worse than none.
fn header_value(text: &Option<String>) -> Option<axum::http::HeaderValue> {
    text.as_deref()
        .and_then(|text| axum::http::HeaderValue::from_str(text).ok())
}