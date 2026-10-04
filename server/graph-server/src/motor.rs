//! The motor as the service calls it: `graph_wasm::service`, the functions the wasm exports
//! call too (Verdict condition 1), and the mapping of its refusals to HTTP statuses
//! (`docs/contract/service-api.md` "Errors").

use crate::error::ApiError;
use axum::http::StatusCode;
use graph_contract::binary::Snapshot;
use graph_contract::canonical_json;

pub use graph_wasm::service::{Code, Source, build, layout_ids, post_ids, run};

/// The status a motor refusal answers with. A code the service never expects from a build or
/// a run (a handle code, an allocation failure) is the server's own failure, so a 500.
pub fn status_of(code: Code) -> StatusCode {
    match code {
        Code::IngestTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        Code::UnknownLayoutId | Code::IndexOutOfRange => StatusCode::BAD_REQUEST,
        Code::IngestInvalid | Code::ContractInvalid | Code::LayoutFailed | Code::PostFailed => {
            StatusCode::UNPROCESSABLE_ENTITY
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// The error body for a motor refusal. A 500 is named `Internal`, the one name the contract
/// gives a server failure; the motor's own name goes in the message.
pub fn refusal(code: Code) -> ApiError {
    let status = status_of(code);
    match code {
        _ if status == StatusCode::INTERNAL_SERVER_ERROR => {
            ApiError::internal(format!("the motor failed with {}", code.name()))
        }
        Code::UnknownLayoutId => ApiError::new(status, code.name(), "no layout has this id"),
        Code::IndexOutOfRange => ApiError::new(status, code.name(), "no POST pass has this id"),
        Code::IngestTooLarge => ApiError::too_large("the document is past the motor's limits"),
        _ => ApiError::new(status, code.name(), refused_message(code)),
    }
}

fn refused_message(code: Code) -> &'static str {
    match code {
        Code::IngestInvalid => "the document is not valid provisional ingest JSON",
        Code::ContractInvalid => "the document is not a valid ingest contract document",
        Code::LayoutFailed => "the layout refused this graph",
        _ => "the POST pass refused this geometry",
    }
}

/// The canonical JSON face of a binary snapshot. The motor just produced the bytes, so a
/// failure to read them back is the server's own fault.
pub fn json_face(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
    Snapshot::from_bytes(bytes)
        .map(|snapshot| canonical_json::to_json(&snapshot).into_bytes())
        .map_err(|_| ApiError::internal("the snapshot could not be re-read for its JSON face"))
}
