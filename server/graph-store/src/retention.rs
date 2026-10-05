//! §5.1 step 6: the change log's two bounds, `GRAPH_HUB_RETAIN` changes and
//! `GRAPH_HUB_RETAIN_BYTES` bytes per workspace, enforced inside the batch that grew it.
//!
//! WHY inside the batch's transaction and never in a job of its own (Decision 9): a prune that
//! commits on its own can land while the batch that triggered it rolls back, which leaves a log
//! shorter than its bounds and a cursor that is `Gone` although nothing replaced it. A cursor below
//! what is kept is answered by `changes::page` as [`StoreError::Gone`], so this module only deletes.
//!
//! Caveat: the cut is found by scanning the log newest first until either running total crosses
//! its bound, so every committing batch reads up to `retain` header rows: O(kept) per commit, about
//! 100 000 rows at the default once a workspace's log is full. A batch on a short log reads a short
//! log. The upgrade path is a `log_bytes` column on `workspaces`, kept by `change::insert` and by
//! this prune, which turns both checks into O(1) and costs a migration.

use tokio_postgres::Client;

use crate::error::StoreError;

/// The newest seq that must go: the first one, walking down from the head, past which the kept
/// changes would number more than `$2` or weigh more than `$3` bytes.
///
/// The frame is the default `RANGE UNBOUNDED PRECEDING .. CURRENT ROW`, and `seq` is unique per
/// workspace, so each row's running totals count exactly the changes newer than or equal to it.
const CUT: &str = "SELECT seq FROM (\
     SELECT seq, count(*) OVER w AS n, sum(bytes) OVER w AS b \
     FROM change_headers WHERE ws = $1 WINDOW w AS (ORDER BY seq DESC)) t \
     WHERE n > $2 OR b > $3::bigint ORDER BY seq DESC LIMIT 1";

/// Delete the oldest changes of `ws` until at most `retain` of them, and at most `retain_bytes`
/// bytes of them, are left; return how many changes went.
///
/// `client` must be inside the caller's open transaction with `hub.writer` set: this function
/// never begins, commits or rolls back. A change's operations go first, then its header, so the
/// two tables never disagree inside the transaction either.
pub async fn prune(
    client: &Client,
    ws: &str,
    retain: u64,
    retain_bytes: u64,
) -> Result<u64, StoreError> {
    let bounds = [retain, retain_bytes].map(|b| i64::try_from(b).unwrap_or(i64::MAX));
    let Some(row) = client
        .query_opt(CUT, &[&ws, &bounds[0], &bounds[1]])
        .await?
    else {
        return Ok(0);
    };
    let cut: i64 = row.get(0);
    own_transaction_break(
        client,
        "COMMIT; BEGIN; SELECT set_config('hub.writer','1',true)",
    )
    .await?;
    client
        .execute(
            "DELETE FROM change_ops WHERE ws = $1 AND seq <= $2",
            &[&ws, &cut],
        )
        .await?;
    let gone = client
        .execute(
            "DELETE FROM change_headers WHERE ws = $1 AND seq <= $2",
            &[&ws, &cut],
        )
        .await?;
    own_transaction_break(
        client,
        "COMMIT; BEGIN; SELECT set_config('hub.writer','1',true), \
         set_config('synchronous_commit','on',true)",
    )
    .await?;
    Ok(gone)
}

/// `prune-own-transaction`: commit what the caller has so far and run the deletes on their own,
/// which is exactly what Decision 9 forbids and what `prune_never_runs_in_its_own_transaction`
/// must catch. Without the `negctl` feature this is a no-op.
async fn own_transaction_break(client: &Client, sql: &str) -> Result<(), StoreError> {
    if crate::breaks::on("prune-own-transaction") {
        client.batch_execute(sql).await?;
    }
    Ok(())
}
