//! The request log and CORS (Verdict conditions 9 and 12), row `svc-log`: one JSON line per
//! request with its id, key name, route, status, duration, layout, post and size, and never the
//! key or its hash (the row's negative control logs the header). The preflight needs no key and
//! answers configured origins only, on `/v1/` only.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{doc, server};

#[tokio::test]
async fn one_line_per_request_with_every_field() {
    let server = server(&[]);
    let request = server.request(
        "POST",
        "/v1/layout?layout=layout.grid&post=post.style.bezier",
    );
    let request = request.header("x-request-id", "trace-7");
    let reply = server
        .send(request.body(Body::from(doc(6, 5))).unwrap())
        .await;
    assert_eq!(reply.status, StatusCode::OK);
    let lines = server.lines();
    assert_eq!(lines.len(), 1, "{lines:?}");
    let line = &lines[0];
    let expected = serde_json::json!({
        "event": "request", "id": "trace-7", "method": "POST", "key": "tester",
        "route": "/v1/layout", "status": 200, "layout": "layout.grid",
        "post": "post.style.bezier", "n": 6, "m": 5,
    });
    for (field, value) in expected.as_object().unwrap() {
        assert_eq!(&line[field], value, "{field}");
    }
    assert!(line["ms"].is_u64(), "{line}");
}

#[tokio::test]
async fn no_line_holds_the_key_or_its_hash() {
    let server = server(&[]);
    let hash = std::fs::read_to_string(server.dir.join("keys")).unwrap();
    let hash = hash.split_whitespace().nth(1).unwrap().to_owned();
    server.get("/v1/meta").await;
    server.layout("layout=layout.grid", doc(3, 2)).await;
    let wrong = Request::builder()
        .uri("/v1/meta")
        .header("authorization", format!("Bearer {}x", server.key));
    server.send(wrong.body(Body::empty()).unwrap()).await;
    let log = server.log.lock().unwrap().join("\n");
    assert_eq!(server.log.lock().unwrap().len(), 3);
    assert!(!log.contains(&server.key), "a log line holds the key");
    assert!(!log.contains(&hash), "a log line holds the key's hash");
    assert!(!log.to_ascii_lowercase().contains("bearer"), "{log}");
}

const ORIGIN: &str = "https://studio.example";

fn preflight(path: &str, origin: &str) -> Request<Body> {
    Request::builder()
        .method("OPTIONS")
        .uri(path)
        .header("origin", origin)
        .header("access-control-request-method", "POST")
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn a_preflight_needs_no_key_and_a_configured_origin() {
    let server = server(&[("GRAPH_CORS_ORIGINS", ORIGIN)]);
    let reply = server.send(preflight("/v1/layout", ORIGIN)).await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    assert_eq!(reply.header("access-control-allow-origin"), ORIGIN);
    assert_eq!(reply.header("access-control-allow-methods"), "GET, POST");
    for (path, origin) in [
        ("/v1/layout", "https://other.example"),
        ("/v1/layout", "https://studio.example.evil"),
        ("/healthz", ORIGIN),
        ("/embed/0123456789abcdef/a.js", ORIGIN),
    ] {
        let reply = server.send(preflight(path, origin)).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND, "{path} from {origin}");
        assert_eq!(reply.header("access-control-allow-origin"), "", "{path}");
    }
}

#[tokio::test]
async fn cors_headers_ride_on_v1_for_a_configured_origin_only() {
    let server = server(&[("GRAPH_CORS_ORIGINS", ORIGIN)]);
    let with_origin = |path: &str, origin: &str| {
        let request = server.request("GET", path).header("origin", origin);
        request.body(Body::empty()).unwrap()
    };
    let reply = server.send(with_origin("/v1/meta", ORIGIN)).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.header("access-control-allow-origin"), ORIGIN);
    assert_eq!(
        reply.header("access-control-expose-headers"),
        "x-request-id"
    );
    assert!(
        reply.header("vary").contains("origin"),
        "{:?}",
        reply.headers
    );
    for (path, origin) in [("/v1/meta", "https://other.example"), ("/healthz", ORIGIN)] {
        let reply = server.send(with_origin(path, origin)).await;
        assert_eq!(
            reply.header("access-control-allow-origin"),
            "",
            "{path} from {origin}"
        );
    }
}

#[tokio::test]
async fn without_configured_origins_no_preflight_is_answered() {
    let server = server(&[]);
    let reply = server.send(preflight("/v1/layout", ORIGIN)).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}
