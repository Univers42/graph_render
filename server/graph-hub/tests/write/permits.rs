//! The WRITERS gate across all three write routes: with the one permit held, each route waits and
//! then 503s, which is the row's own evidence the permit is on the route.

use axum::body::Body;

use crate::support::fixtures::{hub_db, manifest_at, upsert};

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
