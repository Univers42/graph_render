//! `POST …/batches`, the records page and the one-record route: the three reads and the one write
//! that hang off a plugin.
//!
//! `Idempotency-Key` and `If-Match` are read from the headers and **passed through**: the hub never
//! interprets either, and the store is what enforces §5.1's 128-byte cap, its one-replay rule and
//! its 412.

use axum::body::Body;
use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use std::collections::BTreeMap;
use std::sync::Arc;

use graph_contract::hub::batch::read_batch;
use graph_contract::hub::{Cursor, qualify};
use graph_store::records::{RecordsReq, page as records_page};
use graph_store::writer::{BatchWrite, Idempotency};

use crate::app::App;
use crate::auth::Credential;
use crate::error::HubApiError;
use crate::routes::{limits, plugins::head_of, scan, write_fault};

/// `POST /v1/workspaces/{ws}/plugins/{plugin}/batches`: `answer_json` and a `Graph-Seq` header.
///
/// Two permits: `WRITERS` for the store and `WRITERS_PER_KEY` for this key, in that order, so a key
/// that already has a batch in flight waits before it takes a slot a different key could have used.
pub async fn post(
    State(app): State<Arc<App>>,
    Extension(credential): Extension<Credential>,
    Path((ws, plugin)): Path<(String, String)>,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, HubApiError> {
    let permit = crate::gate::admit(&app, &app.gates.writers).await?;
    let per_key = crate::gate::admit_key(&app, &credential.key).await?;
    crate::hooks::pause_after_admit(&app.hooks, "post-batch").await;
    let raw = crate::body::read(
        body,
        &headers,
        app.settings.limits.max_body,
        app.settings.limits.body_timeout,
    )
    .await?;
    let store = app.store().await?;
    let read_limits = limits(&app);
    let manifest = scan::manifest(store, &ws, &plugin, head_of(store, &ws).await?)
        .await
        .map_err(|error| write_fault(&error))?
        .ok_or(HubApiError::NotFound("no such plugin"))?;
    let batch = read_batch(&lossy(&raw), &read_limits).map_err(|e| crate::routes::hub_fault(&e))?;
    let write = BatchWrite {
        ws: ws.clone(),
        plugin: plugin.clone(),
        manifest,
        batch,
        idem: idempotency(&headers, &raw),
        if_match: if_match(&headers)?,
        limits: read_limits,
    };
    let outcome = store
        .apply_batch(&write)
        .await
        .map_err(|error| write_fault(&error))?;
    crate::hooks::before_ack(&app.hooks, outcome.seq);
    if outcome.applied > 0 {
        publish(&app, &ws, outcome.seq);
    }
    drop((permit, per_key));
    Ok(answer(outcome.seq, outcome.applied, &outcome.response, store, &ws).await)
}

/// `GET …/plugins/{plugin}/records?cursor=&limit=`: one page in byte order, with `plugin_seq` on
/// every page and an opaque `next` until the last one.
pub async fn page(
    State(app): State<Arc<App>>,
    Extension(_credential): Extension<Credential>,
    Path((ws, plugin)): Path<(String, String)>,
    uri: Uri,
) -> Result<Response, HubApiError> {
    let permit = crate::gate::admit(&app, &app.gates.readers).await?;
    crate::hooks::pause_after_admit(&app.hooks, "records-page").await;
    let store = app.store().await?;
    head_of(store, &ws).await?;
    let query = query_of(&uri);
    let request = RecordsReq {
        ws: ws.clone(),
        plugin: plugin.clone(),
        after: cursor_pair(&query)?,
        limit: limit(&query)?,
    };
    let found = records_page(store, &request)
        .await
        .map_err(|error| write_fault(&error))?;
    drop(permit);
    let rows: Vec<serde_json::Value> = found
        .rows
        .iter()
        .map(|(collection, id, rev)| {
            serde_json::json!({ "collection": collection, "id": id, "rev": rev })
        })
        .collect();
    let body = serde_json::json!({
        "plugin_seq": found.plugin_seq.to_string(),
        "records": rows,
        "next": found.next.as_ref().map(|(c, i)| format!("{c}\u{1f}{i}")),
    });
    Ok(json(body))
}

/// `GET …/records/{plugin}/{collection}/{id}`: the record's `values` and its `rev`, read from the
/// store's own canonical text.
pub async fn one(
    State(app): State<Arc<App>>,
    Extension(_credential): Extension<Credential>,
    Path((ws, plugin, collection, id)): Path<(String, String, String, String)>,
) -> Result<Response, HubApiError> {
    let permit = crate::gate::admit(&app, &app.gates.readers).await?;
    crate::hooks::pause_after_admit(&app.hooks, "one-record").await;
    let store = app.store().await?;
    let head = head_of(store, &ws).await?;
    let found = scan::record(store, &ws, &qualify(&plugin, &collection), &id, head)
        .await
        .map_err(|error| write_fault(&error))?;
    drop(permit);
    let Some((rev, text)) = found else {
        return Err(HubApiError::NotFound(String::from("no such record")));
    };
    Ok(json(record_body(&collection, &id, rev, &text)))
}

/// The answer of one record: its coordinate, the store's `rev`, and the `values` member lifted out
/// of the store's own canonical text.
///
/// Caveat: `values` is taken verbatim from the text the store wrote, so it is graph-contract's
/// canonical spelling and the hub adds no member, renames none and drops none.
fn record_body(collection: &str, id: &str, rev: u64, text: &str) -> serde_json::Value {
    let stored: serde_json::Value = serde_json::from_str(text).unwrap_or(serde_json::Value::Null);
    let values = stored
        .get("values")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    serde_json::json!({
        "collection": collection,
        "id": id,
        "rev": rev,
        "values": values,
    })
}

/// The batch's answer: graph-contract's `answer_json` body, under 200, with `Graph-Seq`.
///
/// Caveat: the header is read back from the workspace row rather than from `BatchOutcome`, which
/// carries the seq and not the epoch. One extra statement, and the epoch is the store's own number.
async fn answer(seq: u64, applied: u64, response: &str, store: &graph_store::Store, ws: &str) -> Response {
    let _ = applied;
    let (epoch, _) = head_of(store, ws).await.unwrap_or((0, 0));
    let value = HeaderValue::from_str(&Cursor { epoch, seq }.to_string())
        .unwrap_or_else(|_| HeaderValue::from_static("0.0"));
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("application/json")),
            (HeaderName::from_static("graph-seq"), value),
        ],
        response.to_owned(),
    )
        .into_response()
}

/// Raises the workspace's watch after the store's step 8 returned, which is §5.1's order: only then
/// is the response sent and the watch updated.
fn publish(app: &Arc<App>, ws: &str, seq: u64) {
    let store = app.store();
    let ws_owned = ws.to_owned();
    let app = Arc::clone(app);
    // The position is read through the store's own row read, off the accept path: the answer is
    // already on its way and a subscriber must not wait for this statement.
    tokio::spawn(async move {
        if let Ok(store) = store.await {
            if let Ok((epoch, _)) = head_of(&store, &ws_owned).await {
                app.watch.publish(&ws_owned, epoch, seq);
                app.watch.prune();
            }
        }
    });
}

/// `Idempotency-Key` and the SHA-256 of the body it claims, or `None` when the header is absent.
///
/// Caveat: the hash is over the **raw bytes** the caller sent, not over the re-serialized batch, so
/// a client that sends the same operations in a different member order is told its key does not
/// match its body. That is §5.1's rule: the key claims one body, and the bytes are the body.
fn idempotency(headers: &HeaderMap, raw: &[u8]) -> Option<Idempotency> {
    let key = headers.get("idempotency-key")?.to_str().ok()?.trim();
    if key.is_empty() {
        return None;
    }
    Some(Idempotency {
        key: key.to_owned(),
        body_sha256: graph_store::writer::idempotency::sha256(raw),
    })
}

/// `If-Match` as a cursor, or `None` when the header is absent.
///
/// A malformed `If-Match` is a 400 and not an absent header: §5.3 makes a cursor `<epoch>.<seq>`
/// and a bare seq is 400 (`HubError::Cursor` → 400).
fn if_match(headers: &HeaderMap) -> Result<Option<Cursor>, HubApiError> {
    let Some(value) = headers.get("if-match") else {
        return Ok(None);
    };
    let text = value
        .to_str()
        .map_err(|_| HubApiError::BadRequest("If-Match is not text"))?
        .trim()
        .trim_matches('"');
    if text.is_empty() {
        return Ok(None);
    }
    Cursor::parse(text)
        .map(Some)
        .map_err(|_| HubApiError::BadRequest("If-Match is not <epoch>.<seq>"))
}

/// The request's own query string as a map, percent-decoded.
///
/// Caveat: a repeated parameter keeps the **first** value, because the records page reads one
/// `cursor` and one `limit` and a caller that sent two sent a request the page cannot honour.
/// `axum::extract::Query` would have needed a feature the hub's edge deliberately does not carry
/// (plan fact 1), so this is the whole of the query reader the hub has.
fn query_of(uri: &Uri) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for pair in uri.query().unwrap_or("").split('&').filter(|p| !p.is_empty()) {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        let name = decode(name);
        if out.contains_key(&name) {
            continue;
        }
        out.insert(name, decode(value));
    }
    out
}

/// One percent-decoded query token; an undecodable one is left as it arrived, because the only two
/// parameters the page reads are then refused as malformed rather than silently dropped.
fn decode(text: &str) -> String {
    let replaced = text.replace('+', " ");
    percent_encoding::percent_decode_str(&replaced)
        .decode_utf8_lossy()
        .into_owned()
}

/// The `cursor` query parameter as the records page's `(collection, id)` pair.
///
/// Caveat: the page's cursor is **opaque** on the wire and is the pair joined by `\u{1f}`, which no
/// collection or record id can hold (`check_collection_id` and `check_record_id` both refuse it).
/// The SDK passes it back without reading it, which is what "opaque" means.
fn cursor_pair(query: &BTreeMap<String, String>) -> Result<Option<(String, String)>, HubApiError> {
    let Some(cursor) = query.get("cursor") else {
        return Ok(None);
    };
    if cursor.is_empty() {
        return Ok(None);
    }
    let (collection, id) = cursor
        .split_once('\u{1f}')
        .ok_or(HubApiError::BadRequest("the cursor is not this page's own"))?;
    Ok(Some((collection.to_owned(), id.to_owned())))
}

/// The `limit` query parameter: default 1 000, at most 10 000, and never zero.
fn limit(query: &BTreeMap<String, String>) -> Result<u64, HubApiError> {
    const DEFAULT_LIMIT: u64 = 1_000;
    const MAX_LIMIT: u64 = 10_000;
    let Some(raw) = query.get("limit") else {
        return Ok(crate::config::capped(DEFAULT_LIMIT));
    };
    let asked: u64 = raw
        .parse()
        .map_err(|_| HubApiError::BadRequest("limit is not a number"))?;
    if asked == 0 {
        return Err(HubApiError::BadRequest("limit is at least 1"));
    }
    Ok(crate::config::capped(asked.min(MAX_LIMIT)))
}

/// A JSON answer under 200.
fn json(body: serde_json::Value) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, HeaderValue::from_static("application/json"))],
        body.to_string(),
    )
        .into_response()
}

/// The body text a reader handed back. See `routes::plugins` for why this cannot change a legal
/// body: §4 makes a batch UTF-8 and `read_batch` refuses anything else with its own error.
fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
