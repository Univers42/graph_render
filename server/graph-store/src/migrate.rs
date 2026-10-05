//! Migrations: SQL files under `sql/`, embedded with `include_str!`, applied in file-name order.
//!
//! Each file is applied in its own transaction and recorded in `hub_migrations` with its
//! SHA-256. A file whose bytes changed after it was applied is **refused**, which is how the
//! schema stays pinned to the code that was written against it. There is no migration crate: the
//! files are the schema, and they are readable.

use sha2::{Digest, Sha256};
use tokio_postgres::Client;

use crate::breaks;
use crate::error::{DbError, StoreError};

/// Every migration, in apply order: `(name, bytes)`.
pub const FILES: [(&str, &str); 2] = [
    ("0001_schema.sql", include_str!("../sql/0001_schema.sql")),
    ("0002_epoch.sql", include_str!("../sql/0002_epoch.sql")),
];

/// Every migration's `(name, sha256)`, in apply order.
pub fn manifest() -> Vec<(String, String)> {
    FILES
        .iter()
        .map(|(name, body)| ((*name).to_string(), sha256_hex(body.as_bytes())))
        .collect()
}

/// The SHA-256 of `bytes`, as hex.
///
/// Hand-rolled rather than a hex crate: the store needs one function, and a dependency that
/// formats a digest is a dependency to keep pinned for the rest of the slice's life.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Apply every migration that is not yet recorded, and return how many were applied.
///
/// A second call applies zero and changes no row. The `hub_migrations` table itself is created
/// on a connection with no transaction open, because it has to exist before anything can be
/// recorded in it.
pub async fn apply(client: &mut Client) -> Result<u32, StoreError> {
    client
        .batch_execute(
            "CREATE TABLE IF NOT EXISTS hub_migrations (name text PRIMARY KEY, \
             sha256 text NOT NULL, \
             applied_at timestamptz NOT NULL DEFAULT clock_timestamp())",
        )
        .await?;
    let mut applied = 0;
    for (name, body) in FILES {
        if apply_one(client, name, body).await? {
            applied += 1;
        }
    }
    epoch_breaks(client).await?;
    Ok(applied)
}

/// The negative controls' trigger-catalog breaks, applied after the DDL and never recorded.
///
/// WHY here and not in `0002_epoch.sql`: a break must change what the server *has*, without
/// changing a file's bytes, or the migration-hash guard would refuse the database and the control
/// would go red for the wrong reason.
///
/// Without the `negctl` feature this is a no-op that the optimizer removes.
async fn epoch_breaks(client: &mut Client) -> Result<(), StoreError> {
    if breaks::on("no-trigger") {
        client.batch_execute(DROP_ALL).await?;
    }
    if breaks::on("trigger-enable-origin") {
        client.batch_execute(ENABLE_ORIGIN_ALL).await?;
    }
    if breaks::on("one-trigger-origin") {
        client.batch_execute(ENABLE_ORIGIN_ONE).await?;
    }
    Ok(())
}

/// Every trigger this slice names, and the tables they sit on.
pub const TRIGGER_TABLES: [(&str, &str); 6] = [
    ("workspaces", "id"),
    ("manifests", "ws"),
    ("records", "ws"),
    ("links", "ws"),
    ("change_headers", "ws"),
    ("change_ops", "ws"),
];

/// `DROP TRIGGER` for all 24, in the order the events are created.
const DROP_ALL: &str = concat!(
    "ALTER TABLE workspaces DISABLE TRIGGER hub_workspaces_ins, hub_workspaces_upd, hub_workspaces_del, hub_workspaces_trunc;",
    "DROP TRIGGER hub_workspaces_ins ON workspaces;",
    "DROP TRIGGER hub_workspaces_upd ON workspaces;",
    "DROP TRIGGER hub_workspaces_del ON workspaces;",
    "DROP TRIGGER hub_workspaces_trunc ON workspaces;",
    "ALTER TABLE manifests DISABLE TRIGGER hub_manifests_ins, hub_manifests_upd, hub_manifests_del, hub_manifests_trunc;",
    "DROP TRIGGER hub_manifests_ins ON manifests;",
    "DROP TRIGGER hub_manifests_upd ON manifests;",
    "DROP TRIGGER hub_manifests_del ON manifests;",
    "DROP TRIGGER hub_manifests_trunc ON manifests;",
    "ALTER TABLE records DISABLE TRIGGER hub_records_ins, hub_records_upd, hub_records_del, hub_records_trunc;",
    "DROP TRIGGER hub_records_ins ON records;",
    "DROP TRIGGER hub_records_upd ON records;",
    "DROP TRIGGER hub_records_del ON records;",
    "DROP TRIGGER hub_records_trunc ON records;",
    "ALTER TABLE links DISABLE TRIGGER hub_links_ins, hub_links_upd, hub_links_del, hub_links_trunc;",
    "DROP TRIGGER hub_links_ins ON links;",
    "DROP TRIGGER hub_links_upd ON links;",
    "DROP TRIGGER hub_links_del ON links;",
    "DROP TRIGGER hub_links_trunc ON links;",
    "ALTER TABLE change_headers DISABLE TRIGGER hub_change_headers_ins, hub_change_headers_upd, hub_change_headers_del, hub_change_headers_trunc;",
    "DROP TRIGGER hub_change_headers_ins ON change_headers;",
    "DROP TRIGGER hub_change_headers_upd ON change_headers;",
    "DROP TRIGGER hub_change_headers_del ON change_headers;",
    "DROP TRIGGER hub_change_headers_trunc ON change_headers;",
    "ALTER TABLE change_ops DISABLE TRIGGER hub_change_ops_ins, hub_change_ops_upd, hub_change_ops_del, hub_change_ops_trunc;",
    "DROP TRIGGER hub_change_ops_ins ON change_ops;",
    "DROP TRIGGER hub_change_ops_upd ON change_ops;",
    "DROP TRIGGER hub_change_ops_del ON change_ops;",
    "DROP TRIGGER hub_change_ops_trunc ON change_ops;"
);

/// `ENABLE ORIGIN` on all 24: a replica-role write then moves no epoch at all.
const ENABLE_ORIGIN_ALL: &str = concat!(
    "ALTER TABLE workspaces ENABLE ORIGIN TRIGGER hub_workspaces_ins, hub_workspaces_upd, hub_workspaces_del, hub_workspaces_trunc;",
    "ALTER TABLE manifests ENABLE ORIGIN TRIGGER hub_manifests_ins, hub_manifests_upd, hub_manifests_del, hub_manifests_trunc;",
    "ALTER TABLE records ENABLE ORIGIN TRIGGER hub_records_ins, hub_records_upd, hub_records_del, hub_records_trunc;",
    "ALTER TABLE links ENABLE ORIGIN TRIGGER hub_links_ins, hub_links_upd, hub_links_del, hub_links_trunc;",
    "ALTER TABLE change_headers ENABLE ORIGIN TRIGGER hub_change_headers_ins, hub_change_headers_upd, hub_change_headers_del, hub_change_headers_trunc;",
    "ALTER TABLE change_ops ENABLE ORIGIN TRIGGER hub_change_ops_ins, hub_change_ops_upd, hub_change_ops_del, hub_change_ops_trunc;"
);

/// `ENABLE ORIGIN` on exactly one, so the catalog assertion and one replica-role write both fail.
const ENABLE_ORIGIN_ONE: &str =
    "ALTER TABLE records ENABLE ORIGIN TRIGGER hub_records_ins;";

/// Apply one file. `Ok(true)` when it was applied, `Ok(false)` when it was already recorded with
/// the same hash.
async fn apply_one(client: &mut Client, name: &str, body: &str) -> Result<bool, StoreError> {
    let hash = sha256_hex(body.as_bytes());
    let recorded = client
        .query_opt(
            "SELECT sha256 FROM hub_migrations WHERE name = $1",
            &[&name],
        )
        .await?;
    if let Some(row) = recorded {
        let have: String = row.get(0);
        if have == hash {
            return Ok(false);
        }
        return Err(StoreError::Db(DbError::migration_hash(name)));
    }
    client.batch_execute("BEGIN").await?;
    client.batch_execute(body).await?;
    client
        .execute(
            "INSERT INTO hub_migrations (name, sha256) VALUES ($1, $2) \
             ON CONFLICT (name) DO NOTHING",
            &[&name, &hash],
        )
        .await?;
    client.batch_execute("COMMIT").await?;
    Ok(true)
}
