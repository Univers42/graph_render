//! `detector_bumps_on_a_volume_snapshot_under_a_running_hub`: a volume copied out from under a
//! hub keeps every one of §5.3's three identity keys, so the only thing left to catch it is the
//! high-water — the restored server's WAL is behind the flush LSN the hub read after its last
//! commit (§5.3; the last-seen map is the second signal and is exact here).
//!
//! # Shape
//!
//! Three `#[ignore]` phases, sequenced by row `hub-snapshot`. Only `scripts/orch/hub-pg.sh` can
//! copy a volume, and a `gr` test container has no `docker`:
//!
//! ```text
//! reset && start && run --test snapshot -- --ignored --exact snapshot_phase_write
//!   && copy-data && start
//!   && run --test snapshot -- --ignored --exact snapshot_phase_after_copy
//!   && restore-data && start
//!   && run --test snapshot -- --ignored --exact snapshot_phase_assert
//! ```
//!
//! The middle phase is the point: the hub commits AFTER the copy is taken and never stops, so the
//! phase state handed to the assert phase is a hub's real memory (a high-water and a map entry)
//! that no longer matches the volume it is about to reconnect to.
#![cfg(feature = "db-tests")]

mod support;

use graph_store::pool::Detector;
use support::case;

/// The phase-state file this case's three phases share.
const CASE: &str = "snapshot";

/// Phase 1 — a committed batch and a `hub_meta` that matches this volume exactly.
#[tokio::test]
#[ignore = "container-level: row hub-snapshot runs this phase between hub-pg.sh verbs"]
async fn snapshot_phase_write() {
    let mut client = case::hub().await;
    graph_store::migrate::apply(&mut client)
        .await
        .expect("migrate the row's database");
    case::seed(&mut client).await;
    let detector = Detector::new(64);
    let epoch = case::prime(&detector, &mut client).await;
    let (_, seq) = case::commit(&mut client, "a").await;
    let (sysid, datoid) = case::identity(&mut client).await;
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

/// Phase 2 — the copy exists; the hub, still running, commits on top of the original volume.
#[tokio::test]
#[ignore = "container-level: row hub-snapshot runs this phase between hub-pg.sh verbs"]
async fn snapshot_phase_after_copy() {
    let mut client = case::hub().await;
    let (_, seq) = case::commit(&mut client, "b").await;
    let (sysid, datoid) = case::identity(&mut client).await;
    // The epoch is unchanged by design: the commit carries the writer guard (§5.1 step 2), so the
    // state recorded here is the hub's map entry and nothing else moved.
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

/// Phase 3 — the snapshot is back under the hub, and the detector must bump on the high-water.
#[tokio::test]
#[ignore = "container-level: row hub-snapshot runs this phase between hub-pg.sh verbs"]
async fn snapshot_phase_assert() {
    let mut client = case::hub().await;
    let hw = case::get(CASE, "hw");
    let flush = case::flush(&mut client).await;
    assert!(
        flush < hw,
        "a volume copied before the hub's last commit starts behind that commit's flush LSN, so \
         the high-water leg is the one that fires; if this ever reads the other way the case has \
         stopped being a snapshot test"
    );
    assert_eq!(
        case::record_ids(&mut client).await,
        vec!["a".to_string()],
        "the restored volume predates the batch the hub committed after the copy"
    );
    case::assert_bump(CASE, &mut client).await;
    let stored = case::assert_same_identity(CASE, &mut client).await;
    assert_eq!(
        stored,
        case::get(CASE, "timeline"),
        "a volume snapshot changes none of §5.3's three identity keys — that is why the \
         high-water and the map are what catch it"
    );
}
