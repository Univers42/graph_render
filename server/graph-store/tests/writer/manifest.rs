//! §4's manifest PUT: growth, the caps, the conflict, and the lock order (N9).

use super::*;

/// [`GROWN`] with the `label` collection taken out and the version bumped: a removal, which §4
/// refuses because the records already stored carry that collection's declarations.
///
/// Written as its own text rather than spliced out of [`GROWN`], because the point of the case is
/// that the refusal names a *removed* declaration and a text built by `replace` would make that
/// depend on where the whitespace fell.
const REMOVES_LABEL: &str = r#"{
  "version": 1,
  "manifestVersion": 3,
  "name": "Tasks",
  "collections": [
    { "id": "task", "name": "Tasks", "titleField": "name", "fields": [
      { "id": "name", "name": "Name", "role": "title", "link": null },
      { "id": "note", "name": "Note", "role": "scalar", "link": null },
      { "id": "state", "name": "State", "role": "group", "link": null },
      { "id": "up", "name": "Up", "role": "parent", "link": null }
    ] }
  ]
}"#;

/// A first registration is 201 and takes a seq; a byte-identical resend is 200 and takes none; a
/// grown manifest is 200 and takes one; and both refusals are 409.
#[tokio::test]
async fn manifest_put_grows_and_refuses() {
    let (store, mut client, _, _) = ready("manifest_put_grows_and_refuses").await;
    assert_eq!(
        seqs(&mut client).await,
        vec![1],
        "the first registration took seq 1"
    );

    let again = store
        .put_manifest(&manifest_write("ws", "tracker"))
        .await
        .expect("the same manifest again");
    assert_eq!(again.status, 200, "an identical resend is a 200");
    assert_eq!(
        again.growth,
        graph_contract::hub::Growth::Same,
        "identical content at the same version is `Same`"
    );
    assert_eq!(again.seq, 1, "an identical resend takes no seq");
    assert_eq!(head_of(&mut client).await, 1, "head_seq did not move");

    let grown = store
        .put_manifest(&manifest_write_text("ws", "tracker", GROWN))
        .await
        .expect("a grown manifest");
    assert_eq!(grown.status, 200, "growing an existing plugin is a 200");
    assert_eq!(
        grown.growth,
        graph_contract::hub::Growth::Grown,
        "adding is growth"
    );
    assert_eq!(grown.seq, 2, "growth takes the next seq");
    let headers = client
        .query(
            "SELECT seq, kind, ops FROM change_headers WHERE ws = 'ws' ORDER BY seq",
            &[],
        )
        .await
        .expect("read the change headers");
    assert_eq!(headers.len(), 2, "two changes so far");
    assert_eq!(
        headers[1].get::<_, String>(1),
        "manifest",
        "a manifest change is logged as kind `manifest`"
    );
    assert_eq!(headers[1].get::<_, i32>(2), 0, "it carries no operations");

    let error = store
        .put_manifest(&manifest_write_text("ws", "tracker", REMOVES_LABEL))
        .await
        .expect_err("a manifest that removes a collection is a conflict");
    assert_eq!(
        status(&error),
        "409",
        "a removed collection is a 409, not growth"
    );

    let other = MANIFEST.replace(r#""name": "Tasks""#, r#""name": "Renamed""#);
    let error = store
        .put_manifest(&manifest_write_text("ws", "tracker", &other))
        .await
        .expect_err("the same version with other content is a conflict");
    assert_eq!(
        status(&error),
        "409",
        "the same manifestVersion with different content is a 409 (§4)"
    );
    assert_eq!(head_of(&mut client).await, 2, "a refused PUT takes no seq");
}

/// The 65th plugin is a 413, and the 64 before it are stored.
#[tokio::test]
async fn manifest_put_refuses_the_sixty_fifth_plugin() {
    let (store, client, _, _) = ready("manifest_put_refuses_65th_plugin").await;
    for i in 1..64 {
        let plugin = format!("p{i:02}");
        let written = store
            .put_manifest(&manifest_write("ws", &plugin))
            .await
            .unwrap_or_else(|e| panic!("plugin {plugin}: {e}"));
        assert_eq!(written.status, 201, "{plugin} is within the cap");
    }
    let error = store
        .put_manifest(&manifest_write("ws", "p64"))
        .await
        .expect_err("the 65th plugin is past MAX_PLUGINS");
    assert_eq!(status(&error), "413", "past the plugin cap is a 413");
    let stored: i64 = client
        .query_one("SELECT count(*) FROM manifests WHERE ws = 'ws'", &[])
        .await
        .expect("count the manifests")
        .get(0);
    assert_eq!(stored, 64, "the refused registration left nothing behind");
}

/// Each plugin's `plugin_seq` is the seq of its own last change, and another plugin's PUT does not
/// move it — the fact `If-Match` is per plugin (§5.1).
#[tokio::test]
async fn manifest_put_moves_only_its_own_plugin_seq() {
    let (store, client, _, _) = ready("manifest_put_moves_its_own_plugin_seq").await;
    let written = store
        .put_manifest(&manifest_write("ws", "other"))
        .await
        .expect("register a second plugin");
    assert_eq!(written.seq, 2, "the second registration took seq 2");
    let rows = client
        .query(
            "SELECT plugin, plugin_seq FROM manifests WHERE ws = 'ws' ORDER BY plugin",
            &[],
        )
        .await
        .expect("read both plugin rows");
    let seen: Vec<(String, i64)> = rows.iter().map(|row| (row.get(0), row.get(1))).collect();
    assert_eq!(
        seen,
        vec![("other".to_owned(), 2), ("tracker".to_owned(), 1)],
        "each plugin's plugin_seq names its own last change"
    );
}

/// The manifest PUT takes the workspace row lock first, so it waits for a manual holder rather than
/// deadlocking or passing through (N9).
///
/// WHY the wait is observed rather than the absence of a deadlock: "did not deadlock" is also what
/// a PUT that ignored the lock entirely would look like. The claim is that it *blocks*, so the case
/// holds the row from a second session and watches the PUT not finish.
#[tokio::test]
async fn manifest_put_takes_the_workspace_lock_first() {
    let (store, _, url, _) = ready("manifest_put_takes_the_workspace_lock_first").await;
    let manual = support::db::more(&url).await;
    manual
        .batch_execute("BEGIN")
        .await
        .expect("the manual session begins");
    manual
        .query_one("SELECT id FROM workspaces WHERE id = 'ws' FOR UPDATE", &[])
        .await
        .expect("the manual session holds the workspace row");

    let req = manifest_write("ws", "tracker");
    let waited = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        store.put_manifest(&req),
    )
    .await;
    assert!(
        waited.is_err(),
        "the PUT waited for the row lock instead of passing through it"
    );

    manual
        .batch_execute("ROLLBACK")
        .await
        .expect("the manual session releases it");
    let grown = store
        .put_manifest(&manifest_write("ws", "tracker"))
        .await
        .expect("the PUT completes once the row lock is free");
    assert_eq!(grown.status, 200, "the identical manifest is a 200");
}
