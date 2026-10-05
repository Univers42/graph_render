//! `detector_bumps_on_a_point_in_time_recovery`: a PITR to a named restore point replays the
//! archive onto a base copy and then promotes itself, which moves the WAL timeline and leaves the
//! server in recovery forever unless the recovery target says `promote` (§5.3).
//!
//! # Shape
//!
//! Two `#[ignore]` phases, sequenced by row `hub-pitr` — the verbs there are fixed by the plan and
//! must not change:
//!
//! ```text
//! reset && start && copy-data && start
//!   && run --test pitr -- --ignored --exact pitr_phase_write
//!   && switch-wal && restore-data && pitr hub_pitr
//!   && run --test pitr -- --ignored --exact pitr_phase_assert
//! ```
//!
//! The base copy is taken BEFORE any of this case's data exists, so the whole case is the archive
//! doing its work: every row asserted below was replayed out of `/archive`, not read off a copy.
#![cfg(feature = "db-tests")]

mod support;

use graph_store::pool::Detector;
use support::case;

/// The phase-state file this case's two phases share.
const CASE: &str = "pitr";

/// The restore point the row's `hub-pg.sh pitr` verb names. Fixed by the row, so it is a
/// constant here and not something the phases agree on by handshake.
const RESTORE_POINT: &str = "hub_pitr";

/// Phase 1 — write A, drop a named restore point, then write B. B must never survive.
#[tokio::test]
#[ignore = "container-level: row hub-pitr runs this phase between hub-pg.sh verbs"]
async fn pitr_phase_write() {
    let mut client = case::hub().await;
    graph_store::migrate::apply(&mut client)
        .await
        .expect("migrate the row's database");
    case::seed(&mut client).await;
    let detector = Detector::new(64);
    let epoch = case::prime(&detector, &mut client).await;
    let (_, seq_a) = case::commit(&mut client, "a").await;
    let admin = case::admin().await;
    admin
        .execute("SELECT pg_create_restore_point($1)", &[&RESTORE_POINT])
        .await
        .expect("create the named restore point");
    let (_, seq_b) = case::commit(&mut client, "b").await;
    assert!(
        seq_b > seq_a,
        "B is a later batch than A, or nothing was replayed"
    );
    let (sysid, datoid) = case::identity(&mut client).await;
    case::put(
        CASE,
        &[
            ("epoch", epoch.to_string()),
            ("seq", seq_b.to_string()),
            ("hw", case::flush(&mut client).await),
            ("timeline", case::wal_timeline(&mut client).await),
            ("sysid", sysid.to_string()),
            ("datoid", datoid.to_string()),
        ],
    );
}

/// Phase 2 — the recovery stopped at the restore point, promoted itself, and the detector must
/// bump.
#[tokio::test]
#[ignore = "container-level: row hub-pitr runs this phase between hub-pg.sh verbs"]
async fn pitr_phase_assert() {
    let mut client = case::hub().await;
    case::assert_out_of_recovery(
        &mut client,
        "a named recovery target whose action is not `promote` leaves the server read-only and IN \
         RECOVERY, and §5.3 refuses that before it ever compares a key",
    )
    .await;
    assert_eq!(
        case::wal_timeline(&mut client).await,
        "00000002",
        "the promotion at the end of recovery picks the next timeline"
    );
    assert_eq!(
        case::record_ids(&mut client).await,
        vec!["a".to_string()],
        "A was committed before the restore point and B after it: the archive must replay one and \
         not the other, and replaying B would mean the target did not hold"
    );
    case::assert_bump(CASE, &mut client).await;
    let stored = case::assert_same_identity(CASE, &mut client).await;
    assert_eq!(
        stored, "00000002",
        "a PITR moves the WAL timeline and rewinds head_seq, so hub_meta carries the timeline the \
         detector compared against"
    );
}
