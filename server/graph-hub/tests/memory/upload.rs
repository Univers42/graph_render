//! The client half of Task 10 Step 4: one workspace filled to `GRAPH_HUB_MAX_DOC_BYTES`, then six
//! `POST /layout` calls against the hub container `scripts/orch/hub-run.sh` started.
//!
//! Decision 4's input is **one `GRAPH_HUB_MAX_DOC_BYTES` canonical document made of the smallest
//! record the contract admits**, because the record count is then the highest the cap allows and
//! the per-record overhead the lowest: the upload is as many chunks as it can be, which is the
//! relay's worst case rather than its best. The plan's "one-character id" is not reachable — ids
//! must be distinct, and a document of this size holds ~880 000 of them — so the ids are the
//! shortest distinct ones of a **fixed** width, and the case prints the record size and the count it
//! actually wrote so `docs/measurements/hub-memory.md` records the real figures rather than the
//! plan's estimate.
//!
//! One warm-up and then five timed runs, each asserted 200 with `Graph-Seq` equal to the `/graph`
//! ETag at the same cursor (H15), which is what makes the six `layout-upload` lines in the hub's log
//! five measurements of one thing.
//!
//! Caveat: the case reads the hub's **answer heads** only, so it does not check that a layout of
//! this size is correct; `hub-roundtrip` checks that at a small size. What this case is about is the
//! upload's cost and the headers the contract promises on the way out.

use std::time::Duration;

use crate::PLUGIN;
use crate::support::db;
use crate::support::wire::Remote;

/// The workspace this case fills, and the plugin it is registered under.
const WS: &str = "upload-cap";

/// The motor layout asked for: `layout.grid`, the first registry entry, as `hub-roundtrip` uses.
const LAYOUT: &str = "layout.grid";

/// How long one request may take.
///
/// Caveat: a bound above one 64 MiB upload plus the motor's own layout of it, and not a measurement;
/// the figure that matters is in `target/hub-mem/upload.txt`, read out of the hub's log. This is here
/// so a hub that is gone ends the case on an error instead of on the test harness's own patience.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

/// One warm-up, then this many timed runs: the pass condition is over the slowest of the five, so
/// five is what the row reads and the warm-up is what keeps a first run's cold page cache out of it.
const WARM_UPS: usize = 1;
const RUNS: usize = 5;

/// Records per batch.
///
/// `max_batch` is 10 000 operations (graph-contract's `Limits::DEFAULT`), and a body of that many
/// smallest records is ~760 kB, well under `max_body`'s 4 MiB — so the operation cap, not the body
/// cap, is what bounds a batch here, and taking the whole of it is what keeps the fill to ~88
/// round trips.
const BATCH: usize = 10_000;

/// The id width, and why it is not one character.
///
/// A document at the cap holds ~880 000 records and ids must be distinct across all of them, so one
/// character (36 of them) is arithmetically impossible. `id` admits `[a-z0-9-]`, so 36^5 = 60 466 176
/// five-character ids cover the count with room to spare, and a **fixed** width is what makes "one
/// record costs the same as every other" a fact rather than an average — a mixed-width document
/// would give the fill a range of per-record costs and the measurement one number to name.
const ID_WIDTH: usize = 5;

/// What the fill wrote, read back off the store rather than predicted.
struct Filled {
    /// The workspace's `doc_bytes`, the number the cap is enforced on.
    doc_bytes: u64,
    /// Records written.
    records: u64,
    /// One record's stored text, in bytes, separators excluded.
    record_bytes: u64,
    /// Batches posted.
    batches: u64,
}

/// Fill `WS` to `cap` with the smallest admissible record, then time five `/layout` calls.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "run by scripts/orch/hub-mem.sh upload against a hub and a motor it started"]
async fn a_capped_workspace_uploads_five_times_inside_the_motor_body_timeout() {
    db::migrated().await;
    let remote = Remote::with_timeout(REQUEST_TIMEOUT);
    remote.ready(WS, PLUGIN).await;
    let cap = cap();
    let fill = fill_to_the_cap(&remote, cap).await;
    assert!(
        cap - fill.doc_bytes < fill.record_bytes,
        "the fill stopped {} bytes short of the cap, which is a whole record or more",
        cap - fill.doc_bytes
    );
    println!(
        "HUB_MEM upload cap={cap} doc_bytes={} records={} record_bytes={} batches={} id_width={ID_WIDTH}",
        fill.doc_bytes, fill.records, fill.record_bytes, fill.batches
    );
    let etag = graph_etag(&remote).await;
    for _ in 0..WARM_UPS {
        assert_layout_ok(&remote, &etag, "warm-up").await;
    }
    for _ in 0..RUNS {
        assert_layout_ok(&remote, &etag, "run").await;
    }
    println!("HUB_MEM upload etag={etag} warm_ups={WARM_UPS} runs={RUNS}");
}

/// The cap this case fills to, read from the environment rather than from §6's default: the hub
/// container's `GRAPH_HUB_MAX_DOC_BYTES` and this number must be one value, and the script is what
/// sets both.
fn cap() -> u64 {
    std::env::var("HUB_MEM_MAX_DOC_BYTES")
        .expect("HUB_MEM_MAX_DOC_BYTES: run through scripts/orch/hub-mem.sh upload")
        .parse()
        .expect("a byte count")
}

/// Batches of the smallest admissible record until one more record would pass `cap`.
///
/// The loop stops on the store's own `doc_bytes`, read through `materialize::open` once per batch,
/// and not on arithmetic over it: the store refuses a batch that would cross the cap with a 413
/// (`writer/plan.rs:275`), and a fill that raced the cap would fail for a reason the measurement is
/// not about. The store is opened once and its snapshot taken per batch, so the fill holds no
/// connection between batches.
async fn fill_to_the_cap(remote: &Remote, cap: u64) -> Filled {
    let store = db::store().await;
    let mut written = 0_u64;
    let mut batches = 0_u64;
    let mut record_bytes = 0_u64;
    loop {
        let before = doc_bytes(&store).await;
        let room = cap.saturating_sub(before);
        // One record plus the separator that joins it to the previous one. The first pass has no
        // record yet, so it takes one and measures what that cost.
        let per = if written == 0 { 1 } else { record_bytes + 1 };
        if written > 0 && room <= per {
            return Filled {
                doc_bytes: before,
                records: written,
                record_bytes,
                batches,
            };
        }
        let take = ((room / per) as usize).clamp(1, BATCH);
        post(remote, written, take, batches).await;
        written += take as u64;
        batches += 1;
        if record_bytes == 0 {
            // The first batch measured: what `take` records added, over `take`. Fixed-width ids
            // are what make this one number stand for every record in the document.
            record_bytes = (doc_bytes(&store).await - before) / take as u64;
            assert!(
                record_bytes > 0,
                "a batch of {take} records added no bytes to doc_bytes"
            );
        }
    }
}

/// One batch of `take` records with ids `from`, `from + 1`, …, and the 200 that says it landed.
async fn post(remote: &Remote, from: u64, take: usize, batch: u64) {
    let body = batch_body(from, take);
    let reply = remote
        .post_batch(WS, PLUGIN, &body, &format!("{WS}-fill-{batch}"))
        .await
        .unwrap_or_else(|error| panic!("the fill batch {batch}: {error}"));
    assert_eq!(
        reply.code(),
        200,
        "the fill batch {batch} of {take}: {} {}",
        reply.code(),
        reply.body()
    );
}

/// One batch body of `take` records whose ids are `from`, `from + 1`, … in base 36, zero-padded to
/// [`ID_WIDTH`].
///
/// The record is the smallest the contract admits: the four members `read_batch` requires
/// (`collection`, `id`, `updatedAt`, `values`) and one scalar cell. The manifest declares a `title`
/// field and `check_title` runs on the **manifest**, so a record need not carry it
/// (`crates/graph-contract/src/ingest/validate.rs:155`) — which is what lets the record stop at one
/// scalar. `values` is not empty because `Role::Scalar` accepts any JSON (`batch/cells.rs:74`) and a
/// document of no cells is not a document any real workspace holds.
///
/// Caveat: `updatedAt` is fixed at 0 rather than incremented per record. D6 says a `u32`, and every
/// record here is a distinct id rather than a distinct version of one, so the value is not what the
/// upload's byte count turns on — but it does mean the fill never exercises a store that has to
/// resolve two versions of one id, which is a different question from this one.
fn batch_body(from: u64, take: usize) -> String {
    let mut out = String::with_capacity(take * (ID_WIDTH + 64));
    out.push_str(r#"{"upserts":["#);
    for i in 0..take {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            r#"{{"collection":"task","id":"{}","updatedAt":0,"values":{{"note":"x"}}}}"#,
            base36(from + i as u64, ID_WIDTH)
        ));
    }
    out.push_str(r#"],"deletes":[]}"#);
    out
}

/// `n` in base 36, lowercased and zero-padded to exactly `width` bytes.
///
/// Lowercase and digits only, because `check_record_id` takes what `id` admits and the store's
/// records page orders ids under the `C` collation. The assertion is what makes the width a promise:
/// a count that outgrew `width` digits would otherwise silently repeat an id and rewrite an earlier
/// record, which would shrink the document instead of filling it.
fn base36(mut n: u64, width: usize) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = vec![b'0'; width];
    for slot in out.iter_mut().rev() {
        *slot = DIGITS[(n % 36) as usize];
        n /= 36;
    }
    assert_eq!(n, 0, "record {n} needs more than {width} base-36 digits");
    String::from_utf8(out).expect("ASCII digits")
}

/// The workspace's own `doc_bytes`, the number the cap is enforced on.
async fn doc_bytes(store: &graph_store::Store) -> u64 {
    let document = graph_store::materialize::open(store, WS)
        .await
        .expect("a snapshot of the filled workspace");
    document.doc_bytes()
}

/// The `/graph` ETag at the current cursor, which is the position `Graph-Seq` must equal (H15).
async fn graph_etag(remote: &Remote) -> String {
    let reply = remote.graph_head(WS).await;
    assert_eq!(
        reply.code(),
        200,
        "/graph: {} {}",
        reply.code(),
        reply.body()
    );
    let etag = reply.header("etag").trim_matches('"').to_owned();
    assert!(!etag.is_empty(), "/graph carries no ETag");
    etag
}

/// One `/layout` call: 200, and `Graph-Seq` equal to the ETag the document was read at.
async fn assert_layout_ok(remote: &Remote, etag: &str, what: &str) {
    let reply = remote.layout_head(WS, LAYOUT).await;
    assert_eq!(
        reply.code(),
        200,
        "{what}: {} {}",
        reply.code(),
        reply.body()
    );
    assert_eq!(
        reply.header("graph-seq"),
        etag,
        "{what}: Graph-Seq is the position /graph quoted"
    );
}
