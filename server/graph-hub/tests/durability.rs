//! Task 9: an acknowledged batch survives a `docker kill` of the hub container.
//!
//! The cases run against the container `scripts/orch/hub-run.sh` started, over TCP, under
//! `hub-run.sh run`, which serves the kill and start steps they ask for (`support::step`). The kill
//! is SIGKILL, so nothing drains: every 200 the hub sent must already be in PostgreSQL. The store is
//! read directly rather than through `/changes`, so the check does not depend on the feed route.
//!
//! Row `hub-durability` runs the writer case in one process and the three checks in a second one,
//! so the acked list crosses a process boundary as a file (`target/hub-steps/durability.acked`).
//! Row `negctl-ack-before-commit` runs only the last case against a hub that answers before it
//! commits, which must turn it red.
#![cfg(feature = "db-tests")]

#[path = "support/mod.rs"]
mod support;

use graph_contract::hub::Cursor;
use graph_store::changes::{ChangesReq, page};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use support::wire::Remote;
use support::{db, fixtures, step};

const WS: &str = "durability";
const PLUGIN: &str = "task";
const ACKED: &str = "durability.acked";
const WRITERS: usize = 4;
const PER_WRITER: usize = 10_000;
const KILL_AFTER: usize = 50;

/// How long the writers may take to collect [`KILL_AFTER`] acks. Caveat: a guess well above fifty
/// small batches on a loaded host; past it the case fails naming the count, never hangs.
const ACK_WAIT: Duration = Duration::from_secs(60);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn acked_seqs_are_written_before_the_kill() {
    db::migrated().await;
    let remote = Arc::new(Remote::new());
    remote.ready(WS, PLUGIN).await;
    let acked = Arc::new(Mutex::new(Vec::new()));
    let writers: Vec<_> = (0..WRITERS)
        .map(|w| tokio::spawn(write_until_cut(Arc::clone(&remote), w, Arc::clone(&acked))))
        .collect();
    wait_for(&acked, KILL_AFTER).await;
    step::act("kill", "kill").await;
    for writer in writers {
        let cut = writer.await.expect("a writer task");
        assert!(cut.is_some(), "a writer ran out of batches before the kill");
    }
    let seqs = acked.lock().unwrap().clone();
    step::record(ACKED, &seqs);
}

#[tokio::test]
async fn every_acked_seq_is_present_after_a_container_kill() {
    let acked = step::recorded(ACKED);
    assert!(acked.len() >= KILL_AFTER, "only {} acked seqs", acked.len());
    let present = stored_seqs(WS).await;
    let missing: Vec<u64> = acked
        .iter()
        .copied()
        .filter(|seq| present.binary_search(seq).is_err())
        .collect();
    assert!(missing.is_empty(), "acked and lost: {missing:?}");
}

#[tokio::test]
async fn no_gap_after_the_kill() {
    step::act("start", "start").await;
    let remote = Remote::new();
    let before = stored_seqs(WS).await;
    let expected: Vec<u64> = (1..=before.len() as u64).collect();
    assert_eq!(before, expected, "the feed has a hole or a duplicate");
    let body = fixtures::upsert(PLUGIN, "after-the-kill", "n");
    let reply = remote
        .post_batch(WS, PLUGIN, &body, "after-the-kill")
        .await
        .expect("a batch after the restart");
    assert_eq!(reply.seq(), before.len() as u64 + 1, "the next seq skipped");
}

#[tokio::test]
async fn a_batch_answered_after_the_commit_survives_the_kill() {
    const WINDOW: &str = "window";
    db::migrated().await;
    step::act("start", "start").await;
    let remote = Remote::new();
    remote.ready(WINDOW, PLUGIN).await;
    let id = format!("answered-{}", std::process::id());
    let reply = remote
        .post_batch(WINDOW, PLUGIN, &fixtures::upsert(PLUGIN, &id, "n"), &id)
        .await
        .expect("the batch's answer");
    let seq = reply.seq();
    step::act("kill", "kill").await;
    let found = stored_changes(WINDOW)
        .await
        .into_iter()
        .find(|(at, _)| *at == seq);
    step::act("start", "start").await;
    let (_, ids) = found.unwrap_or_else(|| panic!("seq {seq} was answered and is not stored"));
    assert!(ids.contains(&id), "seq {seq} holds {ids:?}, not {id}");
}

/// Post batches under writer `w` until the first transport error, which is the kill; push every
/// acked seq. A refusal is a panic: under no load cap a batch here has no reason to be refused.
async fn write_until_cut(
    remote: Arc<Remote>,
    w: usize,
    acked: Arc<Mutex<Vec<u64>>>,
) -> Option<String> {
    for i in 0..PER_WRITER {
        let id = format!("w{w}-{i}");
        let body = fixtures::upsert(PLUGIN, &id, "n");
        match remote.post_batch(WS, PLUGIN, &body, &id).await {
            Ok(reply) => acked.lock().unwrap().push(reply.seq()),
            Err(error) => return Some(error),
        }
    }
    None
}

/// Wait until `acked` holds `count` seqs. Caveat: polled every 10 ms, bounded by [`ACK_WAIT`].
async fn wait_for(acked: &Mutex<Vec<u64>>, count: usize) {
    let start = Instant::now();
    while acked.lock().unwrap().len() < count {
        assert!(
            start.elapsed() < ACK_WAIT,
            "{} of {count} acks",
            acked.lock().unwrap().len()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Every seq `ws` holds, in order, read from the store.
async fn stored_seqs(ws: &str) -> Vec<u64> {
    stored_changes(ws)
        .await
        .into_iter()
        .map(|(seq, _)| seq)
        .collect()
}

/// Every change of `ws` as its seq and the record ids it upserted, paged from seq 0 to the head.
async fn stored_changes(ws: &str) -> Vec<(u64, Vec<String>)> {
    let store = db::store().await;
    let mut client = store.client().await.expect("a store connection");
    let (epoch, head) = graph_store::epoch::head_of(&mut client, ws)
        .await
        .expect("the workspace row")
        .unwrap_or_else(|| panic!("no workspace {ws}"));
    let mut since = Cursor { epoch, seq: 0 };
    let mut out = Vec::new();
    while since.seq < head {
        let request = ChangesReq {
            ws: ws.to_owned(),
            since,
            limit: 1_000,
            max_bytes: 64 << 20,
        };
        let found = page(&store, &request).await.expect("a page of changes");
        assert!(
            found.next.seq > since.seq,
            "the feed stopped at {}",
            since.seq
        );
        for change in found.changes {
            let ids = change.upserts.into_iter().map(|op| op.id).collect();
            out.push((change.seq, ids));
        }
        since = found.next;
    }
    out
}
