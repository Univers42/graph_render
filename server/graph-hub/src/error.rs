//! The one error enum every refusal of §5.2 becomes, and the two tables that turn a variant into
//! a status and a wire `error` string.
//!
//! The body is graph-server's shape, `{"error", "message"}` (`server/graph-server/src/error.rs:107-112`),
//! so a client parses one refusal the same way whichever service answered it. The `error` string
//! is graph-contract's wire class name for a fault that came from a `HubError`
//! (`crates/graph-contract/src/hub/schema/wire.rs:161-166`: `invalid`, `conflict`, `too_large`,
//! `cursor`, `internal`), the name the SDK already knows from the motor for a transport refusal
//! (`Unauthorized`, `NotFound`, `Busy`, `Timeout`), and §5.2's `Motor*` name for a motor fault.
//!
//! Both tables are `match`es over every variant with no wildcard, so a new variant cannot compile
//! without a status and a class.

mod motor;

pub use motor::MotorFault;

use std::borrow::Cow;

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

/// The longest message an error body carries, in bytes. A refusal can quote the caller's own text,
/// and without a bound a 64 MiB id would be echoed back whole.
/// Caveat: 256 bytes, truncated at a character boundary and marked with `...`, so a message past
/// it is a prefix and not the whole reason. 256 is a guess at one screen line, and the field that
/// matters — the `error` name — is never truncated.
pub(crate) const MAX_MESSAGE: usize = 256;

/// The hub's own error enum. One variant per reason §5.2 refuses for, so the status a variant
/// answers with is written down once and a handler never re-derives it.
#[derive(Debug)]
pub enum HubApiError {
    /// 400: the request itself is wrong (a bare seq as a cursor, a `%00` in a path id, a second
    /// `Authorization` header).
    BadRequest(&'static str),
    /// 401: no key, or a key that is not in the file. One variant, so the two are identical bytes.
    Unauthorized(&'static str),
    /// 403: a key with no grant covering this workspace and plugin. Never reveals existence.
    Forbidden(&'static str),
    /// 404: reached only after authorization said yes.
    NotFound(&'static str),
    /// 406: relayed from the motor; the message is graph-server's own.
    NotAcceptable(String),
    /// 408: the request body did not arrive within `GRAPH_HUB_BODY_TIMEOUT_MS`.
    BodyTimeout,
    /// 409: a manifest that would shrink or change. The message is `HubError`'s own.
    Conflict(String),
    /// 410: a cursor outside what is kept, or from another epoch.
    Gone(String),
    /// 413: over one of §6's caps, or a relayed 413 with the motor's message.
    TooLarge {
        /// What was over the limit, named in the message.
        what: &'static str,
        /// The limit it was over.
        limit: u64,
    },
    /// 422: a wire fault graph-contract refused. `HubError::status()` already said 422.
    Invalid {
        /// The path or coordinate at fault, `HubError`'s own.
        path: String,
        /// What is wrong with it, `HubError`'s own.
        what: String,
    },
    /// 429 on a subscriber cap, 503 on a permit or pool wait. Carries `Retry-After`.
    Busy {
        /// The `Retry-After` in seconds.
        retry_after: u64,
    },
    /// 502: the motor's fault, mapped by §5.2's table.
    Motor(MotorFault),
    /// 500: a hub defect. Logged whole, never sent whole.
    Internal(String),
}

impl HubApiError {
    /// The wire `error` string: a graph-contract class for a `HubError` fault, the motor's own
    /// name when the answer is relayed unchanged, §5.2's name for a motor fault.
    ///
    /// Caveat: this borrows rather than returning `&'static str`, because a relayed body keeps
    /// graph-server's `error` string whole (`docs/contract/hub-api.md` "Errors"); every other arm
    /// is a `&'static str` lifted into the `Cow`.
    pub fn code(&self) -> Cow<'static, str> {
        match self {
            Self::BadRequest(_) => Cow::Borrowed("BadRequest"),
            Self::Unauthorized(_) => Cow::Borrowed("Unauthorized"),
            Self::Forbidden(_) => Cow::Borrowed("Forbidden"),
            Self::NotFound(_) => Cow::Borrowed("NotFound"),
            Self::NotAcceptable(_) => Cow::Borrowed("NotAcceptable"),
            Self::BodyTimeout => Cow::Borrowed("Timeout"),
            Self::Conflict(_) => Cow::Borrowed("conflict"),
            Self::Gone(_) => Cow::Borrowed("cursor"),
            Self::TooLarge { .. } => Cow::Borrowed("too_large"),
            Self::Invalid { .. } => Cow::Borrowed("invalid"),
            Self::Busy { .. } => Cow::Borrowed("Busy"),
            Self::Motor(fault) => fault.code(),
            Self::Internal(_) => Cow::Borrowed("internal"),
        }
    }

    /// The HTTP status, from the same variant list as [`HubApiError::code`].
    pub fn status(&self) -> u16 {
        match self {
            Self::BadRequest(_) => 400,
            Self::Unauthorized(_) => 401,
            Self::Forbidden(_) => 403,
            Self::NotFound(_) => 404,
            Self::NotAcceptable(_) => 406,
            Self::BodyTimeout => 408,
            Self::Conflict(_) => 409,
            Self::Gone(_) => 410,
            Self::TooLarge { .. } => 413,
            Self::Invalid { .. } => 422,
            Self::Busy { retry_after } => {
                if *retry_after == 0 {
                    503
                } else {
                    429
                }
            }
            Self::Motor(fault) => fault.status(),
            Self::Internal(_) => 500,
        }
    }

    /// `Retry-After` in seconds, when §5.2 or §6 gives one: the subscriber caps only.
    ///
    /// A permit or pool wait answers 503 with `Retry-After: 1` through [`HubApiError::busy_wait`],
    /// which is a different constructor because it is not a queue-full refusal.
    pub fn retry_after(&self) -> Option<u64> {
        match self {
            Self::Busy { retry_after } => Some(*retry_after),
            _ => None,
        }
    }

    /// 503 for a wait that ran out (`GRAPH_HUB_TIMEOUT_MS`), with `Retry-After: 1`.
    pub fn busy_wait() -> Self {
        Self::Busy { retry_after: 0 }
    }

    /// 429 for a subscriber cap, with the cap's own `Retry-After`.
    pub fn busy_subscriber(retry_after: u64) -> Self {
        Self::Busy { retry_after }
    }
}

impl IntoResponse for HubApiError {
    fn into_response(self) -> Response {
        // A relayed answer is graph-server's own body, byte for byte: its `error` string and its
        // message, under its own status (Decision 9, `docs/decisions/graph-hub.md`).
        if let Self::Motor(fault) = &self
            && let Some(answer) = fault.relayed()
        {
            return answer.into_response();
        }
        let message = match &self {
            Self::TooLarge { what, limit } => format!("over the {what} limit of {limit}"),
            Self::Invalid { path, what } => {
                if path.is_empty() {
                    format!("the body: {what}")
                } else {
                    format!("{path}: {what}")
                }
            }
            Self::Motor(fault) => fault.message().to_owned(),
            other => reason(other).to_owned(),
        };
        let body = serde_json::json!({ "error": self.code(), "message": one_line(&message) });
        let json = HeaderValue::from_static("application/json");
        let status =
            StatusCode::from_u16(self.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut response =
            (status, [(header::CONTENT_TYPE, json)], body.to_string()).into_response();
        if let Some(extra) = self.extra_header() {
            response.headers_mut().insert(extra.0, extra.1);
        }
        response
    }
}

impl HubApiError {
    /// The one header a refusal adds: `WWW-Authenticate` on a 401, `Retry-After` on a wait.
    ///
    /// Caveat: a 503 whose `Retry-After` is 1 comes from a timed-out wait, not from a full
    /// subscriber cap, and the header is the same on both: the SDK's backoff is jittered either
    /// way (`docs/superpowers/specs/2026-10-05-graph-service-plugins-design.md` §7).
    fn extra_header(&self) -> Option<(axum::http::HeaderName, HeaderValue)> {
        match self {
            Self::Unauthorized(_) => {
                Some((header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer")))
            }
            Self::Busy { .. } => Some((header::RETRY_AFTER, HeaderValue::from_static("1"))),
            _ => None,
        }
    }
}

/// The human-readable reason of every variant that carries no message of its own.
fn reason(error: &HubApiError) -> &str {
    match error {
        HubApiError::BadRequest(why) => why,
        HubApiError::Unauthorized(_) => "missing or unknown API key",
        HubApiError::Forbidden(_) => "the key has no grant for this workspace and plugin",
        HubApiError::NotFound(_) => "no such route",
        HubApiError::BodyTimeout => "the body did not arrive within GRAPH_HUB_BODY_TIMEOUT_MS",
        _ => "",
    }
}

/// `text` on one line and at most [`MAX_MESSAGE`] bytes, so a caller's newline cannot split a log
/// line and a message echoed back cannot grow without bound.
fn one_line(text: &str) -> String {
    let mut out: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if out.len() > MAX_MESSAGE {
        let mut end = MAX_MESSAGE;
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
        out.push_str("...");
    }
    out
}
