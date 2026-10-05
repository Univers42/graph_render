//! The document's byte count, kept exactly (spec §6 `doc_bytes`, plan N8).
//!
//! `doc_bytes` is what bounds the streamed document from above, and it equals it exactly when
//! nothing was pruned. It is therefore arithmetic over graph-contract's own pieces and never a
//! second estimate of them: `frame_bytes` is the ingest writer's, and this module only adds and
//! subtracts the stored lengths under the workspace row lock.
//!
//! The shape, from the ingest writer's concatenation:
//!
//! ```text
//! doc_bytes = frame_bytes(ws, collections, records)
//!           + Σ collection_piece(c).len() over every manifest's collections, ids qualified
//!           + Σ text_bytes over every record of the workspace
//! ```

use graph_contract::hub::{HubError, Manifest, qualify};
use graph_contract::ingest::{collection_piece, frame_bytes};

use crate::error::{DbError, StoreError};

/// The bytes of the document that are not a collection and not a record: the head, the middle, the
/// tail and the separators, for `collections` declarations and `records` records.
///
/// graph-contract's own `frame_bytes`, called rather than reimplemented: the separator counts are
/// the writer's, and a second spelling of them would drift.
pub(crate) fn frame(ws: &str, collections: u64, records: u64) -> u64 {
    frame_bytes(ws, collections, records)
}

/// How much the frame grows when the counts move from `from` to `to`.
///
/// Signed, because a delete shrinks it and the caller holds one signed total for every change in
/// the batch. The two counts are `(collections, records)`.
pub(crate) fn frame_delta(ws: &str, from: (u64, u64), to: (u64, u64)) -> i64 {
    frame(ws, to.0, to.1) as i64 - frame(ws, from.0, from.1) as i64
}

/// The bytes `plugin`'s declarations add to the document, separators excluded.
///
/// The document declares each collection under its qualified id (`plugin.collection`), so that is
/// the id measured here; the bare id would undercount by `plugin.len() + 1` per collection. The
/// separators between collections are in [`frame_delta`], counted over the whole workspace, so
/// counting them here as well would count them twice.
pub(crate) fn decl(plugin: &str, manifest: &Manifest) -> u64 {
    manifest
        .collections
        .iter()
        .map(|c| {
            let mut declared = c.clone();
            declared.id = qualify(plugin, &c.id);
            collection_piece(&declared).len() as u64
        })
        .sum()
}

/// `total + delta`, or a store error when the total would go below zero.
///
/// Caveat: `i128` arithmetic, because a delta is a sum of up to `max_batch` record lengths and
/// `doc_bytes` is a `bigint`: the intermediate sum can leave `i64` even when both ends fit in it.
/// The subtraction cannot go below zero in a correct build — a record's bytes are subtracted only
/// when the row they belong to was read under the same lock — so a refusal here is a store bug, and
/// it is reported as one rather than clamped.
pub(crate) fn add(total: u64, delta: i64) -> Result<u64, StoreError> {
    let next = i128::from(total) + i128::from(delta);
    u64::try_from(next).map_err(|_| {
        StoreError::Db(DbError::store(
            "XX003",
            format!("doc_bytes went below zero: {total} + {delta}"),
        ))
    })
}

/// Refuse past a cap, with the store-side cap named.
///
/// `what` is `HubError::TooLarge`'s own vocabulary, which names the *wire* caps; these two are
/// store-side, so they say what they measure rather than pretending to be a body or a batch.
///
/// Caveat: the check is `have > limit`, so a total exactly at the cap is accepted. N8 states the
/// bound as a bound, and a store that refused at the boundary would refuse a workspace that is
/// exactly as large as the spec allows.
pub(crate) fn cap(what: &'static str, have: u64, limit: u64) -> Result<(), StoreError> {
    if have > limit {
        return Err(StoreError::Hub(HubError::TooLarge { what, limit }));
    }
    Ok(())
}
