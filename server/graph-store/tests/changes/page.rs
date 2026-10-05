//! A page: seq order, the byte cut, and the snapshot that keeps a page whole.

use super::*;
use graph_store::changes::ChangeKind;

/// The registration is change 1 and each batch one change after it, and every change's text is as
/// long as its stored header says.
#[tokio::test]
async fn the_feed_is_the_registration_then_one_change_per_batch() {
    let (store, client, _, _) = ready("the_feed_is_the_registration_then_one_change").await;
    write_tasks(&store, &["1", "2", "3"]).await;
    let epoch = epoch_of(&client).await;

    let all = read(&store, &since(Cursor { epoch, seq: 0 }))
        .await
        .expect("read the feed");
    assert_eq!(seqs_of(&all), [1, 2, 3, 4]);
    assert_eq!(all.changes[0].kind, ChangeKind::Manifest);
    assert!(all.changes[1..].iter().all(|c| c.kind == ChangeKind::Batch));
    assert_eq!((all.epoch, all.head_seq), (epoch, 4));
    assert_eq!(all.next, Cursor { epoch, seq: 4 });
    let lens: Vec<(u64, u64)> = all
        .changes
        .iter()
        .map(|c| (c.seq, c.text.len() as u64))
        .collect();
    assert_eq!(
        lens,
        header_bytes(&client).await,
        "each text is the length its header stored"
    );
    assert_eq!(all.bytes, lens.iter().map(|(_, n)| n).sum::<u64>());

    let first = &all.changes[1].upserts;
    let keys: Vec<(&str, &str, u64)> = first
        .iter()
        .map(|op| (op.qcoll.as_str(), op.id.as_str(), op.rev))
        .collect();
    assert_eq!(keys, [("tracker.task", "1", 1)]);
    assert!(all.changes[1].deletes.is_empty());
}

/// A page after a cursor starts at the next seq; a cursor at the head reads nothing and keeps its
/// place.
#[tokio::test]
async fn changes_after_a_cursor_are_in_seq_order() {
    let (store, client, _, _) = ready("changes_after_a_cursor_are_in_seq_order").await;
    write_tasks(&store, &["1", "2", "3"]).await;
    let epoch = epoch_of(&client).await;

    let after = read(&store, &since(Cursor { epoch, seq: 2 }))
        .await
        .expect("read after 2");
    assert_eq!(seqs_of(&after), [3, 4]);
    let caught_up = read(&store, &since(Cursor { epoch, seq: 4 }))
        .await
        .expect("read at head");
    assert!(caught_up.changes.is_empty());
    assert_eq!(caught_up.next, Cursor { epoch, seq: 4 });
}

/// A delete is sent as a delete, with the `rev` the record had when it went, and no text.
#[tokio::test]
async fn a_delete_is_sent_with_its_rev() {
    let (store, client, _, _) = ready("a_delete_is_sent_with_its_rev").await;
    write_tasks(&store, &["1"]).await;
    let outcome = store
        .apply_batch(&batch_write(
            "ws",
            "tracker",
            batch_of(&[], &[("task", "1")]),
        ))
        .await;
    assert!(outcome.is_ok(), "the delete applies: {outcome:?}");
    let epoch = epoch_of(&client).await;

    let page = read(&store, &since(Cursor { epoch, seq: 2 }))
        .await
        .expect("read the delete");
    let change = &page.changes[0];
    assert!(change.upserts.is_empty());
    assert_eq!(change.deletes.len(), 1);
    assert_eq!(
        (
            change.deletes[0].id.as_str(),
            change.deletes[0].rev,
            change.deletes[0].text.as_str()
        ),
        ("1", 1, "")
    );
    assert_eq!(change.text.len() as u64, header_bytes(&client).await[2].1);
}

/// The byte cut keeps whole changes under `max_bytes`, except that a page is never cut to nothing.
#[tokio::test]
async fn a_page_never_exceeds_max_bytes_but_returns_at_least_one_change() {
    let (store, client, _, _) = ready("a_page_never_exceeds_max_bytes").await;
    write_tasks(&store, &["1", "2", "3", "4", "5"]).await;
    let epoch = epoch_of(&client).await;
    let bytes = header_bytes(&client).await;
    let start = Cursor { epoch, seq: 1 };

    let two = bytes[1].1 + bytes[2].1;
    let cut = read(&store, &req(start, 1000, two))
        .await
        .expect("read under the cut");
    assert_eq!(seqs_of(&cut), [2, 3]);
    assert!(cut.bytes <= two);
    assert_eq!(cut.next, Cursor { epoch, seq: 3 });

    let one = read(&store, &req(start, 1000, 1))
        .await
        .expect("read under a tiny cut");
    assert_eq!(
        seqs_of(&one),
        [2],
        "a change larger than the cut still goes, alone"
    );

    let limited = read(&store, &req(start, 2, 1 << 30))
        .await
        .expect("read under the limit");
    assert_eq!(seqs_of(&limited), [2, 3]);
}

/// A prune that commits after the page chose its headers and before it read their operations does
/// not take the operations out from under it: the page is read in one snapshot.
///
/// The `changes-read-committed` break reads at `READ COMMITTED`, so the operation read sees the
/// prune, finds change 2 short, and the page is refused: this case goes red.
#[tokio::test]
async fn a_prune_between_the_header_and_operation_reads_still_yields_every_operation() {
    let _seam = SEAM.write().await;
    let (store, client, url, _) = ready("a_prune_between_the_header_and_operation_reads").await;
    write_tasks(&store, &["1", "2", "3"]).await;
    let epoch = epoch_of(&client).await;
    let pruner = support::db::more(&url).await;

    let step_dir = support::step::dir();
    std::fs::create_dir_all(&step_dir).expect("create the step directory");
    let [arm, ready_file, done] =
        ["armed", "ready", "done"].map(|ext| format!("{step_dir}/writer-after-headers.{ext}"));
    for path in [&ready_file, &done] {
        let _ = std::fs::remove_file(path);
    }
    std::fs::write(&arm, b"").expect("arm the after-headers seam");

    let reader = store.clone();
    let page = tokio::spawn(async move {
        graph_store::changes::page(&reader, &since(Cursor { epoch, seq: 0 })).await
    });
    let mut paused = false;
    for _ in 0..600 {
        if std::path::Path::new(&ready_file).exists() {
            paused = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let _ = std::fs::remove_file(&arm);
    assert!(paused, "the page read its headers and paused");
    prune_through(&pruner, 2).await;
    std::fs::write(&done, b"").expect("release the page");

    let page = page.await.expect("the page task joins");
    let _ = std::fs::remove_file(&done);
    let page = page.expect("the page is whole despite the prune");
    assert_eq!(seqs_of(&page), [1, 2, 3, 4]);
    assert_eq!(
        page.changes[1].upserts.len(),
        1,
        "change 2 kept its operation"
    );
    assert_eq!(header_bytes(&client).await.len(), 2, "the prune did commit");
}

/// A header whose stored operation count differs from the rows found is a store fault, and the
/// page is refused rather than sent short.
#[tokio::test]
async fn a_header_whose_operation_count_differs_is_never_sent() {
    let (store, client, _, _) = ready("a_header_whose_operation_count_differs").await;
    write_tasks(&store, &["1"]).await;
    let epoch = epoch_of(&client).await;
    as_writer(
        &client,
        "UPDATE change_headers SET ops = ops + 1 WHERE ws = 'ws' AND seq = 2",
    )
    .await;

    let refused = read(&store, &since(Cursor { epoch, seq: 0 })).await;
    assert!(
        matches!(refused, Err(StoreError::Db(_))),
        "refused: {refused:?}"
    );
}
