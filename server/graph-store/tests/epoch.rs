//! The epoch clock and the 24 triggers (spec H15, §5.3, Task 5).
//!
//! The tests are split across `epoch/` by concern so no file is over the 300-line house limit.
//! Every test name is unchanged, so the rows still select them.
#![cfg(feature = "db-tests")]

mod clock;
mod guard;
mod triggers;

use tokio_postgres::Client;


use graph_store::epoch::{bump_now, head_of};
use tokio_postgres::Client;

/// The clock's current value.
///
/// WHY the clock and not `workspaces.epoch`: the triggers call `hub_next_epoch()`, which advances
/// `epoch_clock.last`. A workspace row's own `epoch` column moves only on a create or a detector
/// bump. So "an event moved the epoch" means the clock moved, and "the trigger on `workspaces`
/// does not fire on its own update" means the row did not move itself.
async fn clock(client: &mut Client) -> u64 {
    client
        .query_one("SELECT last FROM epoch_clock WHERE one", &[])
        .await
        .expect("read the epoch clock")
        .get::<_, i64>(0) as u64
}

/// Park the clock far enough in the future that `last + 1` always beats the wall clock.
///
/// WHY: `hub_next_epoch()` is `greatest(last + 1, wall)`, so while the clock is behind the wall
/// clock a single draw jumps it to the wall clock and the number of draws is unobservable — the
/// value moves by however long the test took, not by how many times a trigger fired. Parking it a
/// thousand seconds ahead makes the `last + 1` term dominate, and every draw then advances `last`
/// by exactly one, which is what makes "the guard held" countable rather than merely plausible.
///
/// Caveat: this makes the clock wrong on purpose for the rest of the test, so only tests that
/// assert on *differences* may use it. `epoch_is_microseconds` must not.
async fn pin_clock_ahead(client: &mut Client) {
    client
        .batch_execute(
            "UPDATE epoch_clock SET last = \
             (extract(epoch FROM clock_timestamp()) * 1000000)::bigint + 1000000000",
        )
        .await
        .expect("park the epoch clock ahead of the wall clock");
}

/// Insert a workspace the way an operator would: outside the guard, so the trigger fires.
async fn make_ws(client: &mut Client, id: &str) {
    client
        .execute(
            "INSERT INTO workspaces (id, epoch) VALUES ($1, hub_next_epoch())",
            &[&id],
        )
        .await
        .expect("insert a workspace");
}

/// A hub write path, in §5.1 step 1's shape: an explicit transaction opening with the guard.
async fn hub_tx(client: &mut Client) {
    client.batch_execute("BEGIN").await.expect("begin");
    client
        .execute("SELECT set_config('hub.writer','1',true)", &[])
        .await
        .expect("set the writer guard");
}
