//! `ETag` and `If-None-Match`: the exact string compare `/graph` answers 304 from.
//!
//! RFC 9110 §13.1.2 allows a weak comparison for `If-None-Match`, and this refuses it. The hub's
//! `ETag` is `"<epoch>.<seq>"` — a **position**, not a byte range and not a content hash — so two
//! documents at the same cursor are byte-identical (the store's own property) and a weak compare
//! would add nothing while letting `W/"1.2"` through, which no client of this API ever sends.
//!
//! Caveat: `*` is honoured, because RFC 9110 defines it as "if any current representation exists"
//! and that is the one wildcard a client may send without knowing the tag.

use axum::http::HeaderMap;
use axum::http::header;
use graph_contract::hub::Cursor;

/// The `ETag` of a cursor: `"<epoch>.<seq>"`, quoted as RFC 9110 requires.
pub fn quoted(cursor: &Cursor) -> String {
    format!("\"{cursor}\"")
}

/// Does `If-None-Match` name `cursor`?
///
/// `None` (the header absent) is `false`: no condition was sent, so there is nothing to match. A
/// header may carry several tags, and one match is enough. Every other byte of the tag — a `W/`
/// prefix, whitespace — is compared literally, which is what makes this an exact compare.
pub fn matches(headers: &HeaderMap, cursor: &Cursor) -> bool {
    if !headers.contains_key(header::IF_NONE_MATCH) {
        return false;
    }
    let wanted = quoted(cursor);
    headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .any(|value| {
            value.to_str().is_ok_and(|text| {
                text.split(',')
                    .any(|tag| tag.trim() == "*" || tag.trim() == wanted)
            })
        })
}
