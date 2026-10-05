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
pub use support::workspace::{as_writer, batch_write, epoch_of, ready, write_tasks};

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
