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
        client.batch_execute(&drop_all()).await?;
    }
    if breaks::on("trigger-enable-origin") {
        client.batch_execute(&alters("ENABLE")).await?;
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

/// The four event suffixes, in the order the triggers are created.
const EVENTS: [&str; 4] = ["ins", "upd", "del", "trunc"];

/// One `ALTER TABLE` per trigger, for all 24.
///
/// WHY not a comma list: PostgreSQL's `ALTER TABLE ... { ENABLE | DISABLE } TRIGGER` takes exactly
/// one trigger name. A list is a syntax error (42601), which would make every control red for a
/// reason that is not the one it is meant to prove.
fn alters(action: &str) -> String {
    let mut sql = String::new();
    for (table, _) in TRIGGER_TABLES {
        for event in EVENTS {
            sql.push_str(&format!("ALTER TABLE {table} {action} TRIGGER hub_{table}_{event};"));
        }
    }
    sql
}

/// `DROP TRIGGER` for all 24, each preceded by a disable so an `ENABLE ALWAYS` trigger is not
/// firing while the set is torn down.
fn drop_all() -> String {
    let mut sql = alters("DISABLE");
    for (table, _) in TRIGGER_TABLES {
        for event in EVENTS {
            sql.push_str(&format!("DROP TRIGGER hub_{table}_{event} ON {table};"));
        }
    }
    sql
}

/// A bare `ENABLE TRIGGER` on exactly one, so the catalog assertion and one replica-role
/// write both fail.
///
/// WHY bare `ENABLE` and not `ENABLE ORIGIN`: `ENABLE ORIGIN TRIGGER` is not a PostgreSQL
/// grammar production. Origin is the state a trigger is created in, and the bare form is how
/// you set it, which is also what sets `pg_trigger.tgenabled` to `O` rather than `A`.
const ENABLE_ORIGIN_ONE: &str = "ALTER TABLE records ENABLE TRIGGER hub_records_ins;";

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
