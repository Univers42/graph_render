//! `POST .../batches`: the answer's `seq`/`applied` and `Graph-Seq`, §5.1's atomicity and
//! `If-Match`, and the two body-grammar refusals the store owns.

use axum::body::Body;

use crate::support::fixtures::{batch, hub_db, ready, upsert};

/// A batch answers `{"seq","applied"}` — graph-contract's `answer_json`, which the hub relays
/// whole — and carries `Graph-Seq: <epoch>.<seq>` so a caller can subscribe from the answer.
#[tokio::test]
async fn post_batch_answers_seq_and_applied_and_a_graph_seq_header() {
    let hub = hub_db(&[]).await;
    ready(&hub, "answers", "task").await;
    let reply = hub
        .post(
            "/v1/workspaces/answers/plugins/task/batches",
            upsert("task", "one", "first"),
        )
        .await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let body: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON answer");
    assert_eq!(body["applied"], 1, "{}", reply.body());
    let seq = body["seq"].as_u64().expect("a seq");
    assert!(seq > 0, "the answer carries the seq it took");
    let graph_seq = reply.header("graph-seq");
    assert!(
        graph_seq.ends_with(&format!(".{seq}")),
        "Graph-Seq is <epoch>.<seq>: {graph_seq:?}"
    );
}

/// An identical upsert applies nothing and says so: §5.1 step 5 only moves `head_seq` when something
/// was applied.
#[tokio::test]
async fn an_identical_upsert_answers_applied_zero() {
    let hub = hub_db(&[]).await;
    ready(&hub, "noop", "task").await;
    let body = upsert("task", "same", "note");
    let path = "/v1/workspaces/noop/plugins/task/batches";
    let first = hub.post(path, body.clone()).await;
    assert_eq!(first.code(), 200, "{}", first.body());
    let again = hub.post(path, body).await;
    assert_eq!(again.code(), 200, "{}", again.body());
    let value: serde_json::Value = serde_json::from_str(&again.body()).expect("a JSON answer");
    assert_eq!(value["applied"], 0, "an identical upsert applies nothing");
}

/// §5.1's atomicity: one bad record in a batch changes nothing at all, and the answer is 422 with
/// no seq.
#[tokio::test]
async fn a_batch_with_one_bad_record_changes_nothing() {
    let hub = hub_db(&[]).await;
    ready(&hub, "atomic", "task").await;
    let path = "/v1/workspaces/atomic/plugins/task/batches";
    let good = hub.post(path, upsert("task", "kept", "note")).await;
    assert_eq!(good.code(), 200, "{}", good.body());
    let before: serde_json::Value = serde_json::from_str(&good.body()).expect("a JSON answer");
    // The second upsert names a collection the manifest does not declare, so `Batch::check` refuses
    // the whole body rather than the one record.
    let refused = hub
        .post(
            path,
            batch(&[("task", "ok", "n"), ("elsewhere", "bad", "n")], &[]),
        )
        .await;
    assert_eq!(refused.code(), 422, "{}", refused.body());
    let after = hub.post(path, upsert("task", "kept", "note")).await;
    let value: serde_json::Value = serde_json::from_str(&after.body()).expect("a JSON answer");
    assert_eq!(
        value["applied"], 0,
        "the refused batch changed nothing, so the kept record is unchanged"
    );
    assert!(
        value["seq"].as_u64().unwrap() >= before["seq"].as_u64().unwrap(),
        "the refused batch took no seq"
    );
}

/// §5.1's `If-Match`: a stale `plugin_seq` is a 412, and another plugin's writes never move it.
#[tokio::test]
async fn if_match_gives_412_on_a_stale_plugin_seq() {
    let hub = hub_db(&[]).await;
    ready(&hub, "precond", "task").await;
    ready(&hub, "precond", "other").await;
    let page = hub
        .get_with("/v1/workspaces/precond/plugins/task/records")
        .await;
    assert_eq!(page.code(), 200, "{}", page.body());
    let value: serde_json::Value = serde_json::from_str(&page.body()).expect("a JSON page");
    let plugin_seq = value["plugin_seq"]
        .as_str()
        .expect("a plugin_seq")
        .to_owned();

    // Another plugin's write must not move this plugin's seq.
    hub.post(
        "/v1/workspaces/precond/plugins/other/batches",
        upsert("task", "elsewhere", "n"),
    )
    .await;

    let stale = hub
        .send(
            hub.request("POST", "/v1/workspaces/precond/plugins/task/batches")
                .header("if-match", "1.999")
                .body(Body::from(upsert("task", "one", "n")))
                .expect("the request"),
        )
        .await;
    assert_eq!(stale.code(), 412, "{}", stale.body());

    let fresh = hub
        .send(
            hub.request("POST", "/v1/workspaces/precond/plugins/task/batches")
                .header("if-match", plugin_seq)
                .body(Body::from(upsert("task", "one", "n")))
                .expect("the request"),
        )
        .await;
    assert_eq!(fresh.code(), 200, "{}", fresh.body());
}

#[tokio::test]
async fn a_qualified_collection_in_a_body_is_422() {
    let hub = hub_db(&[]).await;
    ready(&hub, "qualified", "task").await;
    // §4's collection grammar inside a **body**: `other.coll` is a qualified id and a body may not
    // qualify, because the plugin comes from the path. This is the case row `negctl-lax-reader`
    // turns red: graph-contract's `lax-reader` is exactly the switch that drops that check.
    let refused = hub
        .post(
            "/v1/workspaces/qualified/plugins/task/batches",
            batch(&[("other.coll", "one", "n")], &[]),
        )
        .await;
    assert_eq!(refused.code(), 422, "{}", refused.body());
}

#[tokio::test]
async fn a_nul_anywhere_in_a_body_is_422() {
    let hub = hub_db(&[]).await;
    ready(&hub, "nul", "task").await;
    // §4's body NUL rule is graph-contract's (`HubError::Nul`, `crates/graph-contract/src/hub/strict.rs:30`),
    // and the walk is over the parsed tree, so a NUL in a cell value counts exactly as one in a key
    // would. This is the case row `negctl-lax-reader` turns red.
    let body = concat!(
        r#"{"upserts":[{"collection":"task","id":"one","updatedAt":1,""#,
        r#""values":{"name":"one","note":"a\u0000b"}}],"deletes":[]}"#
    );
    let refused = hub
        .post("/v1/workspaces/nul/plugins/task/batches", body)
        .await;
    assert_eq!(refused.code(), 422, "{}", refused.body());
}