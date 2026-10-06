//! The document equals the Model's whatever the order of the writes, and holds still while a write
//! commits under it.

use super::*;

/// Both plugins and one record of every reference shape: a resolved link out, a parent, a list
/// of blocks, a scalar, and the two plain collections.
pub async fn seeded(name: &str, fetch_rows: u64) -> Twin {
    let mut twin = Twin::new(name, fetch_rows).await;
    twin.register("tracker", TRACKER).await;
    twin.register("other", OTHER).await;
    let things = batch_of(&[("thing", "t1", 1, r#""name":"T1""#)], &[]);
    assert!(twin.apply("other", things).await, "the things apply");
    let tasks = batch_of(
        &[
            (
                "task",
                "a",
                1,
                r#""name":"A","state":"open","tags":["x","y"],"blocks":["b"],"up":"b","link":"t1","note":1.5"#,
            ),
            ("task", "b", 1, r#""name":"B","blocks":[],"up":null"#),
            ("c", "1", 1, r#""name":"C1""#),
            ("note", "n", 1, r#""name":"N""#),
        ],
        &[],
    );
    assert!(twin.apply("tracker", tasks).await, "the tasks apply");
    twin
}

/// `head + Σ next + tail` is `Model::to_json`, one row per page and one page for everything.
#[tokio::test]
async fn document_bytes_equal_to_json_over_the_model() {
    for fetch_rows in [1, 2, 32] {
        let twin = seeded(&format!("doc_equals_model_{fetch_rows}"), fetch_rows).await;
        assert_eq!(
            twin.read().await,
            twin.model.to_json(),
            "fetch_rows {fetch_rows}"
        );
    }
}

/// A page cut by bytes rather than rows loses no record and ends only on an empty page, and a
/// record that costs more than the whole budget is still read, alone on its page.
#[tokio::test]
async fn pages_cut_by_bytes_read_every_record() {
    let mut cfg = StoreConfig::defaults();
    cfg.max_body = 4096;
    cfg.max_batch = 8;
    cfg.changes_bytes = cfg.max_change();
    let mut twin = Twin::configured("doc_pages_cut_by_bytes", cfg).await;
    twin.register("tracker", TRACKER).await;
    twin.register("other", OTHER).await;
    let wide = format!(r#""name":"{}""#, "w".repeat(4500));
    for chunk in 0..8 {
        let ids: Vec<String> = (0..5).map(|i| format!("t{chunk}{i}")).collect();
        let ups: Vec<(&str, &str, u32, &str)> = ids
            .iter()
            .map(|id| ("task", id.as_str(), 1, r#""name":"N""#))
            .collect();
        assert!(twin.apply("tracker", batch_of(&ups, &[])).await, "a chunk");
    }
    let mid = batch_of(&[("task", "t35x", 1, wide.as_str())], &[]);
    assert!(twin.apply("tracker", mid).await, "the wide record");
    assert_eq!(twin.read().await, twin.model.to_json());
}

/// Two reads at one cursor are the same bytes, and they quote the same cursor.
#[tokio::test]
async fn document_is_byte_identical_at_the_same_cursor() {
    let twin = seeded("doc_identical_at_cursor", 2).await;
    let first = read_all(&twin.store).await;
    let second = read_all(&twin.store).await;
    assert_eq!(first, second, "bytes, doc_bytes and cursor");
}

/// A batch that commits after the first piece is wholly out of the open document: the snapshot
/// is the one `open` took, and the cursor it quotes is that snapshot's.
#[tokio::test]
async fn a_batch_committed_mid_stream_is_not_in_the_document() {
    let mut twin = seeded("doc_snapshot_mid_stream", 2).await;
    let (before, _, cursor) = read_all(&twin.store).await;
    let mut doc = graph_store::materialize::open(&twin.store, "ws")
        .await
        .expect("open the document");
    let mut out = doc.head().to_owned();
    let first = doc.next().await.expect("the first piece");
    out.push_str(&first.expect("a first record"));
    let write = batch_of(&[("c", "2", 1, r#""name":"C2""#)], &[("note", "n")]);
    assert!(twin.apply("tracker", write).await, "the mid-stream batch");
    while let Some(piece) = doc.next().await.expect("the next piece") {
        out.push_str(&piece);
    }
    out.push_str(&doc.tail());
    assert_eq!(out, before, "the open document is the snapshot");
    assert_eq!(doc.cursor(), cursor, "and quotes the snapshot's cursor");
    let after = twin.read().await;
    assert_ne!(after, before, "a new document sees the batch");
    assert_eq!(after, twin.model.to_json());
}

/// A shuffle of `0..n` by Fisher-Yates over `seed`.
fn shuffled(n: usize, seed: u64) -> Vec<usize> {
    let mut rng = crate::support::rng::SplitMix64::seeded(seed);
    let mut order: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        order.swap(i, rng.below(i as u64 + 1) as usize);
    }
    order
}

/// Record `i` of the permutation case: a ring of blocks, a binary-tree parent, and a link to one
/// of three things of which only two exist, so some pruning survives every order.
fn ring_cells(i: usize) -> String {
    let up = if i == 0 {
        "null".to_owned()
    } else {
        format!(r#""t{:02}""#, (i - 1) / 2)
    };
    format!(
        r#""name":"T{i}","blocks":["t{:02}"],"up":{up},"link":"x{}""#,
        (i + 1) % 40,
        i % 3
    )
}

/// The document after writing `order` five records per batch.
async fn written_in(name: &str, order: &[usize]) -> (String, String) {
    let mut twin = Twin::new(name, 7).await;
    twin.register("tracker", TRACKER).await;
    twin.register("other", OTHER).await;
    let things = batch_of(&[("thing", "x0", 1, ""), ("thing", "x1", 1, "")], &[]);
    assert!(twin.apply("other", things).await, "the things apply");
    for chunk in order.chunks(5) {
        let ids: Vec<String> = chunk.iter().map(|i| format!("t{i:02}")).collect();
        let cells: Vec<String> = chunk.iter().map(|&i| ring_cells(i)).collect();
        let ups: Vec<(&str, &str, u32, &str)> = ids
            .iter()
            .zip(&cells)
            .map(|(id, c)| ("task", id.as_str(), 1, c.as_str()))
            .collect();
        assert!(twin.apply("tracker", batch_of(&ups, &[])).await, "a chunk");
    }
    (twin.read().await, twin.model.to_json())
}

/// Forty records written in five orders give one document, and it is the Model's.
#[tokio::test]
async fn permuted_insertion_order_gives_the_same_bytes() {
    let (first, model) = written_in("doc_permuted_0", &shuffled(40, 0)).await;
    assert_eq!(first, model, "order 0 against the model");
    for seed in 1..5 {
        let (doc, model) = written_in(&format!("doc_permuted_{seed}"), &shuffled(40, seed)).await;
        assert_eq!(doc, first, "order {seed} against order 0");
        assert_eq!(doc, model, "order {seed} against the model");
    }
}

/// The ingest reader takes the document whole, with every record the Model stores.
#[tokio::test]
async fn ingest_read_accepts_the_document() {
    let twin = seeded("doc_ingest_reads", 3).await;
    let read = ingest(&twin.read().await);
    assert_eq!(read.source, "ws");
    assert_eq!(read.records.len(), twin.model.records().count());
    assert_eq!(read.collections.len(), 4, "tracker's three and other's one");
}
