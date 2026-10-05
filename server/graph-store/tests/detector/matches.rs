use super::*;

/// No `hub_meta` row at all is a mismatch: every workspace draws a fresh epoch.
#[tokio::test]
async fn detector_bumps_on_empty_hub_meta() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, _, _) = db("detector_bumps_on_empty_hub_meta").await;
    let before = seed_ws(&mut client, "ws").await;
    // The seed left `hub_meta` empty, which is exactly the case: a first run has never seen this
    // database, so it cannot recognise it either.
    let detector = Detector::new(64);
    let outcome = run(&detector, &mut client).await.expect("run");
    assert!(
        matches!(outcome, DetectorOutcome::Bumped { workspaces: 1 }),
        "an empty hub_meta did not bump: {outcome:?}"
    );
    let after: i64 = client
        .query_one("SELECT epoch FROM workspaces WHERE id = 'ws'", &[])
        .await
        .expect("read the epoch")
        .get(0);
    assert!(after as u64 > before, "the bump did not move the epoch");
    // `hub_meta` is written by the same transaction, so the next run matches.
    let again = run(&detector, &mut client).await.expect("second run");
    assert_eq!(again, DetectorOutcome::Match, "the second run bumped again");
}

/// so only the oid moves.
#[tokio::test]
async fn detector_bumps_on_a_new_database_oid() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, _, _) = db("detector_bumps_on_a_new_database_oid").await;
    let before = seed_ws(&mut client, "ws").await;
    let detector = Detector::new(64);
    run(&detector, &mut client).await.expect("record hub_meta");
    // Forge the row a restore into another database would carry.
    client
        .execute(
            "UPDATE hub_meta SET datoid = (datoid::bigint + 1000)::oid WHERE one",
            &[],
        )
        .await
        .expect("forge a foreign datoid");
    detector.commit_watermark("0/0", "ws", before, 0);
    let outcome = run(&detector, &mut client).await.expect("run");
    assert!(
        matches!(outcome, DetectorOutcome::Bumped { workspaces: 1 }),
        "a foreign datoid did not bump: {outcome:?}"
    );
}

/// A database that matches is left alone.
#[tokio::test]
async fn detector_match_alone_bumps_nothing() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, _, _) = db("detector_match_alone_bumps_nothing").await;
    seed_ws(&mut client, "ws").await;
    let detector = Detector::new(64);
    let epoch = primed(&detector, &mut client, "ws").await;
    for _ in 0..3 {
        assert_eq!(
            run(&detector, &mut client).await.expect("run"),
            DetectorOutcome::Match,
            "a matching database bumped"
        );
    }
    let after: i64 = client
        .query_one("SELECT epoch FROM workspaces WHERE id = 'ws'", &[])
        .await
        .expect("read the epoch")
        .get(0);
    assert_eq!(after as u64, epoch, "a match moved the epoch");
}

/// reverses that order and this test is what catches it.
#[tokio::test]
async fn detector_match_under_four_writers_bumps_nothing() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, _, url) = db("detector_match_under_four_writers_bumps_nothing").await;
    seed_ws(&mut client, "ws").await;
    let detector = Arc::new(Detector::new(64));
    let epoch = primed(&detector, &mut client, "ws").await;
    // Four writers, each raising the high-water after its own commit, exactly as §5.1 step 8 does.
    let mut writers = Vec::new();
    for id in 0..4u32 {
        let w = support::db::more(&url).await;
        let detector = Arc::clone(&detector);
        writers.push(tokio::spawn(async move {
            for round in 0..5u32 {
                w.execute(
                    "INSERT INTO links (ws, src_qcoll, src_id, field, target_qcoll, target_id) \
                     VALUES ('ws','p.c',$1,'f','p.c',$2)",
                    &[&format!("s{id}"), &format!("t{round}")],
                )
                .await
                .expect("write");
                detector.commit_watermark("0/0", "ws", epoch, 0);
            }
        }));
    }
    // Eight connections opening while they commit.
    let mut outcomes = Vec::new();
    for _ in 0..8 {
        let mut c = support::db::more(&url).await;
        outcomes.push(run(&detector, &mut c).await.expect("run"));
    }
    for w in writers {
        w.await.expect("writer");
    }
    let after: i64 = client
        .query_one("SELECT epoch FROM workspaces WHERE id = 'ws'", &[])
        .await
        .expect("read the epoch")
        .get(0);
    assert_eq!(
        after as u64, epoch,
        "the epoch moved under four writers: {outcomes:?}"
    );
}

/// Two connections opening at once bump once, not twice: the mutex is taken before the snapshot.
#[tokio::test]
async fn two_connections_opened_at_once_bump_once() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, _, url) = db("two_connections_opened_at_once_bump_once").await;
    seed_ws(&mut client, "ws").await;
    let detector = Arc::new(Detector::new(64));
    run(&detector, &mut client).await.expect("record hub_meta");
    // Make a bump warranted, so the two openers are really competing for one bump rather than
    // both correctly matching.
    stale(&detector);
    let mut tasks = Vec::new();
    for _ in 0..2 {
        let detector = Arc::clone(&detector);
        let url = url.clone();
        tasks.push(tokio::spawn(async move {
            let mut c = support::db::more(&url).await;
            run(&detector, &mut c).await.expect("run")
        }));
    }
    let mut bumps = 0;
    for t in tasks {
        if let DetectorOutcome::Bumped { .. } = t.await.expect("task") {
            bumps += 1;
        }
    }
    assert_eq!(bumps, 1, "two connections at once bumped {bumps} times");
}

/// comparison, so it is the control for this leg.
#[tokio::test]
async fn a_row_below_its_map_entry_is_a_mismatch() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, _, _) = db("a_row_below_its_map_entry_is_a_mismatch").await;
    seed_ws(&mut client, "ws").await;
    let detector = Detector::new(64);
    let epoch = primed(&detector, &mut client, "ws").await;
    // The map now claims a head_seq the database has never reached, and the LSN is untouched.
    detector.commit_watermark("0/0", "ws", epoch, 99);
    let outcome = run(&detector, &mut client).await.expect("run");
    assert!(
        matches!(outcome, DetectorOutcome::Bumped { workspaces: 1 }),
        "a row below its map entry did not bump: {outcome:?}"
    );
    // The bump clears the map, so the run after it matches.
    assert_eq!(
        run(&detector, &mut client).await.expect("run"),
        DetectorOutcome::Match,
        "the run after a bump bumped again"
    );
}

/// The high-water must be snapshotted BEFORE the flush-LSN read, and `hw-after-lsn` reverses that.
///
/// The test asserts the CORRECT behaviour only — a healthy database matches and its epoch holds.
/// It has no idea a break exists; `hw-after-lsn` reverses the order inside the library, the run
/// then reads as a restore, and these assertions fail. That is what makes the row a control rather
/// than a mirror.
///
/// A writer waits briefly for the seam to announce the gap and commits into it when it appears.
/// Without the break there is no gap and no seam, so it waits its short bound and exits; that wait
/// is the whole cost of this test when green.
#[tokio::test]
async fn detector_high_water_snapshotted_before_the_lsn_read() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, _, url) = db("detector_high_water_snapshotted_before_the_lsn_read").await;
    seed_ws(&mut client, "ws").await;
    let detector = std::sync::Arc::new(Detector::new(64));
    let epoch = primed(&detector, &mut client, "ws").await;

    for stale in [
        "detector-before-snapshot.ready",
        "detector-before-snapshot.done",
    ] {
        let _ = std::fs::remove_file(stale_step(stale));
    }
    let writer_detector = std::sync::Arc::clone(&detector);
    let writer = tokio::spawn(async move {
        if !wait_briefly_for("detector-before-snapshot.ready").await {
            return;
        }
        let mut w = support::db::more(&url).await;
        w.execute(
            "INSERT INTO links (ws, src_qcoll, src_id, field, target_qcoll, target_id) \
             VALUES ('ws','p.c','in-the-gap','f','p.c','t')",
            &[],
        )
        .await
        .expect("commit inside the gap");
        // Above any LSN the held run could have read, so its comparison must read as a restore.
        writer_detector.set_high_water("FFFFFFFF/FFFFFFFF");
        release("detector-before-snapshot.done");
    });

    let outcome = run(&detector, &mut client).await.expect("run");
    writer.await.expect("writer");
    assert_eq!(
        outcome,
        DetectorOutcome::Match,
        "a healthy database was read as a restore"
    );
    let after: i64 = client
        .query_one("SELECT epoch FROM workspaces WHERE id = 'ws'", &[])
        .await
        .expect("read the epoch")
        .get(0);
    assert_eq!(after as u64, epoch, "the epoch moved with the order intact");
}

/// The path of one handshake file.
fn stale_step(name: &str) -> String {
    let dir = std::env::var("GM_HUB_STEP_DIR").unwrap_or_else(|_| "target/hub-steps".to_string());
    format!("{dir}/{name}")
}

/// Wait a few seconds for `target/hub-steps/<name>.ready`; `false` when it never came.
async fn wait_briefly_for(name: &str) -> bool {
    let dir = std::env::var("GM_HUB_STEP_DIR").unwrap_or_else(|_| "target/hub-steps".to_string());
    let path = format!("{dir}/{name}");
    for _ in 0..20 {
        if std::path::Path::new(&path).exists() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    false
}

/// Create `target/hub-steps/<name>`, which is how a peer is told to stop waiting.
fn release(name: &str) {
    let dir = std::env::var("GM_HUB_STEP_DIR").unwrap_or_else(|_| "target/hub-steps".to_string());
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(format!("{dir}/{name}"), b"");
}
