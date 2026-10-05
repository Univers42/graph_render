//! §5.1's `If-Match`: per plugin, 412 on a stale one, and silent on another plugin's writes.

use super::*;

/// The workspace's `(epoch, head_seq)`, and the plugin's `plugin_seq`, as the cursors a client
/// would have read.
async fn cursors(client: &mut Client, plugin: &str) -> (u64, u64) {
    let epoch: i64 = client
        .query_one("SELECT epoch FROM workspaces WHERE id = 'ws'", &[])
        .await
        .expect("read the epoch")
        .get(0);
    let seq: i64 = client
        .query_one(
            "SELECT plugin_seq FROM manifests WHERE plugin = $1",
            &[&plugin],
        )
        .await
        .expect("read plugin_seq")
        .get(0);
    (epoch as u64, seq as u64)
}

/// A stale `If-Match` is a 412 and stores nothing; the current one is accepted.
#[tokio::test]
async fn if_match_refuses_a_stale_plugin_seq() {
    let (store, mut client, _, _) = ready("if_match_refuses_a_stale_plugin_seq").await;
    let (epoch, plugin_seq) = cursors(&mut client, "tracker").await;

    let mut stale = batch_write(
        "ws",
        "tracker",
        batch_of(&[("task", "1", 7, r#""name":"One""#)], &[]),
    );
    stale.if_match = Some(graph_contract::hub::Cursor {
        epoch,
        seq: plugin_seq + 7,
    });
    let error = store.apply_batch(&stale).await.expect_err("a stale cursor");
    assert_eq!(status(&error), "412", "a stale If-Match is a 412");
    assert_eq!(
        head_of(&mut client).await,
        1,
        "the refused batch took no seq"
    );

    let mut wrong_epoch = batch_write(
        "ws",
        "tracker",
        batch_of(&[("task", "1", 7, r#""name":"One""#)], &[]),
    );
    wrong_epoch.if_match = Some(graph_contract::hub::Cursor {
        epoch: epoch + 1,
        seq: plugin_seq,
    });
    let error = store
        .apply_batch(&wrong_epoch)
        .await
        .expect_err("another epoch's cursor");
    assert_eq!(
        status(&error),
        "412",
        "a cursor from another epoch is a 412 too"
    );

    let mut current = batch_write(
        "ws",
        "tracker",
        batch_of(&[("task", "1", 7, r#""name":"One""#)], &[]),
    );
    current.if_match = Some(graph_contract::hub::Cursor {
        epoch,
        seq: plugin_seq,
    });
    let outcome = store
        .apply_batch(&current)
        .await
        .expect("the current cursor");
    assert_answer(&outcome, 2, 1);
}

/// Another plugin's writes leave this plugin's `plugin_seq` alone, so its cursor stays valid.
#[tokio::test]
async fn if_match_ignores_another_plugins_writes() {
    let (store, mut client, _, _) = ready("if_match_ignores_another_plugins_writes").await;
    store
        .put_manifest(&manifest_write("ws", "other"))
        .await
        .expect("register a second plugin");
    let (epoch, plugin_seq) = cursors(&mut client, "tracker").await;

    for i in 0..3 {
        store
            .apply_batch(&batch_write(
                "ws",
                "other",
                batch_of(&[("task", &format!("{i}"), 7, r#""name":"Other""#)], &[]),
            ))
            .await
            .expect("another plugin's batch");
    }
    assert_eq!(
        cursors(&mut client, "tracker").await,
        (epoch, plugin_seq),
        "three batches under another plugin moved neither half of this plugin's cursor"
    );
    assert_eq!(
        head_of(&mut client).await,
        4,
        "the workspace's head_seq did move: 1 manifest + 2 manifests + 3 batches"
    );

    let mut req = batch_write(
        "ws",
        "tracker",
        batch_of(&[("task", "1", 7, r#""name":"One""#)], &[]),
    );
    req.if_match = Some(graph_contract::hub::Cursor {
        epoch,
        seq: plugin_seq,
    });
    let outcome = store
        .apply_batch(&req)
        .await
        .expect("this plugin's own cursor is still current");
    assert_answer(&outcome, 5, 1);
}
