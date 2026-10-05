//! `GET /v1/meta`: the API version and every §6 limit, from the same `Settings` the start check
//! read.
//!
//! One source, so the two cannot disagree: the start check refused a deployment over these numbers
//! and `/v1/meta` publishes the numbers it refused over. A client sizes itself from this and never
//! reads `docs/deploy/hub.md`.
//!
//! Caveat: the answer carries every cap but no value of any secret, and `GRAPH_HUB_DB_URL` appears
//! as `unset` here exactly as it does in the start line — a client's need to know that a database is
//! configured is not a need to read the URL.

use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

use crate::app::App;
use crate::auth::Credential;
use crate::config::Limits;

/// `{"api":1,"version":…,"limits":{…}}`.
///
/// `version` is this crate's own `CARGO_PKG_VERSION`. Caveat: the plan names "the version from
/// `graph_server::config`", and that crate publishes no version item — its `start_line` reads its
/// own `env!("CARGO_PKG_VERSION")` (`server/graph-server/src/config.rs:131`), which a client of
/// `/v1/meta` would read as the hub's version. A hub client needs the hub's version, so that is what
/// this is.
pub async fn meta(
    State(app): State<Arc<App>>,
    axum::extract::Extension(_credential): axum::extract::Extension<Credential>,
) -> Result<Response, crate::error::HubApiError> {
    let body = serde_json::json!({
        "api": 1,
        "version": env!("CARGO_PKG_VERSION"),
        "limits": limits(&app),
    });
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, HeaderValue::from_static("application/json"))],
        body.to_string(),
    )
        .into_response())
}

/// Every number of §6, keyed by the name a deployment sets it under without the `GRAPH_HUB_` prefix.
///
/// Durations are milliseconds because that is the unit every `GRAPH_HUB_*_MS` variable is in, so a
/// client comparing this against its own configuration compares like with like.
fn limits(app: &App) -> serde_json::Value {
    let read: &Limits = &app.settings.limits;
    let gates = &app.settings.gates;
    let caps = &app.settings.subscribers;
    let net = &app.settings.connections;
    serde_json::json!({
        "max_body": read.max_body,
        "max_batch": read.max_batch,
        "max_record_bytes": read.max_record_bytes,
        "max_plugin_bytes": read.max_plugin_bytes,
        "max_doc_bytes": read.max_doc_bytes,
        "retain": read.retain,
        "retain_bytes": read.retain_bytes,
        "changes_bytes": read.changes_bytes,
        "body_timeout_ms": read.body_timeout.as_millis() as u64,
        "stream_deadline_ms": read.stream_deadline.as_millis() as u64,
        "motor_timeout_ms": read.motor_timeout.as_millis() as u64,
        "timeout_ms": read.timeout.as_millis() as u64,
        "sse_page": read.sse_page,
        "fetch_rows": read.fetch_rows,
        "last_seen": read.last_seen,
        "writers": gates.writers,
        "readers": gates.readers,
        "layouts": gates.layouts,
        "writers_per_key": gates.writers_per_key,
        "max_connections": net.max_connections,
        "header_timeout_ms": net.header_timeout.as_millis() as u64,
        "max_header_bytes": net.max_header_bytes,
        "max_subscribers": caps.max,
        "max_subscribers_per_key": caps.per_key,
    })
}
