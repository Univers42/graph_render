//! §5.2's relay column: one variant per row of the table that maps a graph-server answer to a hub
//! answer. Every fault is 502 except [`MotorFault::TooLarge`] (413) and [`MotorFault::Relayed`]
//! (the status graph-server chose), and `auth.rs` never constructs one — `relay/map.rs` is the
//! only module that does, in Task 8.

use std::borrow::Cow;

use crate::error::MAX_MESSAGE;

/// What the motor answered, in the hub's own vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MotorFault {
    /// 401 from the motor: the hub's own motor key is wrong. A hub defect, logged.
    Auth,
    /// 408 from the motor: the upload missed graph-server's `GRAPH_BODY_TIMEOUT_MS`. No
    /// `Retry-After`: the same request would miss it again.
    BodyTimeout,
    /// 413 from the motor: the workspace is past what the motor accepts. 413 with its message.
    TooLarge {
        /// graph-server's own message, whole.
        message: String,
    },
    /// 422 with `IngestInvalid` or `ContractInvalid`: the document did not read. A hub defect,
    /// logged; `hub-materialize` proves H12 makes it readable.
    MaterializeInvalid,
    /// 500 from the motor, or any status §5.2 has no row for.
    Error,
    /// Unreachable, or no answer within `GRAPH_HUB_MOTOR_TIMEOUT_MS`.
    Unavailable,
    /// A row §5.2 relays unchanged: 400, 406, the 422 `LayoutFailed`/`PostFailed` arm, and 503
    /// (without `Retry-After`).
    Relayed {
        /// graph-server's own status.
        status: u16,
        /// graph-server's own `error` string.
        error: String,
        /// graph-server's own message.
        message: String,
    },
}

impl MotorFault {
    /// §5.2's `Motor*` name for this fault, or graph-server's own `error` string for a relayed
    /// answer. Borrows for the same reason [`crate::error::HubApiError::code`] does.
    pub fn code(&self) -> Cow<'static, str> {
        match self {
            Self::Auth => Cow::Borrowed("MotorAuth"),
            Self::BodyTimeout => Cow::Borrowed("MotorBodyTimeout"),
            Self::TooLarge { .. } => Cow::Borrowed("GraphTooLarge"),
            Self::MaterializeInvalid => Cow::Borrowed("MaterializeInvalid"),
            Self::Error => Cow::Borrowed("MotorError"),
            Self::Unavailable => Cow::Borrowed("MotorUnavailable"),
            Self::Relayed { error, .. } => Cow::Owned(error.clone()),
        }
    }

    /// The status this fault answers with. Every `MotorFault` is 502 except `TooLarge` (413) and
    /// `Relayed` (graph-server's own status).
    pub fn status(&self) -> u16 {
        match self {
            Self::TooLarge { .. } => 413,
            Self::Relayed { status, .. } => *status,
            _ => 502,
        }
    }

    /// The message this fault carries, one line.
    pub fn message(&self) -> &str {
        match self {
            Self::Auth => "the motor refused the hub's own key; this is a hub defect",
            Self::BodyTimeout => "the upload missed the motor's GRAPH_BODY_TIMEOUT_MS",
            Self::TooLarge { message } => message,
            Self::MaterializeInvalid => {
                "the document did not read at the motor; this is a hub defect"
            }
            Self::Error => "the motor refused the request; this is a hub defect",
            Self::Unavailable => "the motor is unreachable or past GRAPH_HUB_MOTOR_TIMEOUT_MS",
            Self::Relayed { message, .. } => message,
        }
    }

    /// The answer to relay verbatim, when §5.2 has a row for it.
    ///
    /// Caveat: a relayed 503 carries no `Retry-After` (§5.2's note): the SDK never retries
    /// `/layout`, and an `Retry-After` there would invite one.
    pub fn relayed(&self) -> Option<Relayed> {
        match self {
            Self::Relayed {
                status,
                error,
                message,
            } => Some(Relayed {
                status: *status,
                error: error.clone(),
                message: one_line(message),
            }),
            _ => None,
        }
    }
}

/// One answer passed through untouched, in the shape every refusal of this hub already has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relayed {
    /// graph-server's status, as the hub answers it.
    pub status: u16,
    /// graph-server's `error` string.
    pub error: String,
    /// graph-server's message, on one line and bounded.
    pub message: String,
}

impl Relayed {
    /// The relayed body: `{"error", "message"}` under graph-server's own status.
    pub fn body(&self) -> String {
        serde_json::json!({ "error": self.error, "message": self.message }).to_string()
    }
}

impl axum::response::IntoResponse for Relayed {
    fn into_response(self) -> axum::response::Response {
        use axum::http::{HeaderValue, StatusCode, header};
        let json = HeaderValue::from_static("application/json");
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::BAD_GATEWAY);
        (status, [(header::CONTENT_TYPE, json)], self.body()).into_response()
    }
}

/// `text` on one line and at most [`MAX_MESSAGE`] bytes. The hub's own bound, so a relayed message
/// is not the one place an answer grows without limit.
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
