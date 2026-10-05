//! §5.1 step 2's lookup and step 7's insert: the idempotency row.
//!
//! One row per `(workspace, plugin, key)`, written in the batch's own transaction and swept 24 h
//! later by Task 10's sweeper. A replay answers with the stored response and takes no seq, so the
//! stored text is the answer and not a re-derivation of it.

use tokio_postgres::Client;

use crate::error::{DbError, StoreError};
use crate::writer::Idempotency;

/// The most bytes a key may be (§5.1).
///
/// 128 bytes, not 128 characters: the key is opaque bytes and a key of 128 multi-byte characters
/// is 384 bytes on the wire.
const MAX_KEY: usize = 128;

/// The prefix of the answer text this store writes, as `answer_json` spells it.
///
/// A replay has to answer with the `applied` count as well as the seq, and the table holds the
/// response rather than the count. Reading the count back out of the two numbers the store itself
/// wrote is cheaper than a JSON parse and cannot disagree with the text a client received.
const APPLIED_PREFIX: &str = "{\"applied\":";

/// What a hit on the key holds.
pub(crate) struct Stored {
    /// The SHA-256 of the body the first request carried.
    pub body_sha256: Vec<u8>,
    /// The answer text, exactly as it was first sent.
    pub response: String,
    /// The seq that answer named.
    pub seq: u64,
}

/// §5.1 step 2: the stored response for this key, or `None`.
///
/// A stored row whose body hash differs from this request's is a 422, not a second application: the
/// key is the client's promise that these two requests are the same request, and they are not.
///
/// Caveat: `no-idem` turns the lookup off, which is what makes a replay apply the batch again
/// rather than answer with what it answered before. The refusal that follows it is unchanged, so
/// the control turns `idem_replay_returns_the_stored_response_and_no_seq` red through the answer
/// and `hundred_writers_have_no_gap` red through the missing rows.
pub(crate) async fn lookup(
    client: &mut Client,
    key: &Idempotency,
    ws: &str,
    plugin: &str,
) -> Result<Option<Stored>, StoreError> {
    if crate::breaks::on("no-idem") {
        return Ok(None);
    }
    let row = client
        .query_opt(
            "SELECT body_sha256, response, seq FROM idempotency \
             WHERE ws = $1 AND plugin = $2 AND key = $3",
            &[&ws, &plugin, &key.key],
        )
        .await?;
    Ok(row.map(|row| Stored {
        body_sha256: row.get(0),
        response: row.get(1),
        seq: row.get::<_, i64>(2) as u64,
    }))
}

/// The body hash mismatch refusal, which is a 422 and not a conflict.
pub(crate) fn different_body() -> StoreError {
    StoreError::Hub(graph_contract::hub::HubError::Invalid {
        path: "idempotency-key".to_owned(),
        what: "the key was used before with a different request body".to_owned(),
    })
}

/// §5.1 step 7: store the answer beside the batch that produced it.
///
/// The unique violation is [`StoreError::Duplicate`] and not a plain conflict, because it is one of
/// the three refusals [`crate::writer::retry`] runs the whole transaction again for; the retry then
/// finds the stored response at step 2.
pub(crate) async fn record(
    client: &mut Client,
    key: &Idempotency,
    ws: &str,
    plugin: &str,
    response: &str,
    seq: u64,
) -> Result<(), StoreError> {
    if crate::breaks::on("no-idem") {
        return Ok(());
    }
    let result = client
        .execute(
            "INSERT INTO idempotency (ws, plugin, key, body_sha256, response, seq) \
             VALUES ($1, $2, $3, $4, $5, $6)",
            &[
                &ws,
                &plugin,
                &key.key,
                &&key.body_sha256[..],
                &response,
                &(seq as i64),
            ],
        )
        .await;
    match result {
        Ok(_) => Ok(()),
        Err(err) if sqlstate(&err) == "23505" => Err(StoreError::Duplicate {
            what: format!("idempotency key `{}` for `{ws}`/`{plugin}`", key.key),
        }),
        Err(err) => Err(err.into()),
    }
}

/// The `applied` count inside a response this store wrote, or a store error.
///
/// The text is always [`graph_contract::hub::answer_json`]'s, so this reads a fixed prefix and one
/// pair of separators. Anything else means the row was written by something that is not this store,
/// and it is reported rather than answered with a count of zero.
pub(crate) fn applied_in(response: &str) -> Result<u64, StoreError> {
    let rest = response.strip_prefix(APPLIED_PREFIX).ok_or_else(|| {
        StoreError::Db(DbError::store(
            "XX004",
            format!("a stored idempotency response is not this store's: {response}"),
        ))
    })?;
    let (applied, _) = rest.split_once(',').ok_or_else(|| {
        StoreError::Db(DbError::store(
            "XX004",
            format!("a stored idempotency response is not this store's: {response}"),
        ))
    })?;
    applied.parse::<u64>().map_err(|_| {
        StoreError::Db(DbError::store(
            "XX004",
            format!("a stored idempotency response has no applied count: {response}"),
        ))
    })
}

/// The SHA-256 of a request body, as `sha2` computes it over bytes.
///
/// Public because the hub hashes the raw body and the store has to hash the same bytes to compare:
/// a caller cannot reproduce `sha2`'s output any other way without taking the dependency itself.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    let mut out = [0u8; 32];
    out.copy_from_slice(&sha2::Sha256::digest(bytes));
    out
}

/// A key over 128 bytes is a 422 (§5.1) and not a 413.
///
/// 422 is what the spec says for this refusal, and it is also the only class that fits: the key is
/// a header the client chose, not a body that is too large, and a 413 would tell it to shorten its
/// body.
///
/// It is `pub(crate)` and called from step 2 rather than from [`lookup`], because the cap is a
/// property of the request and not of the stored row: a control that turns the row's lookup off
/// (`no-idem`) must not turn the cap off with it.
pub(crate) fn check_key(key: &str) -> Result<(), StoreError> {
    if key.len() > MAX_KEY {
        return Err(StoreError::Hub(graph_contract::hub::HubError::Invalid {
            path: "idempotency-key".to_owned(),
            what: format!("{MAX_KEY} bytes at most, this one is {}", key.len()),
        }));
    }
    Ok(())
}

/// The driver's SQLSTATE, or nothing.
fn sqlstate(err: &tokio_postgres::Error) -> &str {
    err.code().map(|code| code.code()).unwrap_or("")
}
