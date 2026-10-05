//! The change feed and the records page (spec §5.3, Task 8).
//!
//! Every case writes through the store itself, so the feed is read back from what the writer
//! stored and never from rows a case inserted by hand. The one case that edits the log does it as
//! the writer (`hub.writer = '1'`), so the epoch trigger leaves the workspace's epoch alone.
#![cfg(feature = "db-tests")]

#[path = "changes/cursor.rs"]
mod cursor;
#[path = "changes/page.rs"]
mod page;
#[path = "changes/records.rs"]
mod records;

mod support;

use graph_contract::hub::Cursor;
use graph_store::Store;
use graph_store::changes::{ChangePage, ChangesReq};
use graph_store::error::StoreError;
use tokio_postgres::Client;

pub use support::fixture::batch_of;
pub use support::workspace::{batch_write, ready};

/// The reader's seam is a file every page read in this binary checks, so the case that arms it
/// takes this for writing and every other page read takes it for reading.
pub static SEAM: tokio::sync::RwLock<()> = tokio::sync::RwLock::const_new(());

/// A request with room for every change a case writes.
pub fn since(cursor: Cursor) -> ChangesReq {
    req(cursor, 1000, 1 << 30)
}

/// A request for `ws` after `since`.
pub fn req(since: Cursor, limit: u64, max_bytes: u64) -> ChangesReq {
    ChangesReq {
        ws: "ws".to_owned(),
        since,
        limit,
        max_bytes,
    }
}

/// One page, read while no case holds the seam armed.
pub async fn read(store: &Store, request: &ChangesReq) -> Result<ChangePage, StoreError> {
    let _seam = SEAM.read().await;
    graph_store::changes::page(store, request).await
}

/// One batch per id in `ids`, each upserting task `id`: seqs 2, 3, … after the registration.
pub async fn write_tasks(store: &Store, ids: &[&str]) {
    for id in ids {
        let cells = format!(r#""name":"Task {id}""#);
        let outcome = store
            .apply_batch(&batch_write(
                "ws",
                "tracker",
                batch_of(&[("task", id, 1, &cells)], &[]),
            ))
            .await;
        assert!(outcome.is_ok(), "batch {id} applies: {outcome:?}");
    }
}

/// The workspace's epoch.
pub async fn epoch_of(client: &Client) -> u64 {
    let epoch: i64 = client
        .query_one("SELECT epoch FROM workspaces WHERE id = 'ws'", &[])
        .await
        .expect("read the epoch")
        .get(0);
    epoch as u64
}

/// Each stored header's `(seq, bytes)`, in seq order.
pub async fn header_bytes(client: &Client) -> Vec<(u64, u64)> {
    client
        .query(
            "SELECT seq, bytes FROM change_headers WHERE ws = 'ws' ORDER BY seq",
            &[],
        )
        .await
        .expect("read the headers")
        .iter()
        .map(|row| (row.get::<_, i64>(0) as u64, row.get::<_, i64>(1) as u64))
        .collect()
}

/// Run `sql` as the writer, so the epoch trigger does not count it as a foreign edit.
pub async fn as_writer(client: &Client, sql: &str) {
    client
        .batch_execute(&format!(
            "BEGIN; SELECT set_config('hub.writer', '1', true); {sql}; COMMIT"
        ))
        .await
        .unwrap_or_else(|e| panic!("run as the writer: {sql}: {e}"));
}

/// Drop changes `1..=seq` from the log, the way retention does.
pub async fn prune_through(client: &Client, seq: u64) {
    as_writer(
        client,
        &format!(
            "DELETE FROM change_ops WHERE ws = 'ws' AND seq <= {seq}; \
             DELETE FROM change_headers WHERE ws = 'ws' AND seq <= {seq}"
        ),
    )
    .await;
}

/// The seqs a page carries.
pub fn seqs_of(page: &ChangePage) -> Vec<u64> {
    page.changes.iter().map(|c| c.seq).collect()
}
