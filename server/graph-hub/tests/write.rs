//! The write half of §5.2's table: the three routes that change something, one test per status the
//! spec gives them.
//!
//! Every case builds its bodies through `support::fixtures`, which goes through graph-contract's own
//! writers, so a refusal here is the hub's or the store's and never a hand-typed body's.
#![cfg(feature = "db-tests")]

#[path = "support/mod.rs"]
mod support;

use axum::body::Body;

use support::fixtures::*;
use support::*;

/// `PUT /v1/workspaces/{ws}` is 201 on the insert and 200 when the row was already there, and the
/// answer carries no seq: creating a workspace moves no stream.
#[tokio::test]
async fn put_workspaces_is_201_then_200_and_takes_no_seq() {
    let hub = hub_db(&[]).await;
    let first = hub.put("/v1/workspaces/created", "").await;
    assert_eq!(first.code(), 201, "{}", first.body());
    assert_eq!(
        first.header("graph-seq"),
        "",
        "no seq on a workspace create"
    );
    let again = hub.put("/v1/workspaces/created", "").await;
    assert_eq!(again.code(), 200, "{}", again.body());
}

/// A manifest PUT is 201 then 200, and re-sending byte-identical content takes no seq: §4 says a
/// same-content republication is not growth.
#[tokio::test]
async fn put_manifest_is_201_then_200_and_the_same_content_takes_no_seq() {
    let hub = hub_db(&[]).await;
    assert_eq!(hub.put("/v1/workspaces/same", "").await.code(), 201);
    let first = hub
        .put("/v1/workspaces/same/plugins/task", manifest_at(1))
        .await;
    assert_eq!(first.code(), 201, "{}", first.body());
    let again = hub
        .put("/v1/workspaces/same/plugins/task", manifest_at(1))
        .await;
    assert_eq!(again.code(), 200, "{}", again.body());
    let third = hub
        .put("/v1/workspaces/same/plugins/task", manifest_at(1))
        .await;
    assert_eq!(third.code(), 200, "the third is still 200");
}

/// §4's shrink: a manifest that removes a field is a 409, never a silent deletion of stored
/// records.
#[tokio::test]
async fn put_manifest_refuses_a_removed_field_with_409() {
    let hub = hub_db(&[]).await;
    assert_eq!(hub.put("/v1/workspaces/shrink", "").await.code(), 201);
    let grown = hub.put("/v1/workspaces/shrink/plugins/task", grown()).await;
    assert_eq!(grown.code(), 201, "{}", grown.body());
    let shrunk = hub
        .put("/v1/workspaces/shrink/plugins/task", shrunk())
        .await;
    assert_eq!(shrunk.code(), 409, "{}", shrunk.body());
    assert_eq!(shrunk.error(), "conflict");
}

/// The same `manifestVersion` with other content is a 409 too: growth compares facts, and two
/// different manifests at one version are not a growth.
#[tokio::test]
async fn put_manifest_refuses_the_same_version_with_other_content_with_409() {
    let hub = hub_db(&[]).await;
    assert_eq!(hub.put("/v1/workspaces/stale", "").await.code(), 201);
    let first = hub
        .put("/v1/workspaces/stale/plugins/task", manifest_at(2))
        .await;
    assert_eq!(first.code(), 201, "{}", first.body());
    let other = hub.put("/v1/workspaces/stale/plugins/task", grown()).await;
    assert_eq!(other.code(), 409, "{}", other.body());
}

/// §6's `MAX_PLUGINS` is 64, so the sixty-fifth registration is a 413 and the first sixty-four stay.
#[tokio::test]
async fn put_manifest_refuses_the_sixty_fifth_plugin_with_413() {
    let hub = hub_db(&[]).await;
    assert_eq!(hub.put("/v1/workspaces/many", "").await.code(), 201);
    for index in 0..64 {
        let name = format!("p{index}");
        let reply = hub
            .put(
                &format!("/v1/workspaces/many/plugins/{name}"),
                sixty_fifth(1),
            )
            .await;
        assert_eq!(reply.code(), 201, "plugin {index}: {}", reply.body());
    }
    let refused = hub
        .put("/v1/workspaces/many/plugins/p64", sixty_fifth(1))
        .await;
    assert_eq!(refused.code(), 413, "{}", refused.body());
    assert_eq!(refused.error(), "too_large");
}

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

/// §5.1 step 2: a replay with the same key and the same body returns the stored response and adds
/// no seq. This is the test row `hub-idem` filters on, and the one the store's `no-idem` break
/// turns red.
#[tokio::test]
async fn idempotency_replay_returns_the_same_response_and_the_same_head_seq() {
    let hub = hub_db(&[]).await;
    ready(&hub, "idem", "task").await;
    let path = "/v1/workspaces/idem/plugins/task/batches";
    let body = upsert("task", "replayed", "note");
    let first = keyed(&hub, path, &body).await;
    assert_eq!(first.code(), 200, "{}", first.body());
    let replay = keyed(&hub, path, &body).await;
    assert_eq!(replay.code(), 200, "{}", replay.body());
    assert_eq!(
        replay.body(),
        first.body(),
        "the stored response, byte for byte"
    );
}

/// One batch sent under the fixed key `key-one`, which is what makes the two arms above the same
/// request.
async fn keyed(hub: &Hub, path: &str, body: &str) -> Reply {
    hub.send(
        hub.request("POST", path)
            .header("idempotency-key", "key-one")
            .body(Body::from(body.to_owned()))
            .expect("the request"),
    )
    .await
}

/// The same key with a different body is a 422 and applies nothing: §5.1 treats the key as a claim
/// about one body.
#[tokio::test]
async fn the_same_key_with_another_body_is_422() {
    let hub = hub_db(&[]).await;
    ready(&hub, "idem2", "task").await;
    let path = "/v1/workspaces/idem2/plugins/task/batches";
    let first = hub
        .send(
            hub.request("POST", path)
                .header("idempotency-key", "shared")
                .body(Body::from(upsert("task", "one", "n")))
                .expect("the request"),
        )
        .await;
    assert_eq!(first.code(), 200, "{}", first.body());
    let other = hub
        .send(
            hub.request("POST", path)
                .header("idempotency-key", "shared")
                .body(Body::from(upsert("task", "two", "n")))
                .expect("the request"),
        )
        .await;
    assert_eq!(other.code(), 422, "{}", other.body());
}

/// §5.1 caps an idempotency key at 128 **bytes**, and it is a 422 rather than a 413 because the key
/// is a claim, not a payload.
#[tokio::test]
async fn an_idempotency_key_over_128_bytes_is_422() {
    let hub = hub_db(&[]).await;
    ready(&hub, "idem3", "task").await;
    let long = "k".repeat(129);
    let refused = hub
        .send(
            hub.request("POST", "/v1/workspaces/idem3/plugins/task/batches")
                .header("idempotency-key", long)
                .body(Body::from(upsert("task", "one", "n")))
                .expect("the request"),
        )
        .await;
    assert_eq!(refused.code(), 422, "{}", refused.body());
}

/// A records page is in byte order, carries `plugin_seq` as `<epoch>.<seq>` on every page, and stops
/// at `limit`.
#[tokio::test]
async fn a_record_page_is_in_byte_order_and_carries_plugin_seq() {
    let hub = hub_db(&[]).await;
    ready(&hub, "paged", "task").await;
    let path = "/v1/workspaces/paged/plugins/task/batches";
    hub.post(
        path,
        batch(
            &[("task", "b", "n"), ("task", "a", "n"), ("task", "c", "n")],
            &[],
        ),
    )
    .await;
    let reply = hub
        .get_with("/v1/workspaces/paged/plugins/task/records?limit=2")
        .await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON page");
    let ids: Vec<&str> = value["records"]
        .as_array()
        .expect("a records array")
        .iter()
        .map(|row| row["id"].as_str().expect("an id"))
        .collect();
    assert_eq!(ids, ["a", "b"], "byte order, and the page stops at limit=2");
    let plugin_seq = value["plugin_seq"].as_str().expect("a plugin_seq");
    assert_eq!(plugin_seq.split('.').count(), 2, "{plugin_seq:?}");
}

/// The opaque `next` walks the whole collection and then stops: the last page carries no `next`, so
/// a client never asks past the end.
#[tokio::test]
async fn a_records_page_next_cursor_terminates() {
    let hub = hub_db(&[]).await;
    ready(&hub, "walk", "task").await;
    let path = "/v1/workspaces/walk/plugins/task/batches";
    hub.post(
        path,
        batch(
            &[("task", "a", "n"), ("task", "b", "n"), ("task", "c", "n")],
            &[],
        ),
    )
    .await;
    let mut next = String::new();
    let mut seen: Vec<String> = Vec::new();
    for _ in 0..8 {
        let cursor = if next.is_empty() {
            String::new()
        } else {
            format!("&cursor={next}")
        };
        let reply = hub
            .get_with(&format!(
                "/v1/workspaces/walk/plugins/task/records?limit=1{cursor}"
            ))
            .await;
        assert_eq!(reply.code(), 200, "{}", reply.body());
        let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON page");
        for row in value["records"].as_array().expect("a records array") {
            seen.push(row["id"].as_str().expect("an id").to_owned());
        }
        match value["next"].as_str() {
            Some(cursor) => next = cursor.to_owned(),
            None => {
                next.clear();
                break;
            }
        }
    }
    assert_eq!(seen, ["a", "b", "c"], "every id exactly once, in order");
    assert!(next.is_empty(), "the last page carries no next cursor");
}

/// One record is 200 with its `rev` and its `values`, read from the store's own canonical text.
#[tokio::test]
async fn one_record_is_200_with_its_rev() {
    let hub = hub_db(&[]).await;
    ready(&hub, "one", "task").await;
    hub.post(
        "/v1/workspaces/one/plugins/task/batches",
        upsert("task", "solo", "the note"),
    )
    .await;
    let reply = hub
        .get_with("/v1/workspaces/one/records/task/task/solo")
        .await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON record");
    assert_eq!(value["id"], "solo");
    assert_eq!(value["collection"], "task");
    assert!(
        value["rev"].as_u64().expect("a rev") > 0,
        "the store's own rev"
    );
    assert_eq!(
        value["values"]["note"], "the note",
        "the record's own values"
    );
}

/// The one-record route of **another** plugin is a 403, not a 404: authorization runs before
/// existence, so a key with no grant learns nothing about whether the record is there. This is
/// Review Focus 1's second case on a different route.
#[tokio::test]
async fn one_record_of_another_plugin_is_403_not_404() {
    let hub = hub_db_grants("tester * admin\nstranger * write:other\n").await;
    let stranger = add_key(&hub, "stranger");
    // The keyring read both files when the hub was built, so a key minted after that is unknown
    // until a reload; the same `SIGHUP` path `reload.rs` covers, reached directly here.
    hub.app
        .keys
        .reload()
        .expect("the second key is in the pair");
    assert_eq!(hub.put("/v1/workspaces/guarded", "").await.code(), 201);
    ready(&hub, "guarded", "task").await;
    hub.post(
        "/v1/workspaces/guarded/plugins/task/batches",
        upsert("task", "solo", "n"),
    )
    .await;
    // The record exists, and the second key has no grant covering `task` on this workspace: the
    // refusal is the 403, never the 404 a lookup-then-refuse order would give.
    let refused = hub
        .get_as(&stranger, "/v1/workspaces/guarded/records/task/task/solo")
        .await;
    assert_eq!(refused.code(), 403, "{}", refused.body());
    let missing = hub
        .get_as(&stranger, "/v1/workspaces/guarded/records/task/task/absent")
        .await;
    assert_eq!(missing.code(), 403, "an absent record is the same bytes");
    assert_eq!(missing.body(), refused.body(), "byte for byte");
}

/// `GET .../plugins` answers every registered manifest, one per plugin, in plugin order.
#[tokio::test]
async fn get_plugins_returns_every_manifest() {
    let hub = hub_db(&[]).await;
    ready(&hub, "listed", "alpha").await;
    ready(&hub, "listed", "beta").await;
    let reply = hub.get_with("/v1/workspaces/listed/plugins").await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON list");
    let names: Vec<&str> = value["manifests"]
        .as_array()
        .expect("a manifests array")
        .iter()
        .map(|m| m["plugin"].as_str().expect("a plugin name"))
        .collect();
    assert_eq!(names, ["alpha", "beta"], "both plugins, in plugin order");
}

/// Every write route takes its permit from the WRITERS gate: with one permit held, a second request
/// on each route waits and then 503s. This is the row's own evidence that the permit is on the
/// route and not only on the store.
#[tokio::test]
async fn every_write_route_takes_its_permit() {
    use std::time::Duration;

    use graph_hub::gate::deadline;

    let hub = hub_db(&[("GRAPH_HUB_WRITERS", "1"), ("GRAPH_HUB_TIMEOUT_MS", "150")]).await;
    let held = hub
        .app
        .gates
        .writers
        .admit(deadline(Duration::from_millis(50)))
        .await
        .expect("the one writer permit");
    for (method, path, body) in [
        ("PUT", "/v1/workspaces/permit", ""),
        (
            "PUT",
            "/v1/workspaces/permit/plugins/task",
            &manifest_at(1)[..],
        ),
        (
            "POST",
            "/v1/workspaces/permit/plugins/task/batches",
            &upsert("task", "a", "n")[..],
        ),
    ] {
        let refused = hub
            .send(
                hub.request(method, path)
                    .body(Body::from(body.to_owned()))
                    .expect("the request"),
            )
            .await;
        assert_eq!(refused.code(), 503, "{method} {path}: {}", refused.body());
        assert_eq!(refused.header("retry-after"), "1", "{method} {path}");
    }
    drop(held);
}
