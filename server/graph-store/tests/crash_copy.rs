//! `detector_bumps_on_a_small_gap_crash_consistent_copy`: the container is SIGKILLed, the volume
//! is copied anyway, the hub commits one more batch on the volume it still has, and the copy is
//! restored over the top. The restored database is a small gap behind the hub's last-seen map, and
//! the identity keys never moved at all (§5.3).
//!
//! # Shape
//!
//! Three `#[ignore]` phases, sequenced by row `hub-crash-copy`. Only `scripts/orch/hub-pg.sh` can
//! kill a container or copy a volume, and a `gr` test container has no `docker`:
//!
//! ```text
//! reset && start && run --test crash_copy -- --ignored --exact crash_copy_phase_write
//!   && kill && copy-data && start
//!   && run --test crash_copy -- --ignored --exact crash_copy_phase_commit
//!   && restore-data && start && switch-wal
//!   && run --test crash_copy -- --ignored --exact crash_copy_phase_assert
//! ```
//!
//! WHY `switch-wal` before the reconnect: it is the LSN half of the case. The spec measured that a
//! restored server whose WAL has been pushed forward by a segment switch reads ABOVE the hub's
//! high-water, which is exactly when the last-seen map is the only signal left. The assert phase
//! reports which of the two fired rather than assuming.
#![cfg(feature = "db-tests")]

mod support;

use graph_store::pool::Detector;
use support::case;
use tokio_postgres::Client;

/// The phase-state file this case's three phases share.
const CASE: &str = "crash-copy";

/// Phase 1 — a committed batch, a `hub_meta` that matches the volume, and a WAL segment switch so
/// the copy that follows a `kill -9` is a volume PostgreSQL can start from.
#[tokio::test]
#[ignore = "container-level: row hub-crash-copy runs this phase between hub-pg.sh verbs"]
async fn crash_copy_phase_write() {
    let mut client = case::hub().await;
    graph_store::migrate::apply(&mut client)
        .await
        .expect("migrate the row's database");
    case::seed(&mut client).await;
    let detector = Detector::new(64);
    let epoch = case::prime(&detector, &mut client).await;
    let (_, seq) = case::commit(&mut client, "a").await;
    let (sysid, datoid) = case::identity(&mut client).await;
    case::quiesce().await;
    case::put(
        CASE,
        &[
            ("epoch", epoch.to_string()),
            ("seq", seq.to_string()),
            ("hw", case::flush(&mut client).await),
            ("timeline", case::wal_timeline(&mut client).await),
            ("sysid", sysid.to_string()),
            ("datoid", datoid.to_string()),
        ],
    );
}

/// Phase 2 — the copy exists; the hub, which never stopped, commits one more batch.
#[tokio::test]
#[ignore = "container-level: row hub-crash-copy runs this phase between hub-pg.sh verbs"]
async fn crash_copy_phase_commit() {
    let mut client = case::hub().await;
    let (_, seq) = case::commit(&mut client, "b").await;
    assert_eq!(seq, 2, "the hub commits exactly one batch after the copy");
    let (sysid, datoid) = case::identity(&mut client).await;
    case::put(
        CASE,
        &[
            ("epoch", case::num(CASE, "epoch").to_string()),
            ("seq", seq.to_string()),
            ("hw", case::flush(&mut client).await),
            ("timeline", case::wal_timeline(&mut client).await),
            ("sysid", sysid.to_string()),
            ("datoid", datoid.to_string()),
        ],
    );
}

/// Phase 3 — the crash copy is back, and the detector must bump.
#[tokio::test]
#[ignore = "container-level: row hub-crash-copy runs this phase between hub-pg.sh verbs"]
async fn crash_copy_phase_assert() {
    let mut client = case::hub().await;
    let (_, seq) = case::head(&mut client).await;
    assert_eq!(
        seq, 1,
        "the restored copy predates the hub's second batch, so its head_seq is below the map entry"
    );
    report_the_leg(&mut client).await;
    case::assert_bump(CASE, &mut client).await;
    let stored = case::assert_same_identity(CASE, &mut client).await;
    assert_eq!(
        stored,
        case::get(CASE, "timeline"),
        "a crash-consistent copy is the same cluster, the same database and the same timeline, so \
         none of §5.3's three identity keys moved and only the high-water or the map can catch it"
    );
}

/// Print which of §5.3's two non-identity signals caught the restore, for the gate log.
///
/// Caveat: which one fires is a property of the gap, not of the code — a gap wider than one WAL
/// segment lets the high-water see it. The assertions do not read this: the bump and the
/// `head_seq` check stand on their own, and this line is the measurement beside them.
async fn report_the_leg(client: &mut Client) {
    let hw = case::get(CASE, "hw");
    let flush = case::flush(client).await;
    let leg = if flush < hw {
        "high-water"
    } else {
        "last-seen map"
    };
    println!("crash-copy: restored flush LSN {flush} against the hub's high-water {hw}");
    println!("crash-copy: the {leg} leg fired; the map leg is exact whenever the LSN leg does not");
}
