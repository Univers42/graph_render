//! Opening a connection, and the restore detector §5.3 specifies.
//!
//! The refusals run first and *outside* a transaction, because each one is a property of the
//! server rather than of a transaction: a database in recovery, `fsync` or `full_page_writes`
//! off, and a `hub.writer` default on the hub's own role all mean the store cannot honour the
//! contract on this database at all.

use tokio_postgres::Client;

use crate::error::StoreError;
use crate::pool::Detector;

/// The advisory lock the detector takes, so two hubs sharing a database serialise here.
///
/// A constant, not a derived value: the two hubs must agree without exchanging anything.
pub const HUB_LOCK: i64 = 8_675_309_001;

/// Open one connection and run the detector on it before it may be used.
pub async fn open(url: &str, detector: &Detector) -> Result<Client, StoreError> {
    let (client, connection) = tokio_postgres::connect(url, tokio_postgres::NoTls).await?;
    tokio::spawn(async move {
        // The connection task must outlive the handle, or the socket closes under the next query.
        let _ = connection.await;
    });
    let mut handle = client;
    detector.run(&mut handle).await?;
    Ok(handle)
}

/// The three refusals, in §5.3's order. Read outside a transaction.
pub(crate) async fn refuse_if_unusable(client: &mut Client) -> Result<(), StoreError> {
    let row = client.query_one("SELECT pg_is_in_recovery()", &[]).await?;
    if row.get::<_, bool>(0) {
        return Err(StoreError::NoDatabase);
    }
    refuse_if_off(client, "fsync").await?;
    refuse_if_off(client, "full_page_writes").await?;
    refuse_default_writer(client).await
}

/// Refuse when `name` reads `off` outside a transaction.
async fn refuse_if_off(client: &mut Client, name: &str) -> Result<(), StoreError> {
    let sql = format!("SELECT current_setting('{name}', true)");
    let row = client.query_one(sql.as_str(), &[]).await?;
    let value: Option<String> = row.get(0);
    if value.as_deref() == Some("off") {
        return Err(StoreError::NoDatabase);
    }
    Ok(())
}

/// Refuse when the hub's own role carries a `hub.writer` default.
///
/// §5.3 reads this outside a transaction on purpose: `SET LOCAL` inside the store's own write
/// transactions is invisible here, but a role-level default would silently suppress the epoch
/// triggers for every write the role ever makes.
async fn refuse_default_writer(client: &mut Client) -> Result<(), StoreError> {
    let row = client
        .query_one("SELECT current_setting('hub.writer', true)", &[])
        .await?;
    let value: Option<String> = row.get(0);
    match value.as_deref() {
        None | Some("") => Ok(()),
        Some(_) => Err(StoreError::NoDatabase),
    }
}

/// The refusals are about OPENING a connection; the comparison and the bump are in
/// `detect.rs`.
pub(crate) use crate::pool::detect::run_detector;
