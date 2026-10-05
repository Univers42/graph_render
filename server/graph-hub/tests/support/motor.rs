//! Two fixtures for the hub's tests: the real motor's router in process, and a scripted stub.
//!
//! Decision 1 of `docs/superpowers/plans/2026-10-05-graph-hub-api.md`: the hub's tests never
//! spawn the built `graph-server` binary, because cargo defines `CARGO_BIN_EXE_graph-server`
//! only for test targets of the crate that owns it and `scripts/service.sh` builds it in another
//! image step. So the motor is served in process on an ephemeral loopback port, and the
//! conventions of `server/graph-server/tests/common/child.rs` are reused verbatim rather than its
//! code: loopback only, the port learned from the listener, a bounded readiness wait
//! ([`PATIENCE`]) and a `Drop` that stops the server.
//!
//! [`real`] speaks the motor's own wire, so `hub-roundtrip` can compare its bytes against the
//! hub's; [`stub`] is a hand-written router over a scripted answer list, for `hub-motor-map`.
//! Caveat: the in-process motor shares the test binary's event loop and its container, so a
//! `/layout` timing measured here is a floor and never the production figure.
#![allow(dead_code, reason = "each test binary uses a part of these fixtures")]

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use graph_server::app::{App, LogSink};
use graph_server::config::Settings;
use graph_server::keys;
use http_body_util::{BodyExt, Empty};
use hyper::Error as HyperError;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::Notify;

/// The bound on a fixture's readiness wait, `child.rs:18`'s value.
/// Caveat: a guess above a cold in-process server, not a measurement, so a host slower than this
/// reads as a failure rather than a hang; a peer that never becomes ready makes the wait out its
/// bound and the fixture panics.
pub const PATIENCE: Duration = Duration::from_secs(10);

/// The pause between readiness probes. Caveat: a poll interval, so it is the granularity of the
/// readiness wait and nothing else.
const PROBE_GAP: Duration = Duration::from_millis(10);

/// One running motor: the real router in process ([`real`]) or the scripted one ([`stub`]).
/// Dropping it stops the accept task, so neither a listener nor a task outlives the fixture.
pub struct Motor {
    addr: SocketAddr,
    task: tokio::task::JoinHandle<()>,
    stop: Arc<Notify>,
}

impl Motor {
    /// The address to dial: the loopback address and the port the kernel gave the listener,
    /// learned from the listener itself (`child.rs:99-101` reads the same fact out of a line).
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// `http://127.0.0.1:<port><path>`, the URL a hub's `GRAPH_HUB_MOTOR_URL` would carry.
    pub fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }
}

/// `Drop` cannot await, so it signals the accept task instead of joining it: the `Notify` is what
/// `with_graceful_shutdown` waits on, and the abort is the backstop for a task still inside a
/// connection when the fixture goes. Either way nothing is left listening.
impl Drop for Motor {
    fn drop(&mut self) {
        self.stop.notify_one();
        self.task.abort();
    }
}

/// The real motor in process: graph-server's own `Settings`, key store, `App` and router, served
/// by `axum::serve` on `127.0.0.1:0`, with its log lines in an in-memory buffer. Caveat: this
/// panics rather than returning a half-built fixture, which is the right shape for a fixture: a
/// motor that answered on a later failure would blame the wrong line, and the message always
/// names [`PATIENCE`].
pub async fn real() -> Motor {
    let (router, _key) = real_parts().await;
    serve(router, |status| status == StatusCode::OK).await
}

/// [`real`] plus the minted key, for the tests that send `Authorization: Bearer <key>`. Same
/// panic shape: never a fixture that works without its credential.
pub async fn real_with_key() -> (Motor, String) {
    let (router, key) = real_parts().await;
    (serve(router, |status| status == StatusCode::OK).await, key)
}

/// The motor's router and its minted key. `GRAPH_BIND` is loopback and `GRAPH_PORT` is 0, as the
/// binary's own fixtures set them, and the port is not this function's to learn: it hands over
/// the router and [`serve`] binds the listener.
async fn real_parts() -> (Router, String) {
    let minted = keys::keygen("tester").expect("a key from /dev/urandom");
    let keys_file = keys_path();
    std::fs::write(&keys_file, format!("{}\n", minted.line)).expect("the fixture's key file");
    // 0640, not what the umask gave it: `KeySet::load` refuses anything wider, so without this
    // every real fixture would refuse to build its state.
    let readable = std::fs::Permissions::from_mode(0o640);
    std::fs::set_permissions(&keys_file, readable).expect("the fixture's key file mode");
    let mut vars: BTreeMap<String, String> = BTreeMap::new();
    vars.insert("GRAPH_BIND".into(), "127.0.0.1".into());
    vars.insert("GRAPH_PORT".into(), "0".into());
    vars.insert("GRAPH_WORKERS".into(), "1".into());
    let path = keys_file.display().to_string();
    vars.insert("GRAPH_API_KEYS_FILE".into(), path);
    let lookup = |name: &str| vars.get(name).map(Into::into);
    let settings = Settings::from_env(&lookup).expect("the fixture's settings");
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&lines);
    let sink: LogSink = Arc::new(move |line: &str| {
        sink.lock()
            .expect("the fixture's log lock")
            .push(line.to_owned());
    });
    let app = App::from_settings(&settings, sink).expect("the fixture's state");
    (graph_server::router(Arc::new(app)), minted.key)
}

/// A key-file path of this fixture's own under cargo's test scratch root. Caveat: the name is a
/// pid and a counter and a pid is reused, so an earlier run's file is removed first.
fn keys_path() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = format!(
        "motor-keys-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_file(&path);
    path
}

/// Binds `127.0.0.1:0`, spawns the accept task and waits until the server answers `GET /healthz`
/// with a status `ready` accepts. The returned fixture is the only thing a test ever sees.
async fn serve(router: Router, ready: fn(StatusCode) -> bool) -> Motor {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("the fixture's listener binds");
    let addr = listener.local_addr().expect("the fixture's listener address");
    let stop = Arc::new(Notify::new());
    let task = tokio::spawn(accept(listener, router, Arc::clone(&stop)));
    let motor = Motor { addr, task, stop };
    await_ready(&motor, ready).await;
    motor
}

/// The accept loop, stopped by a signalled `Notify` through `with_graceful_shutdown`, so the
/// listener goes first and an in-flight request is left to finish. The error is dropped: a
/// fixture that served a test has nowhere to report it that a test would read.
async fn accept(listener: TcpListener, router: Router, stop: Arc<Notify>) {
    let signal = async move { stop.notified().await };
    let _served = axum::serve(listener, router).with_graceful_shutdown(signal).await;
}

/// Polls `GET /healthz` until `ready` accepts the status or [`PATIENCE`] is out. This is the
/// fixture's own readiness, so the panic names the bound instead of surfacing as some later
/// assertion's failure.
async fn await_ready(motor: &Motor, ready: fn(StatusCode) -> bool) {
    let deadline = Instant::now() + PATIENCE;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            panic!("the fixture's server did not answer GET /healthz within {PATIENCE:?}");
        }
        let answered = tokio::time::timeout(left, healthz(motor.addr)).await;
        if matches!(answered, Ok(Ok(status)) if ready(status)) {
            return;
        }
        tokio::time::sleep(PROBE_GAP).await;
    }
}

/// One `GET /healthz`, or the transport error: a refused connection is the state before the
/// accept task is scheduled, so it is expected on the first probe and not a failure.
async fn healthz(addr: SocketAddr) -> Result<StatusCode, HyperError> {
    let client: Client<Empty<Bytes>, Empty<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let uri = format!("http://{addr}/healthz")
        .parse::<hyper::Uri>()
        .expect("the fixture's probe URI");
    let request = Request::builder()
        .uri(uri)
        .body(Empty::new())
        .expect("the fixture's probe request");
    Ok(client.request(request).await?.status())
}

/// Readiness for the real motor: its own `/healthz` answers `200 ok` (`graph_server::router`).
fn ok(status: StatusCode) -> bool {
    status == StatusCode::OK
}

/// Readiness for the stub, whose every other path is its JSON 404: an answer is an answer.
fn missing(status: StatusCode) -> bool {
    status == StatusCode::NOT_FOUND
}

/// One scripted answer for [`stub`]. `body` wins when it is not empty; otherwise the
/// `{"error","message"}` shape is written. Caveat: `content-type` is `application/json` either
/// way, so a verbatim body is expected to be JSON text.
pub struct StubReply {
    /// The status line.
    pub status: u16,
    /// The body's `error` field, when `body` is empty.
    pub error: String,
    /// The body's `message` field, when `body` is empty.
    pub message: String,
    /// The whole body, verbatim, when it is not empty.
    pub body: String,
}

impl StubReply {
    /// The JSON error shape: `status` carrying `error` and `message`.
    pub fn new(status: u16, error: &str, message: &str) -> Self {
        Self {
            status,
            error: error.to_owned(),
            message: message.to_owned(),
            body: String::new(),
        }
    }

    /// A verbatim body, for the arms §5.2 relays unchanged.
    pub fn with_body(status: u16, body: &str) -> Self {
        Self {
            status,
            error: String::new(),
            message: String::new(),
            body: body.to_owned(),
        }
    }

    /// The bytes this answer writes: `body` when it is set, else the `{"error","message"}` shape.
    fn text(&self) -> String {
        if self.body.is_empty() {
            serde_json::json!({ "error": self.error, "message": self.message }).to_string()
        } else {
            self.body.clone()
        }
    }

    /// The response this answer becomes. A status outside 1xx-5xx cannot be written, so it
    /// becomes a 500: the fixture's script is wrong, and the answer must still be a response.
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, [("content-type", "application/json")], self.text()).into_response()
    }
}

/// A hand-written router, no graph-server code at all: `POST /v1/layout` answers with `answers`
/// in order and then with the last of them, and every other method and path is the JSON 404
/// `NotFound`. The request body is drained rather than parsed, so a peer's streamed upload is
/// read to its end and cannot deadlock the stub.
pub async fn stub(answers: Vec<StubReply>) -> Motor {
    let script = Arc::new(Mutex::new(Script::new(answers)));
    let router = Router::new()
        .route("/v1/layout", post(scripted))
        .fallback(not_found)
        .method_not_allowed_fallback(not_found)
        .with_state(script);
    serve(router, missing).await
}

/// The next scripted answer, once the request body has been drained.
async fn scripted(State(script): State<Arc<Mutex<Script>>>, body: Body) -> Response {
    discard(body).await;
    let reply = script.lock().expect("the stub's script lock").take();
    reply.into_response()
}

/// The stub's own 404, in the same `{"error","message"}` shape every other refusal uses.
async fn not_found() -> Response {
    StubReply::new(404, "NotFound", "the stub has no such method and path").into_response()
}

/// Reads the request body to its end and drops every frame. A peer streaming an upload blocks in
/// `write` if nobody reads it, so draining is what keeps a scripted answer prompt.
async fn discard(body: Body) {
    let mut body = body;
    while let Some(Ok(_frame)) = body.frame().await {}
}

/// The answers and how many requests the stub has served.
struct Script {
    answers: Vec<StubReply>,
    served: usize,
}

impl Script {
    /// A script over `answers`, or over one JSON 404 when the list is empty, so an empty script
    /// is a stub that refuses everything rather than a stub that panics.
    fn new(answers: Vec<StubReply>) -> Self {
        let answers = if answers.is_empty() {
            vec![StubReply::new(404, "NotFound", "the stub's script is empty")]
        } else {
            answers
        };
        Self { answers, served: 0 }
    }

    /// The next answer: the script in order, then its last entry for every request past the end.
    /// Caveat: a script shorter than the requests a test makes is read as the motor repeating its
    /// last answer, not as a fault, so a test that wants exactly one answer sends one request.
    fn take(&mut self) -> StubReply {
        let index = self.served.min(self.answers.len() - 1);
        self.served += 1;
        self.answers[index].clone()
    }
}