//! The `Authorization` header as the hub reads it: absent, doubled, not `Bearer`, and the one
//! function that turns a header into a name.
//!
//! Every case goes through `auth::credential` or through the router's own header reading, so a test
//! that passes is a statement about the shipped parser and not about a copy of its rule.

use axum::body::Body;
use graph_hub::auth;

use crate::support::{KEY_NAME, hub_with_env};

/// No `Authorization` header is 401, and the body is the same one an unknown key gets.
#[tokio::test]
async fn no_authorization_header_is_the_same_401_as_an_unknown_key() {
    let hub = hub_with_env(&[]);
    let anonymous = hub
        .send(
            hub.anonymous("GET", "/v1/workspaces")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    let unknown = hub.get_as("gm_not_a_key_at_all", "/v1/workspaces").await;
    assert_eq!(anonymous.code(), 401, "{}", anonymous.body());
    assert_eq!(unknown.code(), 401, "{}", unknown.body());
    assert_eq!(anonymous.body(), unknown.body());
}

/// A second `Authorization` header is **400**, not 401: RFC 9110 makes a message with two of the same
/// field invalid, and §5.2 requires exactly this. It is the one behaviour the hub does that
/// graph-server's own credential check does not.
#[tokio::test]
async fn a_second_authorization_header_is_400() {
    let hub = hub_with_env(&[]);
    let request = hub
        .request("GET", "/v1/workspaces")
        .header("authorization", "Bearer gm_second")
        .body(Body::empty())
        .unwrap();
    let reply = hub.send(request).await;
    assert_eq!(reply.code(), 400, "{}", reply.body());
    assert_eq!(reply.error(), "BadRequest");
}

/// A credential that is not `Bearer` at all is 401, and never 400: the hub reuses
/// `graph_server::auth::bearer`, which is case-insensitive about the scheme and refuses anything else.
#[tokio::test]
async fn a_non_bearer_credential_is_401() {
    let hub = hub_with_env(&[]);
    for value in ["Basic abc", "Bearer", "gm_raw_token"] {
        let request = hub
            .anonymous("GET", "/v1/workspaces")
            .header("authorization", value)
            .body(Body::empty())
            .unwrap();
        let reply = hub.send(request).await;
        assert_eq!(reply.code(), 401, "{value}: {}", reply.body());
    }
}

/// The scheme is case-insensitive (RFC 6750 §2.1), and this is graph-server's own `bearer`, so the
/// hub inherits the case-insensitivity rather than re-deciding it.
#[tokio::test]
async fn the_bearer_scheme_is_case_insensitive() {
    let hub = hub_with_env(&[]);
    for scheme in ["Bearer", "bearer", "BEARER", "BeArEr"] {
        let request = hub
            .anonymous("GET", "/v1/workspaces")
            .header("authorization", format!("{scheme} {}", hub.key))
            .body(Body::empty())
            .unwrap();
        let reply = hub.send(request).await;
        assert_ne!(reply.code(), 401, "{scheme}: {}", reply.body());
    }
}

/// `auth::credential` is the only place a header becomes a name, and it never returns the key.
#[tokio::test]
async fn credential_returns_the_name_and_never_the_key() {
    let hub = hub_with_env(&[]);
    let pair = hub.app.keys.current();
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", hub.key)
            .parse()
            .expect("the header value"),
    );
    let name = auth::credential(&pair, &headers).expect("the fixture's key is in the file");
    assert_eq!(name, KEY_NAME);
    assert!(
        !name.contains("gm_"),
        "a name never carries the key: {name}"
    );
}
