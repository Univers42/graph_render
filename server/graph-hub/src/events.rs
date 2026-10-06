//! §5.3's notice stream: `GET /v1/workspaces/{ws}/events`.
//!
//! Three modules and one route. [`page`] reads a bounded number of change headers, [`stream`] is
//! the loop that pages them and keeps the subscriber's cursor, and [`beat`] is the heartbeat that
//! also re-reads the epoch. This module holds the route and the resume rule, and nothing else.
//!
//! The resume rule is §5.3's and it has three sources, in this order: `Last-Event-ID` (what a
//! reconnecting SDK sends, and the only source that survives a hub restart), then `?since=`, then the
//! workspace's own current position — which answers "everything from now", the only default that is
//! right for a subscriber that has no cursor.
//!
//! Caveat: no source is validated against the workspace's epoch before the stream starts, because the
//! first page read is what decides that and it costs one statement either way. A `Last-Event-ID` from
//! another epoch therefore becomes a `resync` event on the stream rather than a 410 before it.

pub mod beat;
pub mod page;
pub mod stream;
pub mod wire;

use std::sync::Arc;

use axum::extract::{Extension, Path, State};
use axum::http::HeaderMap;
use axum::response::sse::{KeepAlive, Sse};
use axum::response::{IntoResponse, Response};

use graph_contract::hub::Cursor;

use crate::app::App;
use crate::auth::Credential;
use crate::error::HubApiError;
use crate::routes::plugins::head_of;

/// `GET /v1/workspaces/{ws}/events`: the notice stream.
///
/// A `SUBSCRIBERS` permit first and before anything else, because §6's cap is on streams rather than
/// on requests: a hub that admitted a stream it will refuse to serve has spent a connection and a
/// task on nothing. The permit is then moved into the stream, because it must live as long as the
/// stream does.
///
/// Caveat: no `READS` permit is taken here. A stream reads pages over its whole life and a `READS`
/// permit would then be held for as long as the client stays connected, which is the whole point of
/// having a separate subscriber counter; §6 names `READS` for the read routes, and this is not one.
pub async fn get(
    State(app): State<Arc<App>>,
    Extension(credential): Extension<Credential>,
    Path(ws): Path<String>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Result<Response, HubApiError> {
    let subscriber = app.gates.subscribers.admit(&credential.key)?;
    crate::hooks::pause_after_admit(&app.hooks, "events").await;
    let store = app.store().await?;
    let (epoch, head_seq) = head_of(store, &ws).await?;
    let cursor = cursor_of(&headers, &uri, epoch, head_seq)?;
    let keep_alive = KeepAlive::new().interval(beat::EVERY).text("keep-alive");
    let body = Sse::new(stream::subscribe(
        Arc::clone(&app),
        ws,
        cursor,
        epoch,
        subscriber,
    ))
    .keep_alive(keep_alive);
    Ok(body.into_response())
}

/// Where this stream resumes from: `Last-Event-ID`, else `?since=`, else the workspace's position.
///
/// A malformed cursor is a 400 here rather than a `resync` on the stream, because the client sent it
/// and can fix it; a cursor that is well formed but no longer valid is the store's answer and becomes
/// `resync`.
///
/// `since` comes from the request URI rather than an argument so the three sources are read in one
/// place; the header wins because it is what the SDK sets on a reconnect and it is a header for
/// exactly that reason.
pub fn cursor_of(
    headers: &HeaderMap,
    uri: &axum::http::Uri,
    epoch: u64,
    head_seq: u64,
) -> Result<Cursor, HubApiError> {
    if let Some(raw) = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return Cursor::parse(raw)
            .map_err(|_| HubApiError::BadRequest("Last-Event-ID is not <epoch>.<seq>"));
    }
    let query = crate::routes::query_of(uri);
    if let Some(raw) = query.get("since") {
        return Cursor::parse(raw)
            .map_err(|_| HubApiError::BadRequest("since is not <epoch>.<seq>"));
    }
    Ok(Cursor {
        epoch,
        seq: head_seq,
    })
}
