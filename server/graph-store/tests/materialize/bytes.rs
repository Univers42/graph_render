//! `doc_bytes` against the streamed length, and the refusal for a workspace that is not there.

use super::*;

/// A dangling `blocks` and a dangling parent: pruned, so the document is shorter than counted.
async fn pruned(name: &str) -> Twin {
    let mut twin = Twin::new(name, 2).await;
    twin.register("tracker", TRACKER).await;
    twin.register("other", OTHER).await;
    let tasks = batch_of(
        &[
            ("task", "a", 1, r#""name":"A","blocks":["gone"],"up":"gone""#),
            ("task", "b", 1, r#""name":"B""#),
        ],
        &[],
    );
    assert!(twin.apply("tracker", tasks).await, "the tasks apply");
    twin
}

/// The store's count is an upper bound of the document and of the Model's own count.
#[tokio::test]
async fn doc_bytes_bounds_the_document() {
    let twin = pruned("bytes_bound").await;
    let (doc, bytes, _) = read_all(&twin.store).await;
    assert_eq!(doc, twin.model.to_json());
    assert!(bytes > doc.len() as u64, "{bytes} over {}", doc.len());
    assert!(bytes >= twin.model.doc_bytes(), "{bytes}");
}

/// With every reference resolved the count is exact, on both sides.
#[tokio::test]
async fn doc_bytes_equals_the_streamed_length_when_nothing_is_pruned() {
    let twin = super::equal::seeded("bytes_exact", 2).await;
    let (doc, bytes, _) = read_all(&twin.store).await;
    assert_eq!(bytes, doc.len() as u64);
    assert_eq!(bytes, twin.model.doc_bytes());
}

/// `open` on a workspace no one created is `NotFound`, not an empty document.
#[tokio::test]
async fn a_missing_workspace_is_not_found() {
    let twin = Twin::new("bytes_missing_ws", 2).await;
    let opened = graph_store::materialize::open(&twin.store, "nope").await;
    assert!(
        matches!(opened, Err(graph_store::StoreError::NotFound { .. })),
        "{:?}",
        opened.err()
    );
}
