//! The one error body every route answers with (`docs/contract/service-api.md` "Errors"):
//! `{"error": "<code>", "message": "<one line>"}`, where `code` is the motor's own `Code` name
//! whenever the motor is the one refusing, so the browser SDK and the service name a failure
//! the same way.

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

/// The longest message an error body carries, in bytes. A refusal can quote the caller's own
/// text (an unknown member's key, a duplicate id), and without a bound a 64 MiB key would be
/// echoed back whole.
const MAX_MESSAGE: usize = 256;

/// A refusal: the status it answers with, its code name and a one-line reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    /// The HTTP status.
    pub status: StatusCode,
    /// The motor's `Code` name, or the service's own name when the motor never ran.
    pub code: String,
    /// What was wrong, for a human. Never the key or its hash.
    pub message: String,
}

impl ApiError {
    /// A refusal with this status, code and message.
    pub fn new(status: StatusCode, code: &str, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.to_owned(),
            message: message.into(),
        }
    }

    /// 400: the request itself is malformed.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "BadRequest", message)
    }

    /// A 401 with one message for every cause: the body never says whether the key was
    /// absent, malformed or unknown.
    pub fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "Unauthorized",
            "missing or unknown API key",
        )
    }

    /// 413: the document is past a size limit.
    pub fn too_large(message: impl Into<String>) -> Self {
        Self::new(StatusCode::PAYLOAD_TOO_LARGE, "IngestTooLarge", message)
    }

    /// 429: every slot is busy and the queue is full.
    pub fn busy() -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "Busy",
            "every compute slot is busy and the queue is full",
        )
    }

    /// 503: the request ran past `GRAPH_TIMEOUT_MS`.
    pub fn timeout() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Timeout",
            "the request ran past GRAPH_TIMEOUT_MS",
        )
    }

    /// 404.
    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "NotFound", "no such route")
    }

    /// 406: `Accept` excludes both faces.
    pub fn not_acceptable() -> Self {
        Self::new(
            StatusCode::NOT_ACCEPTABLE,
            "NotAcceptable",
            "Accept allows neither application/vnd.graph-motor.snapshot nor application/json",
        )
    }

    /// 408: the body did not arrive within `GRAPH_BODY_TIMEOUT_MS`.
    pub fn body_timeout() -> Self {
        Self::new(
            StatusCode::REQUEST_TIMEOUT,
            "Timeout",
            "the body did not arrive within GRAPH_BODY_TIMEOUT_MS",
        )
    }

    /// 500: the server's own failure, never the caller's.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "Internal", message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({ "error": self.code, "message": one_line(&self.message) });
        let json = HeaderValue::from_static("application/json");
        let mut response = (
            self.status,
            [(header::CONTENT_TYPE, json)],
            body.to_string(),
        )
            .into_response();
        let extra = match self.status {
            StatusCode::TOO_MANY_REQUESTS => Some((header::RETRY_AFTER, "1")),
            StatusCode::UNAUTHORIZED => Some((header::WWW_AUTHENTICATE, "Bearer")),
            _ => None,
        };
        if let Some((name, value)) = extra {
            let value = HeaderValue::from_static(value);
            response.headers_mut().insert(name, value);
        }
        response
    }
}

/// `text` on one line and at most [`MAX_MESSAGE`] bytes: control characters become spaces, so
/// a caller's newline can never split a log line or a message in two.
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

#[cfg(test)]
mod tests {
    use super::one_line;

    #[test]
    fn a_message_is_one_bounded_line() {
        assert_eq!(one_line("a\nb\tc"), "a b c");
        let long = "é".repeat(300);
        let cut = one_line(&long);
        assert!(cut.len() <= super::MAX_MESSAGE + 3, "{} bytes", cut.len());
        assert!(cut.ends_with("..."));
    }
}
