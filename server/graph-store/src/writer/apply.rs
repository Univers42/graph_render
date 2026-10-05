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
use crate::writer::plan::{self, Plan};
use crate::writer::retry::retried;
use crate::writer::rows::{write_counts, write_records};
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
    finish(client, req, &outcome, caps).await?;
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
    Ok(BatchOutcome {
        seq,
        applied,
        response: answer_json(seq, applied),
    })
}

/// §5.1 steps 6 and 7: prune the log past its two bounds, then store the idempotency row.
///
/// Both run inside the batch's transaction. A prune is needed only when this batch added a change,
/// since nothing else grows the log; and a prune committed on its own would leave the log shorter
/// than the bounds say whenever the batch that triggered it rolled back.
async fn finish(
    client: &mut Client,
    req: &BatchWrite,
    outcome: &BatchOutcome,
    caps: &Caps,
) -> Result<(), StoreError> {
    if outcome.applied > 0 {
        crate::retention::prune(client, &req.ws, caps.retain, caps.retain_bytes).await?;
    }
    if let Some(key) = &req.idem {
        let response = &outcome.response;
        idempotency::record(client, key, &req.ws, &req.plugin, response, outcome.seq).await?;
    }
    step::before_commit(outcome.seq).await;
    Ok(())
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
