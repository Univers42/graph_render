//! §5.1 step 4's writing half: the records, their `links` rows and the byte totals.
//!
//! Split out of [`super::apply`] so that module holds only the transaction's order; nothing here
//! decides anything, it writes what [`super::plan`] already decided.

use tokio_postgres::Client;

use crate::error::StoreError;
use crate::writer::BatchWrite;
use crate::writer::plan::{Plan, Planned};

/// §5.1 step 4's writing half: the records, and each one's `links` rows.
///
/// The `ON CONFLICT` clause is `DO UPDATE SET rev = records.rev + 1` rather than
/// `EXCLUDED.rev`: the increment is a fact about the stored row, and under the step-1 lock the two
/// numbers are equal, so writing the computed one back would only risk disagreeing with the row if
/// something had written it since the read.
pub(crate) async fn write_records(
    client: &mut Client,
    req: &BatchWrite,
    planned: &Plan,
) -> Result<(), StoreError> {
    for up in &planned.upserts {
        write_record(client, req, up).await?;
        write_links(client, req, up).await?;
    }
    for (qcoll, id, _) in &planned.deletes {
        client
            .execute(
                "DELETE FROM records WHERE ws = $1 AND qcoll = $2 AND id = $3",
                &[&req.ws, &qcoll, &id],
            )
            .await?;
        client
            .execute(
                "DELETE FROM links WHERE ws = $1 AND src_qcoll = $2 AND src_id = $3",
                &[&req.ws, &qcoll, &id],
            )
            .await?;
    }
    Ok(())
}

/// One record, inserted or updated.
async fn write_record(
    client: &mut Client,
    req: &BatchWrite,
    up: &Planned,
) -> Result<(), StoreError> {
    client
        .execute(
            "INSERT INTO records (ws, plugin, qcoll, id, rev, updated_at, text, text_sha256, \
             text_bytes) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
             ON CONFLICT (ws, qcoll, id) DO UPDATE SET rev = records.rev + 1, \
             updated_at = EXCLUDED.updated_at, text = EXCLUDED.text, \
             text_sha256 = EXCLUDED.text_sha256, text_bytes = EXCLUDED.text_bytes",
            &[
                &req.ws,
                &req.plugin,
                &up.qcoll,
                &up.id,
                &(up.rev as i64),
                &(up.record.updated_at as i32),
                &up.text,
                &&up.sha[..],
                &(up.text.len() as i64),
            ],
        )
        .await?;
    Ok(())
}

/// One record's `links` rows, replaced whole: the record's references are deleted and rewritten,
/// because a `links` row the record no longer holds is a reference the materializer would prune a
/// cell that is still there, and a missing one is a cell kept that should be pruned.
async fn write_links(
    client: &mut Client,
    req: &BatchWrite,
    up: &Planned,
) -> Result<(), StoreError> {
    client
        .execute(
            "DELETE FROM links WHERE ws = $1 AND src_qcoll = $2 AND src_id = $3",
            &[&req.ws, &up.qcoll, &up.id],
        )
        .await?;
    for (field, target, id) in &up.links {
        client
            .execute(
                "INSERT INTO links (ws, src_qcoll, src_id, field, target_qcoll, target_id) \
                 VALUES ($1, $2, $3, $4, $5, $6)",
                &[&req.ws, &up.qcoll, &up.id, &field, &target, &id],
            )
            .await?;
    }
    Ok(())
}

/// The two byte totals, and the plugin's `plugin_seq` when a seq was taken.
///
/// `plugin_seq` moves only on a batch that applied something, which is what `If-Match` compares
/// against (§5.1: "another plugin's writes never change it", and neither does a no-op resend of
/// this one). `doc_bytes` and `plugin_bytes` move on a no-op batch too — by zero, since the plan
/// computed the same totals — and writing them keeps the statement unconditional.
pub(crate) async fn write_counts(
    client: &mut Client,
    req: &BatchWrite,
    planned: &Plan,
    taken: Option<u64>,
) -> Result<(), StoreError> {
    client
        .execute(
            "UPDATE workspaces SET doc_bytes = $2 WHERE id = $1",
            &[&req.ws, &(planned.doc_bytes as i64)],
        )
        .await?;
    let seq = taken.map_or(0, |seq| seq as i64);
    client
        .execute(
            "UPDATE manifests SET plugin_bytes = $3, plugin_seq = \
             CASE WHEN $5 THEN $4 ELSE plugin_seq END WHERE ws = $1 AND plugin = $2",
            &[
                &req.ws,
                &req.plugin,
                &(planned.plugin_bytes as i64),
                &seq,
                &taken.is_some(),
            ],
        )
        .await?;
    Ok(())
}
