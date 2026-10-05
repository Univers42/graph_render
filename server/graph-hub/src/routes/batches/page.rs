//! The records page's own two query parameters: the opaque `cursor` and the `limit`.
//!
//! They are here rather than in the route because both are about **one page's position in the byte
//! order**, which is `graph_store::records`' coordinate and not a hub-wide concern: `/changes` reads
//! a `<epoch>.<seq>` cursor instead, and both go through [`crate::routes::query_of`], the one query
//! reader in the crate.

use std::collections::BTreeMap;

use crate::error::HubApiError;

/// The `cursor` query parameter as the records page's `(collection, id)` pair.
///
/// Caveat: the page's cursor is **opaque** on the wire and is the pair joined by a `\u{1f}` no
/// collection or record id can hold (`check_collection_id` and `check_record_id` both refuse it),
/// percent-encoded by [`encode_cursor`] so it survives a URL unquoted. The SDK passes it back
/// without reading it, which is what "opaque" means.
pub fn cursor_pair(
    query: &BTreeMap<String, String>,
) -> Result<Option<(String, String)>, HubApiError> {
    let Some(cursor) = query.get("cursor") else {
        return Ok(None);
    };
    if cursor.is_empty() {
        return Ok(None);
    }
    let (collection, id) = cursor
        .split_once('\u{1f}')
        .ok_or(HubApiError::BadRequest("the cursor is not this page's own"))?;
    Ok(Some((collection.to_owned(), id.to_owned())))
}

/// The `limit` query parameter: default 1 000, at most 10 000, and never zero.
///
/// Caveat: 10 000 is the page cap and not a §6 setting, so it is a wire constant here rather than
/// something `GRAPH_HUB_*` moves; a deployment that wants fewer rows per page gets them by asking.
pub fn limit(query: &BTreeMap<String, String>) -> Result<u64, HubApiError> {
    const DEFAULT_LIMIT: u64 = 1_000;
    const MAX_LIMIT: u64 = 10_000;
    let Some(raw) = query.get("limit") else {
        return Ok(crate::config::capped(DEFAULT_LIMIT));
    };
    let asked: u64 = raw
        .parse()
        .map_err(|_| HubApiError::BadRequest("limit is not a number"))?;
    if asked == 0 {
        return Err(HubApiError::BadRequest("limit is at least 1"));
    }
    Ok(crate::config::capped(asked.min(MAX_LIMIT)))
}

/// The characters a `next` token is percent-encoded over: the unit separator itself, plus every
/// byte that could end a query value or begin a header, so the token is safe in a URL unquoted.
///
/// Caveat: the encoding is the hub's own and not `base64url`, because the token is opaque to the
/// SDK either way and percent-encoding keeps it readable in a log line without decoding it.
const CURSOR_SET: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'%')
    .add(b'&')
    .add(b'/')
    .add(b':')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

/// The opaque `next` token for a page ending at `(collection, id)`.
pub fn encode_cursor(collection: &str, id: &str) -> String {
    percent_encoding::utf8_percent_encode(&format!("{collection}\u{1f}{id}"), CURSOR_SET)
        .to_string()
}
