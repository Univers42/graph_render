//! Temporary isolation probe: collect the SSE body through oneshot instead of a socket.
#![cfg(feature = "db-tests")]

#[path = "support/mod.rs"]
mod support;

use http_body_util::BodyExt;
use tower::ServiceExt;

use support::fixtures::{hub_db, ready, upsert};
use support::*;

#[tokio::test]
async fn iso_frames() {
    let hub = hub_db(&[]).await;
    let ws = support::db::unique("iso");
    ready(&hub, &ws, "task").await;
    hub.post(
        &format!("/v1/workspaces/{ws}/plugins/task/batches"),
        upsert("task", "one", "n"),
    )
    .await;
    let epoch = {
        let body = hub.get_with("/v1/workspaces").await.body().to_owned();
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        v["workspaces"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == ws)
            .unwrap()["epoch"]
            .as_u64()
            .unwrap()
            .to_string()
    };
    let response = hub
        .router
        .clone()
        .oneshot(
            hub.request("GET", &format!("/v1/workspaces/{ws}/events?since={epoch}.0"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    eprintln!("ISO status={}", response.status());
    let mut body = response.into_body();
    for _ in 0..4 {
        match tokio::time::timeout(std::time::Duration::from_millis(800), body.frame()).await {
            Ok(Ok(Some(frame))) => {
                let data = frame.into_data().unwrap_or_default();
                eprintln!("ISO frame {:?}", String::from_utf8_lossy(&data));
            }
            other => {
                eprintln!("ISO end {other:?}");
                break;
            }
        }
    }
}
