//! `PUT /v1/workspaces/{ws}` and `GET /v1/workspaces`: the two routes that name no plugin.
//!
//! Both go through [`graph_store::epoch`], the store's own read of a workspace row, so the hub asks
//! "is this workspace there, and where is its head" in one statement and never writes SQL.

use axum::extract::{Extension, Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::IntoResponse;
use std::sync::Arc;

use graph_contract::hub::Cursor;
use graph_store::Store;

use crate::app::App;
use crate::auth::Credential;
use crate::error::HubApiError;
use crate::grants::Need;
use crate::routes::{limits, write_fault};

/// `PUT /v1/workspaces/{ws}`: 201 on the insert, 200 when the row was already there.
///
/// A `WRITERS` permit, never a `READS` one (§15(c) condition 9's correction): this is a write even
/// though §6's `READS` row lists `/workspaces`, which is the `GET`.
///
/// Caveat: creating a workspace moves no stream, so the answer carries no `Graph-Seq` and the watch
/// is not raised. The only fact the route asserts is the row's existence.
pub async fn put(
    State(app): State<Arc<App>>,
    Extension(credential): Extension<Credential>,
    Path(ws): Path<String>,
) -> Result<axum::response::Response, HubApiError> {
    let _permit = crate::gate::admit(&app, &app.gates.writers).await?;
    let _key = crate::gate::admit_key(&app, &credential.key).await?;
    crate::hooks::pause_after_admit(&app.hooks, "put-workspaces").await;
    let store = app.store().await?;
    let inserted = store
        .create_workspace(&ws, &limits(&app))
        .await
        .map_err(|error| write_fault(&error))?;
    let status = if inserted {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    let body = serde_json::json!({ "workspace": ws }).to_string();
    Ok((status, [(header::CONTENT_TYPE, json())], body).into_response())
}

/// `GET /v1/workspaces`: the workspaces this key may read, each with its `epoch` and `head_seq`, in
/// id order.
///
/// A `READS` permit: §6's `READS` row names this route, and it reads no document.
///
/// Caveat: one `READS` permit covers the whole listing rather than one per workspace, so a key with
/// a grant on thousands of workspaces holds one permit for a list that took thousands of statements.
/// §6 caps the answer with nothing, and this is the cost of a route with no pagination.
pub async fn list(
    State(app): State<Arc<App>>,
    Extension(credential): Extension<Credential>,
) -> Result<axum::response::Response, HubApiError> {
    let permit = crate::gate::admit(&app, &app.gates.readers).await?;
    crate::hooks::pause_after_admit(&app.hooks, "list-workspaces").await;
    let store = app.store().await?;
    let rows = readable(app.as_ref(), store, &credential).await?;
    drop(permit);
    let body = serde_json::json!({ "workspaces": rows }).to_string();
    Ok(([(header::CONTENT_TYPE, json())], body).into_response())
}

/// Every workspace `key` may read, with `(id, epoch, head_seq)`, in id order.
///
/// The id order is PostgreSQL's own, and it is the only order: §5.2 names none, and a list whose
/// order moved between two calls would make a cursor over it meaningless.
async fn readable(
    app: &App,
    store: &Store,
    credential: &Credential,
) -> Result<Vec<serde_json::Value>, HubApiError> {
    let mut client = store.client().await.map_err(|e| write_fault(&e))?;
    let ids = graph_store::epoch::workspace_ids(&mut client)
        .await
        .map_err(|error| write_fault(&error))?;
    let pair = app.keys.current();
    let mut out = Vec::new();
    for id in ids {
        if !pair.1.allows(&credential.key, &id, &Need::Read) {
            continue;
        }
        let Some((epoch, head_seq)) = graph_store::epoch::head_of(&mut client, &id)
            .await
            .map_err(|error| write_fault(&error))?
        else {
            continue;
        };
        out.push(entry(&id, epoch, head_seq));
    }
    Ok(out)
}

/// One row of the listing: the workspace's id and its position, both as §5.2 spells them.
fn entry(id: &str, epoch: u64, head_seq: u64) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "epoch": epoch,
        "head_seq": head_seq,
        "cursor": Cursor { epoch, seq: head_seq }.to_string(),
    })
}

/// The `application/json` every answer of this module carries.
fn json() -> HeaderValue {
    HeaderValue::from_static("application/json")
}
