//! the guard: hub write paths stay silent, and the reverse case proves it

use super::*;



/// under test is the guard inside the trigger — the same guard whatever wrote the statement.
#[tokio::test]
async fn hub_write_paths_move_no_epoch() {
    let (mut client, _, _) = support::db::fresh_pair("hub_write_paths_move_no_epoch").await;
    pin_clock_ahead(&mut client).await;
    // Seed inside the guard, so the baseline below is untouched by the setup.
    hub_tx(&mut client).await;
    client
        .execute("INSERT INTO workspaces (id, epoch) VALUES ('ws', 1)", &[])
        .await
        .expect("seed a workspace");
    client.batch_execute("COMMIT").await.expect("commit");
    let seed = clock(&mut client).await;

    // A workspace create. It draws ONE epoch of its own, by `hub_next_epoch()` in the VALUES,
    // because a new workspace must start on a fresh epoch. What it must not do is draw a SECOND
    // one from the trigger, which is what "the guard is not holding" would look like.
    hub_tx(&mut client).await;
    client
        .execute(
            "INSERT INTO workspaces (id, epoch) VALUES ('ws2', hub_next_epoch()) \
             ON CONFLICT (id) DO NOTHING",
            &[],
        )
        .await
        .expect("create a workspace");
    client.batch_execute("COMMIT").await.expect("commit");
    // Exactly one draw: the `hub_next_epoch()` in the VALUES. A second one, from the trigger
    // firing under the guard, would advance the parked clock by two.
    assert_eq!(
        clock(&mut client).await,
        seed + 1,
        "the workspace create drew more than its own epoch, so the trigger fired under the guard"
    );

    // A manifest PUT: draws no epoch of its own, so the delta must be exactly zero.
    let before_put = clock(&mut client).await;
    hub_tx(&mut client).await;
    client
        .execute(
            "INSERT INTO manifests (ws, plugin, version, text, text_bytes, decl_bytes) \
             VALUES ('ws','p',1,'{}',2,2) ON CONFLICT (ws, plugin) \
             DO UPDATE SET version = 2",
            &[],
        )
        .await
        .expect("put a manifest");
    client.batch_execute("COMMIT").await.expect("commit");
    assert_eq!(
        clock(&mut client).await,
        before_put,
        "a manifest PUT moved the epoch"
    );

    // An all-no-op batch: the row is written and the text is identical, so the only thing that
    // could move the clock is the trigger. Three statements, one transaction, still zero.
    let before_batch = clock(&mut client).await;
    hub_tx(&mut client).await;
    for _ in 0..3 {
        client
            .execute(
                "INSERT INTO records (ws, plugin, qcoll, id, rev, updated_at, text, text_sha256,\
                 text_bytes) VALUES ('ws','p','p.c','r1',1,7,'{}','\\x616263',2) \
                 ON CONFLICT (ws, qcoll, id) DO UPDATE SET text = records.text",
                &[],
            )
            .await
            .expect("upsert a record");
    }
    client.batch_execute("COMMIT").await.expect("commit");
    assert_eq!(
        clock(&mut client).await,
        before_batch,
        "an all-no-op batch moved the epoch"
    );
}


/// under the guard the clock must not move, and outside it the clock must.
#[tokio::test]
async fn workspaces_update_does_not_bump_itself() {
    let (mut client, _, _) =
        support::db::fresh_pair("workspaces_update_does_not_bump_itself").await;
    make_ws(&mut client, "ws").await;
    pin_clock_ahead(&mut client).await;

    // The store's own update: inside the guard, silent.
    let guarded_before = clock(&mut client).await;
    hub_tx(&mut client).await;
    client
        .execute(
            "UPDATE workspaces SET head_seq = head_seq + 1 WHERE id = 'ws'",
            &[],
        )
        .await
        .expect("bump head_seq");
    client.batch_execute("COMMIT").await.expect("commit");
    assert_eq!(
        clock(&mut client).await,
        guarded_before,
        "the store's own head_seq update drew an epoch"
    );

    // The operator's: outside the guard, not silent. This is the reverse check — it is what
    // proves the first half is the guard and not a dead trigger.
    let bare_before = clock(&mut client).await;
    client
        .execute(
            "UPDATE workspaces SET head_seq = head_seq + 1 WHERE id = 'ws'",
            &[],
        )
        .await
        .expect("bump head_seq again");
    assert_eq!(
        clock(&mut client).await,
        bare_before + 1,
        "an unguarded update of workspaces drew no epoch, so the trigger is not firing at all"
    );
}
