//! The writers for what a hub answers with: a change, a manifest change, a notice and an
//! answer.
//!
//! Four writers, one rule: **keys in byte order, no space, no newline**. The key order is
//! not cosmetic — a change is stored and skipped over, and two clients that stored the
//! same change have to agree byte for byte about what that change was, exactly as two
//! adapters' ingest documents have to (`prompt.md` §4.1, H6). The spec's `/changes`
//! example spells the members in illustration order; the byte order is the contract.
//!
//! Nothing here decides *whether* a change happened or what is in it: this file is given
//! the records and the revs the store assigned and writes them. [`max_change`] and
//! [`check_change`] are the one piece of policy, because a change body's size cannot be
//! bounded from the request that caused it (see `tests/change.rs`'s `1e300` test).

use super::manifest::Manifest;
use super::{HubError, Limits};
use crate::ingest::JsonValue;
use crate::ingest::Record;
use crate::ingest::write::{members, record_rows};

/// The room one operation needs in a change body: its wrapper members (`collection`,
/// `id`, `rev`, the two separators) are bounded by the id grammar, so 96 bytes is a
/// constant rather than a number computed per record.
const PER_OPERATION: u64 = 96;

/// What every change carries about *where* and *when*: the stream position it is at, the
/// plugin it is for, and the timestamp it was applied at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeHead<'a> {
    /// The stream position this change occupies.
    pub seq: u64,
    /// The plugin the records belong to.
    pub plugin: &'a str,
    /// When the change was applied, already in the wire's own timestamp spelling — a `u64`
    /// or a clock is the caller's business, and this crate has no clock (D2).
    pub at: &'a str,
}

/// One batch's worth of change: the records stored and the records removed, each with the
/// `rev` the store assigned.
///
/// The upserts are `(&Record, rev)` because the `rev` is the *store's* number and lives
/// beside the record rather than inside it: a `Record` is an ingest record, and the hub's
/// revision is not a fact the ingest contract knows about.
pub fn change_json(
    head: &ChangeHead<'_>,
    upserts: &[(Record, u64)],
    deletes: &[(String, String, u64)],
) -> String {
    let body = members(&[
        ("at", quote(head.at)),
        ("deletes", deletes_json(deletes)),
        ("kind", "\"batch\"".to_owned()),
        ("plugin", quote(head.plugin)),
        ("seq", head.seq.to_string()),
        ("upserts", upserts_json(upserts)),
    ]);
    body
}

/// A manifest's change: the same envelope, `kind: "manifest"`, and the manifest's own
/// canonical text nested whole. Nested rather than re-serialized, so what a client is told
/// matches what it published byte for byte.
pub fn manifest_change_json(head: &ChangeHead<'_>, manifest: &Manifest) -> String {
    let body = members(&[
        ("at", quote(head.at)),
        ("kind", "\"manifest\"".to_owned()),
        ("manifest", super::manifest_json(manifest)),
        ("plugin", quote(head.plugin)),
        ("seq", head.seq.to_string()),
    ]);
    body
}

/// A notice: the envelope with no payload. It says the stream moved and carries nothing
/// else, so a client that only watches positions does not have to parse records it will
/// never read.
pub fn notice_json(head: &ChangeHead<'_>) -> String {
    let body = members(&[
        ("at", quote(head.at)),
        ("kind", "\"notice\"".to_owned()),
        ("plugin", quote(head.plugin)),
        ("seq", head.seq.to_string()),
    ]);
    format!("{{{body}}}")
}

/// The answer to an applied batch: the sequence the stream is now at, and how many of the
/// batch's operations were applied. The two are different on purpose — a batch can be
/// applied and change nothing, and "applied: 0" is the honest answer for an idempotent
/// re-send.
pub fn answer_json(seq: u64, applied: u64) -> String {
    format!("{{\"applied\":{applied},\"seq\":{seq}}}")
}

/// The largest change body accepted: `max_body` plus room for every operation in a full
/// batch. It cannot be `max_body`, because a batch that fits the request cap can expand
/// (each `1e300` writes as a 301-digit integer); it cannot be `max_batch × record cap`
/// either, since that is a gigabyte the service would never hold. This is the number the
/// spec fixes, and [`check_change`] is what enforces it.
pub fn max_change(limits: &Limits) -> u64 {
    limits.max_body + PER_OPERATION * limits.max_batch
}

/// A change body inside [`max_change`], or the refusal. The check is on the bytes,
/// before anything parses: an oversized response is refused the same way an oversized
/// request is, so one rule covers both directions.
pub fn check_change(text: &str, limits: &Limits) -> Result<(), HubError> {
    let limit = max_change(limits);
    if text.len() as u64 > limit {
        return Err(HubError::TooLarge {
            what: "change",
            limit,
        });
    }
    Ok(())
}

/// The `upserts` array: each record's own members, written by the ingest writer, with the
/// store's `rev` added. `record_rows` is the writer's own list of a record's members, so
/// the change's record text cannot differ from the document's for the same record.
fn upserts_json(upserts: &[(Record, u64)]) -> String {
    let rows: Vec<String> = upserts
        .iter()
        .map(|(record, rev)| upsert_json(record, *rev))
        .collect();
    format!("[{}]", rows.join(","))
}

fn upsert_json(record: &Record, rev: u64) -> String {
    let mut rows = record_rows(record);
    rows.push(("rev", rev.to_string()));
    // Sorted by key, like every other canonical text in this crate: `record_rows` is in
    // the *record's* key order, and `rev` belongs among them rather than after them.
    // `record_rows` is already sorted by key, and appending one member keeps that true,
    // so this insertion sort walks at most one element — but it is written as a sort so
    // the invariant does not depend on the caller having appended.
    rows.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(&b.1)));
    let body = members(&rows);
    format!("{{{body}}}")
}

/// The `deletes` array: `(collection, id, rev)`, all three strings/integers already known.
fn deletes_json(deletes: &[(String, String, u64)]) -> String {
    let rows: Vec<String> = deletes
        .iter()
        .map(|(collection, id, rev)| {
            let body = members(&[
                ("collection", quote(collection)),
                ("id", quote(id)),
                ("rev", rev.to_string()),
            ]);
            format!("{{{body}}}")
        })
        .collect();
    format!("[{}]", rows.join(","))
}

/// A wire string, quoted by the ingest writer, so the escaping here is the one every
/// other canonical text in this crate uses rather than a second implementation of it.
fn quote(text: &str) -> String {
    crate::ingest::to_json_value(&JsonValue::Text(text.to_owned()))
}