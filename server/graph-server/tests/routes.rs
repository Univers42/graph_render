//! The routes answer: `/healthz`, `/v1/meta`, `/v1/layout` on both faces, and the fallbacks.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{doc, fixture, server};
use graph_contract::binary::Snapshot;

#[tokio::test]
async fn healthz_needs_no_key() {
    let server = server(&[]);
    let request = Request::get("/healthz").body(Body::empty()).unwrap();
    let reply = server.send(request).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(&reply.body[..], b"ok");
}

#[tokio::test]
async fn meta_lists_the_registries_and_no_analyses() {
    let server = server(&[]);
    let reply = server.get("/v1/meta").await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.header("content-type"), "application/json");
    let meta: serde_json::Value = serde_json::from_slice(&reply.body).unwrap();
    assert_eq!(meta["api"], 1);
    assert_eq!(meta["abi"], graph_wasm::ABI_VERSION);
    let layouts: Vec<&str> = graph_server::motor::layout_ids().collect();
    let posts: Vec<&str> = graph_server::motor::post_ids().collect();
    assert_eq!(meta["layouts"], serde_json::json!(layouts));
    assert_eq!(meta["posts"], serde_json::json!(posts));
    assert!(meta.get("analyses").is_none(), "{meta}");
}

#[tokio::test]
async fn meta_refuses_a_parameter() {
    let server = server(&[]);
    let reply = server.get("/v1/meta?layout=x").await;
    assert_eq!(
        (reply.status, reply.code().as_str()),
        (StatusCode::BAD_REQUEST, "BadRequest")
    );
}

#[tokio::test]
async fn layout_answers_the_binary_face_by_default() {
    let server = server(&[]);
    let reply = server
        .layout("layout=layout.grid", fixture("fixtures/scale/n220.json"))
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    assert_eq!(
        reply.header("content-type"),
        "application/vnd.graph-motor.snapshot"
    );
    assert_eq!(reply.header("vary"), "Accept");
    Snapshot::from_bytes(&reply.body).expect("the binary face decodes");
}

#[tokio::test]
async fn layout_answers_the_json_face_on_request() {
    let server = server(&[]);
    let request = server
        .request(
            "POST",
            "/v1/layout?layout=layout.circular.ring&post=post.style.bezier",
        )
        .header("accept", "application/json")
        .body(Body::from(doc(30, 40)))
        .unwrap();
    let reply = server.send(request).await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.body);
    assert_eq!(reply.header("content-type"), "application/json");
    let text = std::str::from_utf8(&reply.body).unwrap();
    graph_contract::canonical_json::from_json(text).expect("the JSON face parses");
}

#[tokio::test]
async fn an_unknown_route_or_method_is_404() {
    let server = server(&[]);
    for (method, path) in [
        ("GET", "/nope"),
        ("GET", "/v1/layout"),
        ("DELETE", "/v1/meta"),
    ] {
        let request = server.request(method, path).body(Body::empty()).unwrap();
        let reply = server.send(request).await;
        assert_eq!(
            (reply.status, reply.code().as_str()),
            (StatusCode::NOT_FOUND, "NotFound")
        );
    }
}

#[tokio::test]
async fn a_request_id_is_echoed_or_generated() {
    let server = server(&[]);
    let request = server
        .request("GET", "/healthz")
        .header("x-request-id", "abc-1");
    let reply = server.send(request.body(Body::empty()).unwrap()).await;
    assert_eq!(reply.header("x-request-id"), "abc-1");
    for bad in ["has space", &"x".repeat(129)] {
        let request = server
            .request("GET", "/healthz")
            .header("x-request-id", bad);
        let reply = server.send(request.body(Body::empty()).unwrap()).await;
        let id = reply.header("x-request-id");
        assert!(!id.is_empty() && id != bad, "{id}");
    }
}
