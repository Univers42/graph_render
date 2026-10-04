//! The key check (Verdict condition 9), row `svc-auth`: one `Authorization: Bearer <key>`, the
//! scheme in any case; every other way of sending a key is refused, and a refusal reads no
//! body.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{doc, server};

fn refused(status: StatusCode, code: &str) -> (StatusCode, String) {
    (status, code.to_owned())
}

#[tokio::test]
async fn a_missing_wrong_or_truncated_key_is_401_with_a_challenge() {
    let server = server(&[]);
    let truncated = format!("Bearer {}", &server.key[..server.key.len() - 1]);
    let wrong = format!("Bearer {}x", server.key);
    for credential in [
        None,
        Some("Bearer"),
        Some("Basic Zm9vOmJhcg=="),
        Some(&*truncated),
        Some(&*wrong),
    ] {
        let mut request = Request::get("/v1/meta");
        if let Some(value) = credential {
            request = request.header("authorization", value);
        }
        let reply = server.send(request.body(Body::empty()).unwrap()).await;
        assert_eq!(
            (reply.status, reply.code()),
            refused(StatusCode::UNAUTHORIZED, "Unauthorized")
        );
        assert_eq!(reply.header("www-authenticate"), "Bearer");
    }
}

#[tokio::test]
async fn the_scheme_is_case_insensitive() {
    let server = server(&[]);
    for scheme in ["Bearer", "bearer", "BEARER", "bEaReR"] {
        let value = format!("{scheme} {}", server.key);
        let request = Request::get("/v1/meta").header("authorization", value);
        let reply = server.send(request.body(Body::empty()).unwrap()).await;
        assert_eq!(reply.status, StatusCode::OK, "{scheme}");
    }
}

#[tokio::test]
async fn a_second_authorization_header_is_400() {
    let server = server(&[]);
    let value = format!("Bearer {}", server.key);
    let request = Request::get("/v1/meta")
        .header("authorization", &value)
        .header("authorization", &value);
    let reply = server.send(request.body(Body::empty()).unwrap()).await;
    assert_eq!(
        (reply.status, reply.code()),
        refused(StatusCode::BAD_REQUEST, "BadRequest")
    );
}

#[tokio::test]
async fn a_key_in_the_query_is_400_and_never_echoed() {
    let server = server(&[]);
    let key = server.key.clone();
    for query in [
        format!("key={key}"),
        format!("api_key={key}"),
        format!("x={key}"),
        "token=abc".into(),
    ] {
        let uri = format!("/v1/layout?layout=layout.grid&{query}");
        let request = Request::post(uri).body(Body::from(doc(3, 2))).unwrap();
        let reply = server.send(request).await;
        assert_eq!(
            (reply.status, reply.code()),
            refused(StatusCode::BAD_REQUEST, "BadRequest")
        );
        assert!(
            !String::from_utf8_lossy(&reply.body).contains(&key[3..]),
            "{query}"
        );
    }
}

#[tokio::test]
async fn auth_off_serves_without_a_key_on_loopback() {
    let server = server(&[("GRAPH_AUTH", "off")]);
    let reply = server
        .send(Request::get("/v1/meta").body(Body::empty()).unwrap())
        .await;
    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn a_refused_key_reads_no_body() {
    let server = server(&[]);
    // A handler that read this body would answer 400, not 401.
    let request = Request::post("/v1/layout?layout=layout.grid").body(common::failing_body());
    let request = request.unwrap();
    let reply = server.send(request).await;
    assert_eq!(
        (reply.status, reply.code()),
        refused(StatusCode::UNAUTHORIZED, "Unauthorized")
    );
}
