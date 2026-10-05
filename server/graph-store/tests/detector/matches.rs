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
