//! The restore detector (spec §5.3, Task 6).
//!
//! # Scope of this file
//!
//! These are the cases that live entirely inside one database. The container-level ones — a
//! promotion, a point-in-time recovery, a volume snapshot, a `kill -9` — are NOT here: `scripts/orch/gr`
//! has no `docker` and no route to the Docker socket, so a test process cannot run a `hub-pg.sh`
//! verb. Those need a driver outside this container, which is a gate-row concern; see the return
//! block.
#![cfg(feature = "db-tests")]

mod support;

use std::sync::Arc;

use graph_store::pool::{Detector, DetectorOutcome, LastSeen};
use tokio_postgres::Client;

/// Cluster-wide settings (`fsync`, `full_page_writes`, a role default) are per CLUSTER, not per
/// database, and this crate runs its tests in parallel threads against one server. A case that
/// turns one off must therefore hold this for its whole body, or it silently breaks whichever
/// detector run happens to overlap it.
static CLUSTER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A migrated database, a superuser connection to it, and its URL for extra connections.
async fn db(name: &str) -> (Client, Client, String) {
    support::db::fresh_pair(name).await
}

/// A workspace the operator created, outside the guard, so the trigger moved the clock.
async fn seed_ws(client: &mut Client, id: &str) -> u64 {
    client
        .execute(
            "INSERT INTO workspaces (id, epoch) VALUES ($1, hub_next_epoch())",
            &[&id],
        )
        .await
        .expect("insert a workspace");
    client
        .query_one("SELECT epoch FROM workspaces WHERE id = $1", &[&id])
        .await
        .expect("read the epoch")
        .get::<_, i64>(0) as u64
}

/// Run the detector on `client` with `detector`, which is what `Store::client` does per connection.
async fn run(
    detector: &Detector,
    client: &mut Client,
) -> Result<DetectorOutcome, graph_store::StoreError> {
    detector.run(client).await
}

/// A detector that has already seen this database, and the epoch to compare later runs against.
///
/// WHY the epoch is read AFTER the priming run: a fresh database has no `hub_meta` row, so the
/// first run is a BUMP by §5.3's own rule, and it moves the epoch. A baseline captured before
/// priming is therefore stale by construction, and every later assertion fails against it.
async fn primed(detector: &Detector, client: &mut Client, ws: &str) -> u64 {
    run(detector, client)
        .await
        .expect("the first run records hub_meta");
    let head: i64 = client
        .query_one("SELECT head_seq FROM workspaces WHERE id = $1", &[&ws])
        .await
        .expect("head_seq")
        .get(0);
    let epoch: i64 = client
        .query_one("SELECT epoch FROM workspaces WHERE id = $1", &[&ws])
        .await
        .expect("epoch")
        .get(0);
    detector.commit_watermark("0/0", ws, epoch as u64, head as u64);
    detector.set_high_water("0/0");
    epoch as u64
}

/// A detector whose high-water is unreachable, so the next run is GUARANTEED to see a mismatch.
///
/// This is how a test arranges "a bump is warranted" without having to stage a real restore.
fn stale(detector: &Detector) {
    detector.set_high_water("FFFFFFFF/FFFFFFFF");
}

mod map;
mod matches;
mod refusals;
