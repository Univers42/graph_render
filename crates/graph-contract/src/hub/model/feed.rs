//! Staging: what an `apply` computes before anything is committed.
//!
//! Split out of `model.rs` because it is the one part of `apply` that is not "look at the
//! map": it builds a *complete* second set of records and hands it back, so the commit is
//! one assignment. That is what makes a batch atomic — there is no window in which half a
//! batch is stored, because there is no window at all.
//!
//! Two rules are decided here rather than in `model.rs`, and both are about `rev`:
//!
//! - A record that is **not there** gets rev 1. That includes one that was deleted: a
//!   re-created record has not existed as far as the store is concerned, and a client that
//!   saw rev 9 of the *old* record must not see its replacement called rev 10.
//! - An **identical** upsert stages nothing. Same collection, same id, same canonical
//!   text: nothing about the stored record would change, so there is no new `rev` and no
//!   entry in [`Applied`]. A change in the stream that a reader replays to no effect is a
//!   change the reader has to skip, and skipping is a rule the reader has to get right.

use super::{Applied, Model, Stored};

/// The records map a staging walk mutates: the model's own key, so a staged key and a
/// stored one cannot be spelled differently.
type Records = std::collections::BTreeMap<(String, String), Stored>;
use crate::ingest::{Record, record_piece};

/// The staged outcome of one batch: the records map it would leave behind, and what it
/// would report. Both come from the same walk, so they cannot disagree.
pub(super) struct Staged {
    /// Every record the model would hold afterwards.
    pub records: Records,
    /// What the batch changed.
    pub applied: Applied,
}

impl Staged {
    /// The records, moved out: the model takes this and nothing else, so committing is one
    /// field assignment and the old map is dropped whole.
    pub(super) fn into_records(self) -> Records {
        self.records
    }
}

/// The batch staged against `model`, or the refusal. Nothing is written to `model`: the
/// caller decides, once, whether to take the result.
pub(super) fn stage(
    model: &Model,
    plugin: &str,
    batch: &super::super::batch::Batch,
) -> Result<Staged, HubError> {
    let mut records = model.records_map().clone();
    let upserted = apply_upserts(&mut records, plugin, batch);
    let deleted = apply_deletes(&mut records, plugin, batch);
    Ok(Staged {
        records,
        applied: Applied { upserted, deleted },
    })
}

/// Every upsert staged into `records`, and the `(record, rev)` pairs to report.
fn apply_upserts(
    records: &mut Records,
    plugin: &str,
    batch: &super::super::batch::Batch,
) -> Vec<(Record, u64)> {
    let mut upserted = Vec::with_capacity(batch.upserts.len());
    for up in &batch.upserts {
        let record = up.record(plugin);
        let key = (record.collection.clone(), record.id.clone());
        let text = record_piece(&record);
        // An identical upsert: same text at the same key. Checked on the *text*, not on
        // the cells, because the text is what a reader of this document would see — two
        // records whose cells compare equal in one order and not another are a writer bug,
        // and here we care about the bytes.
        if records.get(&key).is_some_and(|stored| stored.text == text) {
            continue;
        }
        let rev = next_rev(records.get(&key));
        records.insert(
            key,
            Stored {
                rev,
                text,
                record: record.clone(),
            },
        );
        upserted.push((record, rev));
    }
    upserted
}

/// Every delete staged into `records`, and the `(collection, id, rev)` triples to report.
fn apply_deletes(
    records: &mut Records,
    plugin: &str,
    batch: &super::super::batch::Batch,
) -> Vec<(String, String, u64)> {
    let mut deleted = Vec::with_capacity(batch.deletes.len());
    for remove in &batch.deletes {
        let key = (
            super::super::qualify(plugin, &remove.collection),
            remove.id.clone(),
        );
        // A delete of something absent is a no-op, not an error: a batch may be a replay,
        // and a replay must not fail. Nothing is reported for it either, because nothing
        // changed — same rule as the identical upsert.
        if let Some(stored) = records.remove(&key) {
            deleted.push((key.0, key.1, stored.rev));
        }
    }
    deleted
}

/// The `rev` a record written over `previous` gets: one more, or 1 when there was nothing
/// there — including after a delete, which is the case the module doc is about.
fn next_rev(previous: Option<&Stored>) -> u64 {
    previous.map_or(Model::first_rev(), |stored| stored.rev + 1)
}

/// The error type, named once so the signature above reads as a sentence.
type HubError = super::super::HubError;
