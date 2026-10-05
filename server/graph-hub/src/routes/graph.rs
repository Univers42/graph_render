//! `GET /v1/workspaces/{ws}/graph`: the materialized document, streamed, with an `ETag`.
//!
//! The body is the store's `Document` piece by piece ([`crate::routes::document`]) and the `READS`
//! permit is moved into that stream, so a permit is held exactly as long as the bytes are.
//!
//! Caveat: a `READS` permit is therefore held for up to `GRAPH_HUB_STREAM_DEADLINE_MS` by a slow
//! client. That is §6's stated cost of the cap and the reason the deadline exists at all.

use axum::body::Body;
use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use std::sync::Arc;

use crate::app::App;
use crate::auth::Credential;
use crate::error::HubApiError;
use crate::routes::{document, plugins::head_of};

/// The document at the workspace's current cursor, or a 304 for a matching `If-None-Match`.
pub async fn get(
    State(app): State<Arc<App>>,
    Extension(_credential): Extension<Credential>,
    Path(ws): Path<String>,
    headers: HeaderMap,
) -> Result<Response, HubApiError> {
    let permit = crate::gate::admit(&app, &app.gates.readers).await?;
    crate::hooks::pause_after_admit(&app.hooks, "graph").await;
    let store = app.store().await?;
    head_of(store, &ws).await?;
    let document = graph_store::materialize::open(store, &ws)
        .await
        .map_err(|error| crate::routes::write_fault(&error))?;
    let tag = crate::etag::quoted(&document.cursor());
    if crate::etag::matches(&headers, &document.cursor()) {
        // The `Document` is dropped here, which closes the snapshot: a 304 costs one read and no
        // bytes, and the permit goes back with the document.
        return Ok(not_modified(&tag));
    }
    let body = Body::from_stream(document::pieces(
        document,
        permit,
        app.settings.limits.stream_deadline,
    ));
    Ok(with_etag(body, &tag))
}

/// A 304 with the `ETag` and no body.
fn not_modified(tag: &str) -> Response {
    let value = HeaderValue::from_str(tag).unwrap_or_else(|_| HeaderValue::from_static("\"0.0\""));
    (StatusCode::NOT_MODIFIED, [(header::ETAG, value)]).into_response()
}

/// The 200: the stream, with `ETag` and the contract's own content type for a document.
///
/// Caveat: `application/x-ndjson` is not used; the document is one JSON object streamed in pieces,
/// so the content type is plain JSON and a client assembles it from the chunks.
fn with_etag(body: Body, tag: &str) -> Response {
    let value = HeaderValue::from_str(tag).unwrap_or_else(|_| HeaderValue::from_static("\"0.0\""));
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("application/json")),
            (header::ETAG, value),
        ],
        body,
    )
        .into_response()
}

use axum::response::IntoResponse;
