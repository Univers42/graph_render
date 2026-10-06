//! §6's `max_header`: what one change header costs a subscriber while its page is read, and the
//! proof that an SSE read holds at most `SSE_PAGE` of them and none of their operations.
//!
//! The workspace is seeded with `MEASURED + 2` changes of one `NOTE`-byte record each, so a read
//! that pulled operations would hold more than `NOTE` bytes per header and miss the ceiling
//! (break `sse-full-page`, row `negctl-sse-full-page`).

use crate::support::db;
use crate::support::fixtures::{epoch_of, hub_db, ready, upsert};
use crate::{Measured, PLUGIN, child, env, ledger, rss};
use graph_contract::hub::Cursor;
use graph_hub::events::page::{PageReq, page};
use std::hint::black_box;

/// `GRAPH_HUB_SSE_PAGE`'s default (`src/config/env.rs`).
const SSE_PAGE: u64 = 256;

/// `GRAPH_HUB_CHANGES_BYTES`'s default, the byte cut an SSE read passes on.
const CHANGES_BYTES: u64 = 8 << 20;

/// The headers the slope is taken over. Caveat: one read's resident peak moves by about 150 KiB
/// from run to run (three runs at 256 headers gave 48, 257 and 1124 B per header), so the count
/// must be large enough that this noise is a few bytes per header.
const MEASURED: u64 = 2048;

/// The note each seeded change carries: large enough that a read of operations shows, small enough
/// that `MEASURED` of them stay under `CHANGES_BYTES` and the byte cut does not shorten the page.
const NOTE: usize = 1 << 10;

/// Per-header cost is the slope between a one-header read and a `MEASURED` read, each in its own
/// child, so the connection and runtime both reads open cancel out; an `SSE_PAGE` read in between
/// proves the read stops at the count it is given.
///
/// Caveat: the slope is taken over resident pages and one run's noise, so it is good to about
/// 150 KiB / 2047 ≈ 75 B per header either way; the ledger's ceiling carries that margin.
#[tokio::test]
async fn a_changes_page_holds_at_most_sse_page_headers() {
    let hub = hub_db(&[]).await;
    let ws = db::unique("maxheader");
    ready(&hub, &ws, PLUGIN).await;
    for i in 0..MEASURED + 2 {
        let note = format!("{i:0>width$}", width = NOTE);
        let sent = hub
            .post(
                &format!("/v1/workspaces/{ws}/plugins/{PLUGIN}/batches"),
                upsert("task", "a", &note),
            )
            .await;
        assert_eq!(sent.code(), 200, "batch {i}: {}", sent.message());
    }
    let epoch = epoch_of(&hub, &ws).await;
    let one = read_in_child(&ws, &epoch, 1).await;
    let page = read_in_child(&ws, &epoch, SSE_PAGE).await;
    let full = read_in_child(&ws, &epoch, MEASURED).await;
    assert_eq!(one.size, 1, "a one-header read returned {} headers", one.size);
    assert_eq!(page.size, SSE_PAGE, "the read did not stop at SSE_PAGE");
    assert_eq!(full.size, MEASURED, "the byte cut shortened the measured read");
    let per_header = full.peak.saturating_sub(one.peak) as f64 / (MEASURED - 1) as f64;
    println!(
        "HUB_MEM max_header={per_header:.1} peak_1={} peak_{SSE_PAGE}={} peak_{MEASURED}={}",
        one.peak, page.peak, full.peak
    );
    let ceiling = ledger::value("max_header_ceiling_bytes");
    assert!(
        per_header <= ceiling,
        "one header costs {per_header:.1} B as read, over the recorded {ceiling}"
    );
}

async fn read_in_child(ws: &str, epoch: &str, at_most: u64) -> Measured {
    let vars = vec![
        ("HUB_MEM_WS", ws.to_owned()),
        ("HUB_MEM_EPOCH", epoch.to_owned()),
        ("HUB_MEM_AT_MOST", at_most.to_string()),
        ("MALLOC_ARENA_MAX", "1".to_owned()),
    ];
    tokio::task::spawn_blocking(move || child("heads::heads_read_peak", &vars))
        .await
        .expect("the child runner")
}

/// One SSE read of `HUB_MEM_AT_MOST` headers from the start of `HUB_MEM_WS`, measured from after the
/// store is built to after the page is in hand.
#[test]
#[ignore = "a child process of a_changes_page_holds_at_most_sse_page_headers"]
fn heads_read_peak() {
    let request = PageReq {
        ws: env("HUB_MEM_WS"),
        since: Cursor {
            epoch: env("HUB_MEM_EPOCH").parse().expect("an epoch"),
            seq: 0,
        },
        at_most: env("HUB_MEM_AT_MOST").parse().expect("a header count"),
        max_bytes: CHANGES_BYTES,
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let store = runtime.block_on(db::store());
    let base = rss::baseline();
    let answer = runtime
        .block_on(page(&store, &request))
        .unwrap_or_else(|error| panic!("the read: {error:?}"));
    let peak = rss::peak();
    println!("HUB_MEM peak={} size={}", peak - base, answer.heads.len());
    black_box(&answer);
}
