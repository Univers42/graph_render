//! The idempotency sweeper (§5.1): idempotency rows older than `GRAPH_HUB_IDEM_TTL_MS` go, in
//! bounded batches, each in its own short transaction.
//!
//! WHY bounded and not one `DELETE`: one statement over a day of keys holds its row locks and its
//! WAL for as long as the whole delete takes, and every writer that replays a key waits on it.
//! A batch of `batch` rows commits quickly and lets the writers through between batches.
//!
//! The sweeper opens with `hub.writer` set, like every hub write path, so the epoch triggers stay
//! quiet: removing an expired key changes nothing a reader's cursor depends on.

use crate::config::StoreConfig;
use crate::error::StoreError;
use crate::store::Store;

/// The oldest `$2` rows created more than `$1` milliseconds ago.
///
/// Caveat: PostgreSQL has no `DELETE … LIMIT`, so the bounded delete names its rows in a subquery
/// ordered by `created_at` (index `idempotency_age`). A row inserted concurrently with an older
/// `created_at` (an operator's clock moved back) is not missed for good: it waits for the next
/// batch or the next run.
const SWEEP: &str = "DELETE FROM idempotency WHERE (ws, plugin, key) IN (\
     SELECT ws, plugin, key FROM idempotency \
     WHERE created_at < clock_timestamp() - $1::bigint * interval '1 millisecond' \
     ORDER BY created_at LIMIT $2)";

/// Delete at most `batch` idempotency rows older than `older_than_ms`, in one transaction, and
/// return how many went. A caller sweeps a backlog by calling again until this returns less than
/// `batch`.
pub async fn run(store: &Store, older_than_ms: u64, batch: u64) -> Result<u64, StoreError> {
    let age = i64::try_from(older_than_ms).unwrap_or(i64::MAX);
    let limit = i64::try_from(batch).unwrap_or(i64::MAX);
    let client = store.client().await?;
    client
        .batch_execute("BEGIN; SELECT set_config('hub.writer','1',true)")
        .await?;
    let gone = client.execute(SWEEP, &[&age, &limit]).await?;
    client.batch_execute("COMMIT").await?;
    Ok(gone)
}

/// How long the hub waits between two sweeps.
///
/// Caveat: a key expires up to one interval late (10 minutes at the default), so the table holds
/// at most `idem_ttl_ms + sweeper_interval_ms` of keys. A replay inside that window is answered
/// from the stored row, which is the safe direction: a late expiry never applies a batch twice.
pub fn interval_ms(cfg: &StoreConfig) -> u64 {
    cfg.sweeper_interval_ms
}
