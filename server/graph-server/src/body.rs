//! Reading the request body (Verdict condition 4): streamed under `GRAPH_MAX_BODY`, chunked
//! bodies included, and under `GRAPH_BODY_TIMEOUT_MS`. A declared `Content-Length` past the
//! limit is refused before a byte is read.

use crate::config::Limits;
use crate::error::ApiError;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, header};
use http_body_util::{BodyExt, LengthLimitError, Limited};

/// The whole body, or the refusal: 413 past the limit, 408 past the read timeout.
pub async fn read(body: Body, headers: &HeaderMap, limits: &Limits) -> Result<Bytes, ApiError> {
    let declared = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok()?.parse::<u64>().ok());
    if declared.is_some_and(|length| length > limits.max_body as u64) {
        return Err(over_limit());
    }
    let limited = Limited::new(body, limits.max_body);
    match tokio::time::timeout(limits.body_timeout, limited.collect()).await {
        Err(_) => Err(ApiError::body_timeout()),
        Ok(Err(error)) if error.downcast_ref::<LengthLimitError>().is_some() => Err(over_limit()),
        Ok(Err(_)) => Err(ApiError::bad_request("the body could not be read")),
        Ok(Ok(collected)) => Ok(collected.to_bytes()),
    }
}

fn over_limit() -> ApiError {
    ApiError::too_large("the body is past GRAPH_MAX_BODY")
}
