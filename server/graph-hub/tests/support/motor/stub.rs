//! The scripted stand-in for graph-server: one answer per request, in order.
//!
//! `hub-motor-map` needs a peer whose failures are the shape it is testing — a 503, a 400 with the
//! motor's own error class, a body that never ends — and the real motor cannot be asked to fail on
//! demand. So this is a router over a list of [`StubReply`], which is the smallest thing that answers
//! every case row `hub-motor-map` names.

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use http_body_util::BodyExt;
use std::sync::{Arc, Mutex};

use super::{Motor, serve};

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
    let script = Arc::new(Mutex::new(Script::new(answers)));
    let router = Router::new()
        .route("/v1/layout", post(scripted))
        .fallback(not_found)
        .method_not_allowed_fallback(not_found)
        .with_state(script);
    serve(router, |status| status == StatusCode::NOT_FOUND).await
}

/// The next scripted answer, once the request body has been drained.
async fn scripted(State(script): State<Arc<Mutex<Script>>>, body: Body) -> Response {
    discard(body).await;
    let reply = script.lock().expect("the stub's script lock").take();
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
            vec![StubReply::new(
                404,
                "NotFound",
                "the stub's script is empty",
            )]
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
