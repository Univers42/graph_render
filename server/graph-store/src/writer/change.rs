//! §5.1 step 5's change: one header row and one row per operation.
//!
//! The change's canonical text is graph-contract's, not this crate's: `change_json` and
//! `manifest_change_json` are the only producers, so a change a client is sent and a change that is
//! stored cannot differ.

use tokio_postgres::Client;

use crate::error::StoreError;

/// One operation as the change log stores it.
///
/// The row is `(ws, seq, ord, op, qcoll, id, rev, text)`, and `ord` is assigned by the caller in
/// the order the change carries: every upsert in batch order, then every delete in batch order.
pub(crate) struct Op {
    /// The operation's place in the change, from zero.
    pub ord: i32,
    /// `upsert` or `delete`.
    pub op: &'static str,
    /// The record's qualified collection.
    pub qcoll: String,
    /// The record's id within that collection.
    pub id: String,
    /// The `rev` the store assigned.
    pub rev: u64,
    /// The record's canonical text, or the empty string for a delete.
    pub text: String,
}

/// One change, whole.
pub(crate) struct Change {
    /// The workspace the change belongs to.
    pub ws: String,
    /// The seq it occupies.
    pub seq: u64,
    /// The plugin the records belong to.
    pub plugin: String,
    /// When it was applied, in the wire's spelling, read with the seq.
    pub at: String,
    /// `batch` or `manifest`.
    pub kind: &'static str,
    /// The canonical change text, whose length is the header's `bytes`.
    pub text: String,
    /// Its operations, in order. A manifest change has none.
    pub ops: Vec<Op>,
}

/// Store one change: the header, then its operations.
///
/// The operations go in one at a time rather than in a single built statement, because their
/// numbers are the store's and a built statement would have to interpolate them into text to avoid
/// the driver's parameter limit of 65535 — a batch of `max_batch` records is at most 10 000
/// operations, so the limit is not reachable and the simpler statement is the one to keep.
///
/// Caveat: the header's `ops` count is written from `change.ops.len()` and not recounted from the
/// rows. Task 8 reads the operations back and compares the count it finds with this one, which is
/// how a half-written change is caught rather than served.
pub(crate) async fn insert(client: &mut Client, change: &Change) -> Result<(), StoreError> {
    client
        .execute(
            // `CAST($4 AS text)` and not `$4::text`: PostgreSQL resolves `$4::timestamptz`'s
            // parameter type to `timestamptz` itself, and the driver then refuses a `String`
            // against it. The explicit `CAST` names the parameter's type as `text`, which is what
            // it is.
            "INSERT INTO change_headers (ws, seq, plugin, at, kind, bytes, ops) \
             VALUES ($1, $2, $3, CAST($4 AS text)::timestamptz, $5, $6, $7)",
            &[
                &change.ws,
                &(change.seq as i64),
                &change.plugin,
                &change.at,
                &change.kind,
                &(change.text.len() as i64),
                &(change.ops.len() as i32),
            ],
        )
        .await?;
    for op in &change.ops {
        insert_op(client, &change.ws, change.seq, op).await?;
    }
    Ok(())
}

/// One operation row.
async fn insert_op(client: &mut Client, ws: &str, seq: u64, op: &Op) -> Result<(), StoreError> {
    client
        .execute(
            "INSERT INTO change_ops (ws, seq, ord, op, qcoll, id, rev, text) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            &[
                &ws,
                &(seq as i64),
                &op.ord,
                &op.op,
                &op.qcoll,
                &op.id,
                &(op.rev as i64),
                &op.text,
            ],
        )
        .await?;
    Ok(())
}
