//! §5.2's motor-status table, as one `match` on `(status, error)` and nothing else.
//!
//! This is the only module that turns a graph-server answer into a hub status. Every other refusal
//! in the hub is one of [`HubApiError`]'s own variants and is written down in `error.rs`; the seven
//! `MotorFault`s exist for exactly the rows below, and no other module constructs one.
//!
//! The order of the arms is the table's own, and the default arm is last: a status §5.2 has no row
//! for is a 502 `MotorError`, never a relayed answer, because relaying an answer the contract does
//! not name would give a caller a status the hub does not promise.
//!
//! Caveat: the 422 split is on the motor's own `error` string and not on its status, because N3
//! gives one status two meanings. A 422 whose `error` is neither `IngestInvalid` nor
//! `ContractInvalid` nor `LayoutFailed` nor `PostFailed` falls to the default arm and is a 502:
//! the hub does not guess which of the two halves of a 422 it is looking at.

use crate::breaks;
use crate::error::{HubApiError, MotorFault};

/// What §5.2's table says about one motor answer.
pub enum Mapped {
    /// The 200 row: the bytes pass through unchanged, with `Content-Type` and `Vary` relayed.
    Pass,
    /// Every other row: the hub's own refusal.
    Refuse(HubApiError),
}

/// §5.2's table, row by row.
///
/// `status` is graph-server's status line and `error` the `error` member of its body; `message` is
/// its `message` member, which the 413 row keeps and the relayed rows pass on.
///
/// Caveat: the 503 row relays graph-server's `error` string with **no** `Retry-After`, because
/// §5.3's Caveat is that graph-server's 503 `Timeout` covers both an admission wait and an overrun
/// and the hub cannot tell them apart; giving it a `Retry-After` would invite the SDK to retry a
/// request that §5.3 says it never retries, and a distinct overrun code is graph-render-4f's call.
pub fn fault(status: u16, error: &str, message: &str) -> Mapped {
    match (status, error) {
        (200, _) => Mapped::Pass,
        (400, _) | (406, _) | (503, _) => relayed(status, error, message),
        (401, _) => Mapped::Refuse(HubApiError::Motor(MotorFault::Auth)),
        (408, _) => Mapped::Refuse(HubApiError::Motor(MotorFault::BodyTimeout)),
        (413, _) => Mapped::Refuse(HubApiError::Motor(MotorFault::TooLarge {
            message: message.to_owned(),
        })),
        (422, "IngestInvalid" | "ContractInvalid") => {
            Mapped::Refuse(HubApiError::Motor(MotorFault::MaterializeInvalid))
        }
        (422, "LayoutFailed" | "PostFailed") if !breaks::on("layoutfailed-as-502") => {
            relayed(status, error, message)
        }
        // The motor's queue is full, so the caller waits in the hub's queue: §5.2 answers 503 with
        // `Retry-After: 1`, which is `busy_wait()` and not a subscriber cap's 429.
        (429, _) => Mapped::Refuse(HubApiError::busy_wait()),
        _ => Mapped::Refuse(HubApiError::Motor(MotorFault::Error)),
    }
}

/// Is this refusal one §5.2 wants written to the log?
///
/// The three that are: the motor refused the hub's **own** key, the document did not read at the
/// motor (which `hub-materialize` proves cannot happen, so it is a hub defect), and any status the
/// table has no row for. A relayed 400, 406, 429 or 503 is the caller's own request and is not a
/// defect of anything.
pub fn defect(error: &HubApiError) -> bool {
    matches!(
        error,
        HubApiError::Motor(MotorFault::Auth | MotorFault::MaterializeInvalid | MotorFault::Error)
    )
}

/// One relayed row: graph-server's own status, `error` and message, kept whole.
fn relayed(status: u16, error: &str, message: &str) -> Mapped {
    Mapped::Refuse(HubApiError::Motor(MotorFault::Relayed {
        status,
        error: error.to_owned(),
        message: message.to_owned(),
    }))
}
