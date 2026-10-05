//! §5.1's batch, in its eight steps and nowhere else.
//!
//! The transaction is one retry around three groups — the reads, the plan, the writes — and step 8
//! is deliberately **outside** the retry. Nothing here reorders: a store that read the idempotency
//! row before taking the workspace lock, or bumped the seq before deciding whether anything
//! applied, would be a different protocol and would break the two claims H6 and §5.1's replay rest
//! on.

use graph_contract::hub::{ChangeHead, answer_json, change_json, check_change};
use tokio_postgres::Client;

use crate::error::StoreError;
use crate::store::Store;
use crate::writer::change::{Change, Op, insert};
use crate::writer::idempotency;
use crate::writer::manifest::Caps;
use crate::writer::plan::{self, Plan, Planned};
use crate::writer::retry::retried;
use crate::writer::step;
use crate::writer::{BatchOutcome, BatchWrite};

/// §5.1's batch. See [`Store::apply_batch`], which is the door callers use.
pub(crate) async fn batch(store: &Store, req: &BatchWrite) -> Result<BatchOutcome, StoreError> {
    let caps = Caps::of(store);
    let mut client = store.client().await?;
    let (outcome, epoch) = retried!(store, client, once(&mut client, req, &caps).await)?;
    step::watermark(&mut client, store.detector(), (&req.ws, epoch, outcome.seq)).await?;
    Ok(outcome)
}

/// One attempt: the whole transaction, plus the two values step 8 needs.
///
/// Returns `(answer, epoch)`. The seq is inside the answer because it *is* the answer: an all-no-op
/// batch's seq is the workspace's `head_seq`, which the step-1 lock already read.
async fn once(
    client: &mut Client,
    req: &BatchWrite,
    caps: &Caps,
) -> Result<(BatchOutcome, u64), StoreError> {
    step::begin(client).await?;
    let ws = step::lock_workspace(client, &req.ws).await?;
    super::HOOKS.pause_after_lock().await;
    if let Some(hit) = replay(client, req).await? {
        client.batch_execute("COMMIT").await?;
        return Ok((hit, ws.epoch));
    }
    let plugin = plan::read_plugin(client, &req.ws, &req.plugin).await?;
    plan::if_match(req, &plugin, ws.epoch)?;
    req.batch
        .check(&req.plugin, &req.manifest, &req.limits)
        .map_err(StoreError::Hub)?;
    let planned = plan::plan(client, req, &plugin, &ws, caps).await?;
    let outcome = write(client, req, &planned, ws.head_seq).await?;
    client.batch_execute("COMMIT").await?;
    Ok((outcome, ws.epoch))
}

/// §5.1 step 2: the stored response for this key, or `None` to carry on.
///
/// The key's length is checked here, before the lookup and not inside it, because it is a property
/// of the *request*: §5.1 caps it at 128 bytes whatever the store does with keys afterwards, so
/// `no-idem` — which turns the row's lookup and its insert off — must not turn the cap off with it.
async fn replay(client: &mut Client, req: &BatchWrite) -> Result<Option<BatchOutcome>, StoreError> {
    let Some(key) = &req.idem else {
        return Ok(None);
    };
    idempotency::check_key(&key.key)?;
    let Some(hit) = idempotency::lookup(client, key, &req.ws, &req.plugin).await? else {
        return Ok(None);
    };
    if hit.body_sha256 != key.body_sha256 {
        return Err(idempotency::different_body());
    }
    let applied = idempotency::applied_in(&hit.response)?;
    Ok(Some(BatchOutcome {
        seq: hit.seq,
        applied,
        response: hit.response,
    }))
}

/// §5.1 steps 4 and 5: write the plan, then either take a seq for it or answer without one.
///
/// An all-no-op batch still takes step 1's lock and still stores its idempotency row (§5.1), so
/// this returns an answer either way; only the middle branch — the seq, the change, the plugin's
/// `plugin_seq` — is skipped when nothing applied.
async fn write(
    client: &mut Client,
    req: &BatchWrite,
    planned: &Plan,
    head_seq: u64,
) -> Result<BatchOutcome, StoreError> {
    let applied = (planned.upserts.len() + planned.deletes.len()) as u64;
    let seq = if applied == 0 {
        head_seq
    } else {
        stamp(client, req, planned).await?
    };
    write_records(client, req, planned).await?;
    write_counts(
        client,
        req,
        planned,
        if applied == 0 { None } else { Some(seq) },
    )
    .await?;
    if let Some(key) = &req.idem {
        let response = answer_json(seq, applied);
        idempotency::record(client, key, &req.ws, &req.plugin, &response, seq).await?;
    }
    step::before_commit(seq).await;
    Ok(BatchOutcome {
        seq,
        applied,
        response: answer_json(seq, applied),
    })
}

/// §5.1 step 5: the seq, the change, and the plugin's `plugin_seq`.
async fn stamp(client: &mut Client, req: &BatchWrite, planned: &Plan) -> Result<u64, StoreError> {
    let taken = step::bump_seq(client, &req.ws).await?;
    let head = ChangeHead {
        seq: taken.seq,
        plugin: &req.plugin,
        at: &taken.at,
    };
    let upserts: Vec<_> = planned
        .upserts
        .iter()
        .map(|up| (up.record.clone(), up.rev))
        .collect();
    let text = change_json(&head, &upserts, &planned.deletes);
    check_change(&text, &req.limits)?;
    insert(
        client,
        &change_of(req, &taken.seq, &taken.at, text, planned),
    )
    .await?;
    Ok(taken.seq)
}

/// The change this batch writes: its operations in the order `change_json` lists them.
///
/// Every upsert in batch order, then every delete in batch order, and the `ord` is dense from zero
/// because `change_ops`' primary key is `(ws, seq, ord)` and the reader replays in `ord` order.
/// A delete's `text` is the empty string: the column is `NOT NULL` and a removal has no bytes.
fn change_of(req: &BatchWrite, seq: &u64, at: &str, text: String, planned: &Plan) -> Change {
    let mut ord = 0;
    let mut ops: Vec<Op> = planned
        .upserts
        .iter()
        .map(|up| Op {
            ord: next(&mut ord),
            op: "upsert",
            qcoll: up.qcoll.clone(),
            id: up.id.clone(),
            rev: up.rev,
            text: up.text.clone(),
        })
        .collect();
    ops.extend(planned.deletes.iter().map(|(qcoll, id, rev)| Op {
        ord: next(&mut ord),
        op: "delete",
        qcoll: qcoll.clone(),
        id: id.clone(),
        rev: *rev,
        text: String::new(),
    }));
    Change {
        ws: req.ws.clone(),
        seq: *seq,
        plugin: req.plugin.clone(),
        at: at.to_owned(),
        kind: "batch",
        text,
        ops,
    }
}

/// The next dense operation number.
fn next(ord: &mut i32) -> i32 {
    let at = *ord;
    *ord += 1;
    at
}

/// §5.1 step 4's writing half: the records, and each one's `links` rows.
///
/// The `ON CONFLICT` clause is `DO UPDATE SET rev = records.rev + 1` rather than
/// `EXCLUDED.rev`: the increment is a fact about the stored row, and under the step-1 lock the two
/// numbers are equal, so writing the computed one back would only risk disagreeing with the row if
/// something had written it since the read.
async fn write_records(
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
async fn write_counts(
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
