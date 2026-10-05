//! The `/layout` relay: the hub's own materialized document streamed into graph-server's request
//! body, and graph-server's answer streamed back to the caller under §5.2's table.
//!
//! H1 in one sentence: the hub never runs motor math, so `/layout` is a relay and not a
//! computation. The hub owns the request, the credential (`GRAPH_HUB_MOTOR_KEY_FILE`), the permit
//! (`GRAPH_HUB_LAYOUTS`), the stream deadline and the status; graph-server owns every byte of the
//! answer. `source=contract` is fixed here and is not a parameter, because the document the hub
//! streams *is* the ingest contract document (`graph_contract::hub::Model::to_json` writes
//! `ingest::DOC_HEAD`), and H1 says the hub asks for the contract layout.
//!
//! Order inside a relay, and it is §5.2's order of refusals: **admit the permit (or 503), then
//! open the snapshot (404), then call the motor**. Authorization is not in the list because it is
//! not here: [`crate::auth::authorize_middleware`] decides it before any of this runs.
//!
//! Caveat: a `LAYOUTS` permit is held across the upload, the motor's answer head **and** the
//! caller's slow read of that answer ([`body::held`]). §6's default of 1 is what bounds a waiting
//! `/layout` to one snapshot, one xmin horizon and one pool connection, and `GRAPH_HUB_LAYOUTS` is
//! the knob for a deployment that has connections to spare.

pub mod answer;
pub mod body;
pub mod map;

use axum::body::Body;
use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, Uri, header};
use axum::response::Response;
use graph_contract::hub::Cursor;
use graph_store::materialize::Document;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use std::sync::Arc;

use crate::app::App;
use crate::auth::Credential;
use crate::error::{HubApiError, MotorFault};
use crate::relay::body::Probe;

/// What one `/layout` call asks of the motor. `layout` and `post` are `None` when the caller sent
/// no such query parameter, and the motor decides what a missing `layout` means (a relayed 400).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayReq {
    /// The workspace whose document is streamed.
    pub ws: String,
    /// The motor's `layout=` id, passed through.
    pub layout: Option<String>,
    /// The motor's `post=` id, passed through.
    pub post: Option<String>,
    /// The caller's own `Accept`, passed through so the motor picks the face the caller asked for.
    pub accept: Option<String>,
    /// The cursor of the snapshot, which is what `Graph-Seq` carries.
    pub cursor: Cursor,
}

/// What the motor answered, ready to become the hub's response. `status` is 200 and only 200: every
/// other status became a [`HubApiError`] before this value was built.
#[derive(Debug)]
pub struct RelayAnswer {
    /// graph-server's status line.
    pub status: u16,
    /// Its `Content-Type`, relayed.
    pub content_type: Option<String>,
    /// Its `Vary`, relayed.
    pub vary: Option<String>,
    /// Its body, streamed.
    pub body: Body,
}

/// `POST /v1/workspaces/{ws}/layout`: §5.2's relay route.
///
/// The `LAYOUTS` permit is not a `READS` one (§5.3), and it is admitted before the snapshot is
/// opened so a key that may not lay out cannot make the hub read a document.
pub async fn layout(
    State(app): State<Arc<App>>,
    Extension(_credential): Extension<Credential>,
    Path(ws): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Response, HubApiError> {
    let permit = crate::gate::admit(&app, &app.gates.layouts).await?;
    crate::hooks::pause_after_admit(&app.hooks, "layout").await;
    let store = app.store().await?;
    let document = graph_store::materialize::open(store, &ws)
        .await
        .map_err(|error| crate::routes::write_fault(&error))?;
    let request = RelayReq {
        cursor: document.cursor(),
        ws: ws.clone(),
        layout: parameter(&uri, "layout"),
        post: parameter(&uri, "post"),
        accept: header_text(&headers, header::ACCEPT),
    };
    let answer = post(&app, &request, document, None).await?;
    Ok(answer::respond(answer, permit, &request.cursor))
}

/// One `/layout` exchange: build the motor's request, send `document` as its body, and read the
/// answer through [`answer::read`].
///
/// `probe` is [`body::Probe`] or `None`; it is how `layout_never_holds_a_whole_document` counts the
/// chunks the relay actually wrote without reading RSS.
pub async fn post(
    app: &Arc<App>,
    request: &RelayReq,
    document: Document,
    probe: Option<Arc<Probe>>,
) -> Result<RelayAnswer, HubApiError> {
    let deadline = tokio::time::Instant::now() + app.settings.limits.stream_deadline;
    let sent = send(
        app,
        request,
        Body::from_stream(body::document(document, probe, deadline)),
    )
    .await;
    let response = match sent {
        Ok(response) => response,
        Err(fault) => {
            app.log(&serde_json::json!({
                "event": "motor-unreachable",
                "ws": request.ws,
                "fault": fault.code(),
            }));
            return Err(HubApiError::Motor(fault));
        }
    };
    let (parts, incoming) = response.into_parts();
    match answer::read(parts, incoming, deadline).await {
        Ok(answer) => Ok(answer),
        Err(error) => {
            if map::defect(&error) {
                app.log(&serde_json::json!({
                    "event": "motor-fault",
                    "ws": request.ws,
                    "status": error.status(),
                    "fault": error.code(),
                }));
            }
            Err(error)
        }
    }
}

/// One motor request, sent under `GRAPH_HUB_MOTOR_TIMEOUT_MS`.
///
/// The timeout wraps the whole exchange including the upload, which is what makes a motor that
/// never answers a 502 `MotorUnavailable` with the snapshot already closed: the `Document` went
/// out of scope when the body stream ended, and this is waiting on a socket, not on a store.
///
/// Caveat: a client is built per relay rather than held on the `App`. `GRAPH_HUB_LAYOUTS` defaults
/// to 1, so one client serves one concurrent upload and a pooled client would hold connections the
/// hub has no other use for; the cost is a fresh TCP connection per `/layout`, on loopback or on one
/// host's network, which §6's table treats as free.
async fn send(
    app: &Arc<App>,
    request: &RelayReq,
    body: Body,
) -> Result<hyper::Response<hyper::body::Incoming>, MotorFault> {
    let url = motor_uri(app, request);
    let sent = hyper::Request::builder()
        .method("POST")
        .uri(&url)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, bearer(app)?)
        .body(body);
    let Ok(sent) = sent else {
        return Err(MotorFault::Unavailable);
    };
    let client: Client<HttpConnector, Body> = Client::builder(TokioExecutor::new()).build_http();
    let answered = tokio::time::timeout(app.settings.motor_timeout, client.request(sent)).await;
    match answered {
        Ok(Ok(response)) => Ok(response),
        // A refused connection, a reset mid-upload and the timeout itself are one answer here:
        // §5.2's last row is "unreachable, or no answer in GRAPH_HUB_MOTOR_TIMEOUT_MS", and the
        // hub cannot tell those apart without a fact it does not have.
        Ok(Err(_)) | Err(_) => Err(MotorFault::Unavailable),
    }
}

/// The motor's own URL for this relay: `POST {GRAPH_HUB_MOTOR_URL}/v1/layout?layout=&post=&source=contract`.
///
/// `source=contract` is fixed and appended, so a caller cannot send a studio ingest document's
/// source to the motor and get a topology the hub never built. An unknown query parameter is a 400
/// at the motor (graph-server's own reader), which §5.2's table relays.
fn motor_uri(app: &Arc<App>, request: &RelayReq) -> String {
    let mut uri = format!("{}/v1/layout", app.settings.motor_url.trim_end_matches('/'));
    if let Some(layout) = &request.layout {
        uri.push_str(&format!("?layout={}", encode(layout)));
        if let Some(post) = &request.post {
            uri.push_str(&format!("&post={}", encode(post)));
        }
    }
    uri.push_str(if request.layout.is_some() {
        "&source=contract"
    } else {
        "?source=contract"
    });
    uri
}

/// The motor's bearer credential: the first line of `GRAPH_HUB_MOTOR_KEY_FILE` (Decision 6).
///
/// A graph-server keys file stores `name <sha256-hex>` and nothing else, so the secret cannot be
/// recovered from one; the hub is given the plaintext on its own file.
///
/// Caveat: the line is read on every relay rather than cached at start, because §5.3 gives this
/// slice no reload path for the motor credential and a rotated key should take effect without a
/// restart. The read is a few hundred bytes on a path that is already writing a whole document.
fn bearer(app: &Arc<App>) -> Result<String, MotorFault> {
    let path = &app.settings.motor_key_file;
    let text = std::fs::read_to_string(path).map_err(|_| MotorFault::Unavailable)?;
    let key = text.lines().next().unwrap_or_default().trim();
    if key.is_empty() || key.len() > MAX_KEY_BYTES || key.contains('\0') {
        return Err(MotorFault::Unavailable);
    }
    Ok(format!("Bearer {key}"))
}

/// The longest motor key line the relay will send, in bytes.
///
/// Caveat: a bound and not a check against graph-server's own, which imposes none: it exists so a
/// file holding a whole document is refused at the first relay rather than written into a header.
const MAX_KEY_BYTES: usize = 256;

/// One query parameter, percent-decoded, or `None` when the caller sent none.
///
/// `axum::extract::Query` would need a `serde` derive feature the hub's edge deliberately does not
/// carry (plan fact 1), so this reads the one parameter it needs the way `routes/batches.rs` reads
/// `limit`.
///
/// Caveat: a repeated parameter keeps the **first** value, exactly as the records page does, and an
/// undecodable one is passed through as it arrived — which the motor then refuses with a relayed
/// 400 rather than the hub guessing at it.
fn parameter(uri: &Uri, name: &str) -> Option<String> {
    let pair = uri
        .query()?
        .split('&')
        .filter(|pair| !pair.is_empty())
        .find(|pair| pair.split_once('=').map(|(key, _)| key) == Some(name))?;
    let value = pair.split_once('=').map(|(_, value)| value).unwrap_or("");
    Some(decode(value))
}

/// One percent-decoded query value; an undecodable one is left as it arrived.
fn decode(text: &str) -> String {
    let replaced = text.replace('+', " ");
    percent_encoding::percent_decode_str(&replaced)
        .decode_utf8_lossy()
        .into_owned()
}

/// The characters a relayed query value is percent-encoded over.
///
/// `NON_ALPHANUMERIC` and not a hand-picked set, because a layout or post id is a registry name
/// this crate does not own and a new one may carry a character nobody thought of. Over-encoding
/// costs the motor a percent-decode it already does.
fn encode(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC).to_string()
}

/// One header as text, `None` when it is absent or is not text.
fn header_text(headers: &HeaderMap, name: header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}
