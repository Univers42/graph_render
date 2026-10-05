//! `detector_bumps_on_a_promotion`: promoting a standby moves the WAL timeline at once, and
//! §5.3 reads that move from `pg_walfile_name(pg_current_wal_lsn())` and NOT from
//! `pg_control_checkpoint()`, which still names the old timeline until the next checkpoint.
//!
//! # Shape
//!
//! Two `#[ignore]` phases, sequenced by row `hub-promotion` — only `scripts/orch/hub-pg.sh` can
//! promote a standby, and a `gr` test container has no `docker`:
//!
//! ```text
//! reset && start && run --test promotion -- --ignored --exact promotion_phase_write
//!   && replica && replica-promote
//!   && run --test promotion -- --ignored --exact promotion_phase_assert
//! ```
//!
//! `hub-pg.sh replica` replaces the container and a replacement gets a new bridge IP, so the
//! assert phase reconnects through the URL file `tests/support/db.rs` re-reads on every call.
#![cfg(feature = "db-tests")]

mod support;

use graph_store::pool::{Detector, DetectorOutcome};
use support::case;

/// The phase-state file this case's two phases share.
const CASE: &str = "promotion";

/// Phase 1 — a live primary with one batch and a `hub_meta` that names timeline 1.
#[tokio::test]
#[ignore = "container-level: row hub-promotion runs this phase between hub-pg.sh verbs"]
async fn promotion_phase_write() {
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

/// Phase 2 — after the promotion, the detector must bump, and the key it stored must be the
/// WAL-file timeline.
#[tokio::test]
#[ignore = "container-level: row hub-promotion runs this phase between hub-pg.sh verbs"]
async fn promotion_phase_assert() {
    let mut client = case::hub().await;
    assert!(
        !case::in_recovery(&mut client).await,
        "replica-promote must leave the server out of recovery: §5.3 refuses a database in \
         recovery, so a case that ended its row here would have proved the refusal, not the bump"
    );
    let timeline = case::wal_timeline(&mut client).await;
    assert_eq!(
        timeline, "00000002",
        "a promotion picks the next timeline, and it is the timeline key that carries it"
    );
    let before = case::num(CASE, "epoch");
    let outcome = case::hub_detector(CASE)
        .run(&mut client)
        .await
        .expect("the detector runs on the promoted server");
    assert_eq!(
        outcome,
        DetectorOutcome::Bumped { workspaces: 1 },
        "a promotion moves the WAL timeline, so hub_meta's key is stale and §5.3 must bump"
    );
    assert!(
        case::head(&mut client).await.0 > before,
        "the bump draws a fresh epoch, and every workspace must hold it"
    );
    let (sysid, stored, datoid) = case::stored_identity(&mut client).await;
    assert_eq!(
        stored, timeline,
        "hub_meta.timeline must be the WAL-file timeline key: the break `checkpoint-timeline` \
         reads pg_control_checkpoint(), which lags a promotion and prints the id unpadded, so the \
         key the detector compares with is not the key it stores"
    );
    assert_eq!(
        (sysid, datoid),
        (case::num(CASE, "sysid"), case::num(CASE, "datoid") as u32),
        "a promotion changes neither the system identifier nor the database oid, so those two \
         keys cannot be what caught it"
    );
}
