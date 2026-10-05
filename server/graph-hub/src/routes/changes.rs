//! `GET /v1/workspaces/{ws}/changes`: the change feed after a cursor, one bounded page at a time.
//!
//! The order is fixed and is the one §5.2's refusals need: **the cursor is checked before the page
//! is read**, so a cursor that went invalid costs one cheap statement and not a whole page read
//! ([`graph_store::changes::cursor_state`], then [`graph_store::changes::page`]). The page's own
//! texts are graph-contract's — the store rebuilds them with `change_json` — so the hub parses them
//! into the answer and never re-serializes one.

use axum::extract::{Extension, Path, State};
use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use std::collections::BTreeMap;
use std::sync::Arc;

use graph_contract::hub::Cursor;
use graph_store::changes::{ChangesReq, CursorState};

use crate::app::App;
use crate::auth::Credential;
use crate::error::HubApiError;
use crate::routes::{plugins::head_of, query_of, write_fault};

/// One page of changes after `?since=`.
pub async fn get(
    State(app): State<Arc<App>>,
    Extension(_credential): Extension<Credential>,
    Path(ws): Path<String>,
    uri: Uri,
) -> Result<Response, HubApiError> {
    let permit = crate::gate::admit(&app, &app.gates.readers).await?;
    crate::hooks::pause_after_admit(&app.hooks, "changes").await;
    let query = query_of(&uri);
    let store = app.store().await?;
    // The head is read first so a workspace that is not there is a 404 before any cursor work, and
    // so `check` below has the store's own bounds to compare against.
    head_of(store, &ws).await?;
    let since = since_of(&query)?;
    check(store, &ws, since).await?;
    let page = graph_store::changes::page(
        store,
        &ChangesReq {
            ws: ws.clone(),
            since,
            limit: limit(&query, app.as_ref())?,
            max_bytes: app.settings.limits.changes_bytes,
        },
    )
    .await
    .map_err(|error| gone_or_fault(&error))?;
    drop(permit);
    Ok(page_body(&page))
}

/// Is `since` still servable for `ws`? A gone cursor is a 410, and the page read never runs.
async fn check(store: &graph_store::Store, ws: &str, since: Cursor) -> Result<(), HubApiError> {
    match graph_store::changes::cursor_state(store, ws, since).await {
        Ok(CursorState::Valid) => Ok(()),
        Ok(CursorState::Gone) => Err(HubApiError::Gone(String::from(
            "the cursor is from another epoch or outside what is kept",
        ))),
        Err(error) => Err(write_fault(&error)),
    }
}

/// A `StoreError::Gone` from the page read is the same 410 the check would have given.
fn gone_or_fault(error: &graph_store::StoreError) -> HubApiError {
    match error {
        graph_store::StoreError::Gone => {
            HubApiError::Gone(String::from("the cursor is outside what is kept"))
        }
        other => write_fault(other),
    }
}

/// The page's answer: the epoch, the head, the next cursor, the changes, and the bytes they took.
///
/// The changes are the store's own texts, parsed. A text that does not parse is a store fault, not
/// a refusal: the store rebuilt it with graph-contract's `change_json` on the way out.
fn page_body(page: &graph_store::changes::ChangePage) -> Response {
    let changes: Vec<serde_json::Value> = page
        .changes
        .iter()
        .map(|change| serde_json::from_str(&change.text).unwrap_or(serde_json::Value::Null))
        .collect();
    let body = serde_json::json!({
        "epoch": page.epoch,
        "head_seq": page.head_seq,
        "next": page.next.to_string(),
        "bytes": page.bytes,
        "changes": changes,
    });
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, HeaderValue::from_static("application/json"))],
        body.to_string(),
    )
        .into_response()
}

/// `?since=` as a cursor, required.
///
/// Caveat: `since` has no default. A cursor needs an epoch and only the workspace's own row has
/// one, so a missing `since` cannot be filled in without a read whose answer the caller did not ask
/// for; §5.2's row always names the parameter.
fn since_of(query: &BTreeMap<String, String>) -> Result<Cursor, HubApiError> {
    let raw = query
        .get("since")
        .ok_or(HubApiError::BadRequest("since is required, as <epoch>.<seq>"))?;
    Cursor::parse(raw).map_err(|_| HubApiError::BadRequest("since is not <epoch>.<seq>"))
}

/// `?limit=`: at most §6's `SSE_PAGE` changes, because one page is one stream read's worth.
fn limit(query: &BTreeMap<String, String>, app: &App) -> Result<u64, HubApiError> {
    let Some(raw) = query.get("limit") else {
        return Ok(crate::config::capped(app.settings.sse_page));
    };
    let asked: u64 = raw
        .parse()
        .map_err(|_| HubApiError::BadRequest("limit is not a number"))?;
    if asked == 0 {
        return Err(HubApiError::BadRequest("limit is at least 1"));
    }
    Ok(crate::config::capped(asked.min(app.settings.sse_page)))
}
