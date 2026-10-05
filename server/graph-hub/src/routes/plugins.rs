//! `PUT /v1/workspaces/{ws}/plugins/{plugin}` and `GET /v1/workspaces/{ws}/plugins`: the manifest
//! half of §5.2's table.
//!
//! The PUT body is read by graph-contract's own [`read_manifest`] and the answer is its own
//! [`manifest_json`], so the hub is never a second producer of manifest text (§5.2's last rule).

use axum::body::Body;
use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

use graph_contract::hub::{manifest_json, read_manifest};
use graph_store::writer::ManifestWrite;

use crate::app::App;
use crate::auth::Credential;
use crate::error::HubApiError;
use crate::routes::{limits, scan, write_fault};

/// `PUT /v1/workspaces/{ws}/plugins/{plugin}`: 201 on the insert, 200 when it was already there,
/// and `manifest_json` of the manifest just read as the body.
///
/// A `WRITERS` permit, and no per-key one: §6's `WRITERS_PER_KEY` row is about a key's *batches*,
/// and a manifest registration cannot interleave with itself destructively — the store takes the
/// workspace row lock either way.
pub async fn put(
    State(app): State<Arc<App>>,
    Extension(_credential): Extension<Credential>,
    Path((ws, plugin)): Path<(String, String)>,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, HubApiError> {
    let permit = crate::gate::admit(&app, &app.gates.writers).await?;
    crate::hooks::pause_after_admit(&app.hooks, "put-manifest").await;
    let text = crate::body::read(
        body,
        &headers,
        app.settings.limits.max_body,
        app.settings.limits.body_timeout,
    )
    .await?;
    let manifest =
        read_manifest(&lossy(&text), &plugin).map_err(|e| crate::routes::hub_fault(&e))?;
    let write = ManifestWrite {
        ws: ws.clone(),
        plugin: plugin.clone(),
        manifest,
        limits: limits(&app),
    };
    let written = app
        .store()
        .await?
        .put_manifest(&write)
        .await
        .map_err(|error| write_fault(&error))?;
    let status = StatusCode::from_u16(written.status).unwrap_or(StatusCode::OK);
    // §5.1's ordering: the store's step 8 has returned before anything is answered or published.
    crate::hooks::before_ack(&app.hooks, written.seq);
    drop(permit);
    Ok((
        status,
        [(header::CONTENT_TYPE, json())],
        manifest_json(&write.manifest),
    )
        .into_response())
}

/// `GET /v1/workspaces/{ws}/plugins`: every registered manifest, in plugin order.
///
/// A `READS` permit (§6's `READS` row names this route) and a page per plugin, streamed as one JSON
/// array rather than held whole — see [`plugins_body`] for the byte shape.
pub async fn list(
    State(app): State<Arc<App>>,
    Extension(_credential): Extension<Credential>,
    Path(ws): Path<String>,
) -> Result<Response, HubApiError> {
    let permit = crate::gate::admit(&app, &app.gates.readers).await?;
    crate::hooks::pause_after_admit(&app.hooks, "list-plugins").await;
    let store = app.store().await?;
    let head = head_of(store, &ws).await?;
    let manifests = scan::manifests(store, &ws, head)
        .await
        .map_err(|e| write_fault(&e))?;
    drop(permit);
    Ok(plugins_body(&manifests))
}

/// The answer of [`list`]: `{"manifests":[{"plugin", "manifest"}]}` in plugin order.
///
/// Caveat: `manifest_json` is graph-contract's canonical text nested whole, so the hub emits the
/// manifest as a JSON **string** rather than as an embedded object. That is deliberate: the bytes a
/// client reads back are the bytes it published, and §5.2's manifest row is the stored manifest and
/// not a re-serialization of it. A client parses the string with the same reader it wrote with.
fn plugins_body(
    manifests: &std::collections::BTreeMap<String, graph_contract::hub::Manifest>,
) -> Response {
    let rows: Vec<serde_json::Value> = manifests
        .iter()
        .map(|(plugin, manifest)| {
            serde_json::json!({ "plugin": plugin, "manifest": manifest_json(manifest) })
        })
        .collect();
    (
        [(header::CONTENT_TYPE, json())],
        serde_json::json!({ "manifests": rows }).to_string(),
    )
        .into_response()
}

/// The workspace's `(epoch, head_seq)`, or the 404 §5.2 allows after authorization said yes.
pub(crate) async fn head_of(
    store: &graph_store::Store,
    ws: &str,
) -> Result<(u64, u64), HubApiError> {
    let mut client = store.client().await.map_err(|e| write_fault(&e))?;
    graph_store::epoch::head_of(&mut client, ws)
        .await
        .map_err(|error| write_fault(&error))?
        .ok_or_else(|| HubApiError::NotFound(String::from("no such workspace")))
}

/// The body text a reader handed back, lossily: a manifest is UTF-8 by §4 and `read_manifest`
/// refuses anything else with its own error, so the lossy conversion never changes a legal body.
fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The `application/json` every answer of this module carries.
fn json() -> HeaderValue {
    HeaderValue::from_static("application/json")
}
