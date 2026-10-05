//! The three caps: `GRAPH_HUB_MAX_DOC_BYTES`, `GRAPH_HUB_MAX_PLUGIN_BYTES` and `max_change`.

use super::*;

/// The document cap and the plugin cap are both 413, and a refusal past either stores nothing.
///
/// WHY the caps are set small rather than by writing megabytes: the caps are `StoreConfig` fields,
/// so a store opened with `max_doc_bytes: 4096` refuses at the same point in the same code as one
/// opened with the 64 MiB default, and the case runs in milliseconds. The bound is arithmetic, not
/// volume.
#[tokio::test]
async fn plugin_bytes_and_doc_bytes_caps_are_413() {
    let (_, _, url) = support::db::fresh_pair("plugin_bytes_and_doc_bytes_caps").await;
    let mut config = graph_store::StoreConfig::defaults();
    config.max_doc_bytes = 4096;
    config.max_plugin_bytes = 2048;
    let store = store_on(&url, config).await;
    store.create_workspace("ws", &LIMITS).await.expect("create");
    store
        .put_manifest(&manifest_write("ws", "tracker"))
        .await
        .expect("register");

    let filler = "x".repeat(600);
    let mut refused: Option<StoreError> = None;
    let mut applied = 0u64;
    for i in 0..8 {
        let cells = format!(r#""name":"{filler}","note":"{filler}""#);
        match store
            .apply_batch(&batch_write(
                "ws",
                "tracker",
                batch_of(&[("task", &format!("{i}"), 7, &cells)], &[]),
            ))
            .await
        {
            Ok(outcome) => applied += outcome.applied,
            Err(e) => {
                refused = Some(e);
                break;
            }
        }
    }
    let error = refused.expect("one of eight 1.4 KB records is past a 2 KB plugin cap");
    assert_eq!(status(&error), "413", "past a cap is a 413");
    assert!(
        applied > 0,
        "the cap bit after some records had already been applied, so the refusal is a refusal and \
         not a first write failing"
    );

    let mut client = support::db::more(&url).await;
    let stored: i64 = client
        .query_one("SELECT count(*) FROM records WHERE ws = 'ws'", &[])
        .await
        .expect("count the records")
        .get(0);
    assert_eq!(
        stored, applied as i64,
        "the refused batch stored nothing: the count is the applied count"
    );
    let bytes = doc_bytes(&mut client).await;
    let plugin: i64 = client
        .query_one("SELECT plugin_bytes FROM manifests WHERE ws = 'ws'", &[])
        .await
        .expect("read plugin_bytes")
        .get(0);
    assert!(
        plugin <= 2048,
        "plugin_bytes is {plugin}, which is past the 2048 cap the refusal named"
    );
    assert!(
        bytes <= 4096,
        "doc_bytes is {bytes}, which is past the 4096 cap"
    );
}

/// A change over `max_change` is a 413, and it takes no seq.
#[tokio::test]
async fn change_over_max_change_is_413() {
    let (_, _, url) = support::db::fresh_pair("change_over_max_change_is_413").await;
    let config = graph_store::StoreConfig::defaults();
    // A tiny `max_batch` is what shrinks `max_change`: it is `max_body + 96 × max_batch`, so the
    // cap moves without writing a body over the wire at all.
    let limits = graph_contract::hub::Limits {
        max_body: 64,
        max_batch: 1,
        max_record_bytes: 1 << 20,
    };
    let store = store_on(&url, config).await;
    store.create_workspace("ws", &limits).await.expect("create");
    store
        .put_manifest(&manifest_write("ws", "tracker"))
        .await
        .expect("register");

    let filler = "y".repeat(400);
    let mut req = batch_write(
        "ws",
        "tracker",
        batch_of(
            &[
                ("task", "1", 7, &format!(r#""name":"{filler}""#)),
                ("task", "2", 7, &format!(r#""name":"{filler}""#)),
            ],
            &[],
        ),
    );
    req.limits = limits;
    let error = store
        .apply_batch(&req)
        .await
        .expect_err("two 400-byte records are a change over 64 + 96 bytes");
    assert_eq!(status(&error), "413", "a change over max_change is a 413");

    let mut client = support::db::more(&url).await;
    assert_eq!(
        head_of(&mut client).await,
        1,
        "the refused change took no seq"
    );
    assert_eq!(
        seqs(&mut client).await.len(),
        1,
        "the manifest's change is the only one"
    );
}
