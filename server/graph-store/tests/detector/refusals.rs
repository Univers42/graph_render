use super::*;

/// `fsync` off: refused. `fsync` is PGC_SIGHUP, so a reload is enough.
#[tokio::test]
async fn detector_refuses_fsync_off() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, admin, _) = db("detector_refuses_fsync_off").await;
    seed_ws(&mut client, "ws").await;
    admin
        .batch_execute("ALTER SYSTEM SET fsync = off")
        .await
        .expect("set fsync off");
    admin
        .query_one("SELECT pg_reload_conf()", &[])
        .await
        .expect("reload");
    let detector = Detector::new(64);
    let mut fresh = support::db::more(&support::db::url_now()).await;
    let refused = run(&detector, &mut fresh).await;
    admin
        .batch_execute("ALTER SYSTEM RESET fsync")
        .await
        .expect("restore fsync");
    admin
        .query_one("SELECT pg_reload_conf()", &[])
        .await
        .expect("reload after reset");
    assert!(
        matches!(refused, Err(graph_store::StoreError::NoDatabase)),
        "a database with fsync off was accepted: {refused:?}"
    );
}

/// `full_page_writes` off: refused, for the same reason.
#[tokio::test]
async fn detector_refuses_full_page_writes_off() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, admin, _) = db("detector_refuses_full_page_writes_off").await;
    seed_ws(&mut client, "ws").await;
    admin
        .batch_execute("ALTER SYSTEM SET full_page_writes = off")
        .await
        .expect("set full_page_writes off");
    admin
        .query_one("SELECT pg_reload_conf()", &[])
        .await
        .expect("reload");
    let detector = Detector::new(64);
    let mut fresh = support::db::more(&support::db::url_now()).await;
    let refused = run(&detector, &mut fresh).await;
    admin
        .batch_execute("ALTER SYSTEM RESET full_page_writes")
        .await
        .expect("restore full_page_writes");
    admin
        .query_one("SELECT pg_reload_conf()", &[])
        .await
        .expect("reload after reset");
    assert!(
        matches!(refused, Err(graph_store::StoreError::NoDatabase)),
        "a database with full_page_writes off was accepted: {refused:?}"
    );
}

/// here — but a role default would suppress the epoch triggers for every write the role makes.
#[tokio::test]
async fn detector_refuses_a_hub_writer_default() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, admin, _) = db("detector_refuses_a_hub_writer_default").await;
    seed_ws(&mut client, "ws").await;
    admin
        .batch_execute("ALTER ROLE hub SET hub.writer = '1'")
        .await
        .expect("set the role default");
    let detector = Detector::new(64);
    // A NEW connection: `ALTER ROLE ... SET` is applied at login, so the session that set it
    // still has no `hub.writer` and would sail past the refusal.
    let mut fresh = support::db::more(&support::db::url_now()).await;
    let refused = run(&detector, &mut fresh).await;
    admin
        .batch_execute("ALTER ROLE hub RESET hub.writer")
        .await
        .expect("clear the role default");
    assert!(
        matches!(refused, Err(graph_store::StoreError::NoDatabase)),
        "a role with a hub.writer default was accepted: {refused:?}"
    );
}
