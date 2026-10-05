//! The scripted motor: a hand-written router that answers what the case told it to, and counts what
//! it was asked.
//!
//! [`Motor`] is the fixture both halves share; everything here is the stub's own script. It exists
//! because §5.2's map has thirteen rows and the motor that produces them is graph-server itself: a
//! case that had to make the real motor answer 401, or a 422 with a chosen `error`, or nothing at
//! all, would be testing graph-server rather than the hub's table.
//!
//! Caveat: a stub cannot fail the way graph-server fails — it never resets a connection mid-upload
//! and never queues — so what `hub-motor-map` proves is the mapping, and `hub-roundtrip` is what
//! proves the bytes.

use super::{Motor, serve};
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use http_body_util::BodyExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// One scripted answer for [`stub`]. `body` wins when it is not empty; otherwise the
/// `{"error","message"}` shape is written. Caveat: `content-type` is `application/json` either
/// way, so a verbatim body is expected to be JSON text.
#[derive(Debug, Clone, PartialEq, Eq)]
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
            error: error.into(),
            message: message.into(),
            body: String::new(),
        }
    }

    /// A verbatim body, for the arms §5.2 relays unchanged.
    pub fn with_body(status: u16, body: &str) -> Self {
        Self {
            status,
            error: String::new(),
            message: String::new(),
            body: body.into(),
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
    fn into_reply(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, [("content-type", "application/json")], self.text()).into_response()
    }
}

/// A hand-written router, no graph-server code at all: `POST /v1/layout` answers with `answers`
/// in order and then with the last of them, and every other method and path is the JSON 404
/// `NotFound`. The request body is drained rather than parsed, so a peer's streamed upload is
/// read to its end and cannot deadlock the stub.
pub async fn stub(answers: Vec<StubReply>) -> Motor {
    stub_parts(answers, Duration::ZERO).await.0
}

/// [`stub`] and the key a hub under test presents to it, for the fixtures whose hub needs both.
pub async fn stub_with_key(answers: Vec<StubReply>) -> (Motor, String) {
    let (motor, _, key) = stub_parts(answers, Duration::ZERO).await;
    (motor, key)
}

/// [`stub`] plus the number of requests it served, for the cases whose subject is a **count** rather
/// than a status: `a_layout_is_never_retried` needs "exactly one request", which no status code can
/// say.
///
/// Caveat: the counter is incremented after the body is drained and before the answer is written, so
/// it counts requests that reached the stub and not requests whose answer was read. That is the
/// right direction for "the hub did not retry": a second request would be counted even if the hub
/// had given up on reading the first answer.
pub async fn stub_counting(answers: Vec<StubReply>) -> (Motor, Arc<AtomicU64>) {
    let (motor, served, _) = stub_parts(answers, Duration::ZERO).await;
    (motor, served)
}

/// [`stub`] that holds each answer for `hold` after the request body is drained, so a test can look
/// at the hub while the motor still has the request and has not answered.
///
/// This is the fixture seam `the_snapshot_closes_before_the_motor_answer_is_awaited` needs: the real
/// motor's own `before_run` hook is behind graph-server's `test-hooks` feature, and the hub's
/// dev-dependency on graph-server does not forward it (Decision 1 of the plan). A delay here is
/// observable from the outside and needs no feature on either crate.
///
/// Caveat: a wall-clock sleep on the stub's own task, so it holds the connection rather than the
/// accept loop and a second request is still served; it is a fixture delay and never a bound the hub
/// reads.
pub async fn stub_after(answers: Vec<StubReply>, hold: Duration) -> (Motor, String) {
    let (motor, _, key) = stub_parts(answers, hold).await;
    (motor, key)
}

/// The stub itself, plus its served counter and the key a hub under test presents to it.
///
/// [`stub`], [`stub_counting`] and [`stub_after`] are this one function with three shapes taken off
/// it, so a fixture that needs the counter and a hold has one way to ask for both.
pub async fn stub_parts(
    answers: Vec<StubReply>,
    hold: Duration,
) -> (Motor, Arc<AtomicU64>, String) {
    let served = Arc::new(AtomicU64::new(0));
    let script = Arc::new(Mutex::new(Script::new(answers, Arc::clone(&served))));
    let router = Router::new()
        .route("/v1/layout", post(scripted))
        .fallback(not_found)
        .method_not_allowed_fallback(not_found)
        .with_state(Served { script, hold });
    let motor = serve(router, |status| status == StatusCode::NOT_FOUND).await;
    (motor, served, String::from(STUB_KEY))
}

/// The key a stub fixture's hub presents. The stub checks no credential, so this exists only so a
/// fixture has the shape the relay's own reader insists on: one non-empty line of at most 256 bytes
/// and no NUL.
pub const STUB_KEY: &str = "a-key-the-stub-does-not-check";

/// The stub's state: the script, and how long each answer is held after the body is drained.
#[derive(Clone)]
struct Served {
    script: Arc<Mutex<Script>>,
    hold: Duration,
}

/// The next scripted answer, once the request body has been drained and the hold has passed.
async fn scripted(State(served): State<Served>, body: Body) -> Response {
    discard(body).await;
    if !served.hold.is_zero() {
        tokio::time::sleep(served.hold).await;
    }
    let reply = served.script.lock().expect("the stub's script lock").take();
    reply.into_reply()
}

/// The stub's own 404, in the same `{"error","message"}` shape every other refusal uses.
async fn not_found() -> Response {
    StubReply::new(404, "NotFound", "the stub has no such method and path").into_reply()
}

/// Reads the request body to its end and drops every frame. A peer streaming an upload blocks in
/// `write` if nobody reads it, so draining is what keeps a scripted answer prompt.
async fn discard(body: Body) {
    let mut body = body;
    while let Some(Ok(_frame)) = body.frame().await {}
}

/// The answers, in order, and how many of them have been handed out.
struct Script {
    answers: Vec<StubReply>,
    taken: usize,
    /// The shared counter [`stub_counting`] hands back, so the count a test reads is the count the
    /// requests incremented rather than a second number this struct keeps in step.
    served: Arc<AtomicU64>,
}

impl Script {
    /// A script over `answers`, or over one JSON 404 when the list is empty, so an empty script
    /// is a stub that refuses everything rather than a stub that panics.
    fn new(answers: Vec<StubReply>, served: Arc<AtomicU64>) -> Self {
        let answers = if answers.is_empty() {
            vec![StubReply::new(
                404,
                "NotFound",
                "the stub's script is empty",
            )]
        } else {
            answers
        };
        Self {
            answers,
            taken: 0,
            served: Arc::clone(&served),
        }
    }

    /// The next answer: the script in order, then its last entry for every request past the end.
    /// Caveat: a script shorter than the requests a test makes is read as the motor repeating its
    /// last answer, not as a fault, so a test that wants exactly one answer sends one request.
    fn take(&mut self) -> StubReply {
        let index = self.taken.min(self.answers.len() - 1);
        self.taken += 1;
        self.served.fetch_add(1, Ordering::Relaxed);
        self.answers[index].clone()
    }
}
