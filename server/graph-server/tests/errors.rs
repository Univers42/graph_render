//! The query, `Accept` and the motor's refusals (Verdict condition 6): one POST pass, every
//! error body named, and the face chosen by `Accept`.

mod common;

use axum::body::Body;
use axum::http::StatusCode;
use common::{Reply, Server, doc, server};

async fn with_accept(server: &Server, accept: &str) -> Reply {
    let request = server.request("POST", "/v1/layout?layout=layout.grid");
    let request = request.header("accept", accept).body(Body::from(doc(5, 4)));
    server.send(request.unwrap()).await
}

fn named(reply: &Reply) -> (StatusCode, String) {
    (reply.status, reply.code())
}

#[tokio::test]
async fn query_refusals_are_400_with_their_names() {
    let server = server(&[]);
    for (query, code) in [
        ("layout=layout.nope", "UnknownLayoutId"),
        ("layout=layout.grid&post=post.nope", "IndexOutOfRange"),
        (
            "layout=layout.grid&post=post.style.bezier,post.style.straight",
            "BadRequest",
        ),
        ("layout=layout.grid&source=csv", "BadRequest"),
        ("layout=layout.grid&format=json", "BadRequest"),
        ("layout=layout.grid&layout=layout.grid", "BadRequest"),
        ("post=post.style.bezier", "BadRequest"),
        ("", "BadRequest"),
    ] {
        let reply = server.layout(query, doc(3, 2)).await;
        assert_eq!(
            named(&reply),
            (StatusCode::BAD_REQUEST, code.to_owned()),
            "{query}"
        );
    }
}

#[tokio::test]
async fn an_unknown_post_wins_over_a_failing_layout() {
    // tree.tidy is the one layout that refuses a graph this small (an empty one, probed
    // across every id at n <= 4).
    // The motor checks both ids before running (graph_wasm::service::run), and the service
    // checks both before reading the body, so the post's refusal is the one answered.
    let server = server(&[]);
    let reply = server.layout("layout=layout.tree.tidy", doc(0, 0)).await;
    assert_eq!(
        named(&reply),
        (StatusCode::UNPROCESSABLE_ENTITY, "LayoutFailed".into())
    );
    let reply = server
        .layout("layout=layout.tree.tidy&post=post.nope", doc(0, 0))
        .await;
    assert_eq!(
        named(&reply),
        (StatusCode::BAD_REQUEST, "IndexOutOfRange".into())
    );
}

#[tokio::test]
async fn a_bad_document_is_422_with_the_motor_name() {
    let server = server(&[]);
    let reply = server.layout("layout=layout.grid", "{\"version\":1}").await;
    assert_eq!(
        named(&reply),
        (StatusCode::UNPROCESSABLE_ENTITY, "IngestInvalid".into())
    );
    let reply = server
        .layout("layout=layout.grid&source=contract", doc(3, 2))
        .await;
    assert_eq!(
        named(&reply),
        (StatusCode::UNPROCESSABLE_ENTITY, "ContractInvalid".into())
    );
}

#[tokio::test]
async fn accept_picks_the_face() {
    let server = server(&[]);
    let binary = "application/vnd.graph-motor.snapshot";
    for (accept, face) in [
        ("*/*", binary),
        ("application/*", binary),
        (binary, binary),
        ("application/json", "application/json"),
        ("application/json, */*;q=0.5", "application/json"),
        ("application/json;q=0, */*", binary),
        (
            "application/vnd.graph-motor.snapshot;q=0, application/json;q=0.1",
            "application/json",
        ),
        ("text/html, application/json;q=0.9", "application/json"),
    ] {
        let reply = with_accept(&server, accept).await;
        assert_eq!(reply.status, StatusCode::OK, "{accept}");
        assert_eq!(reply.header("content-type"), face, "{accept}");
    }
}

#[tokio::test]
async fn an_accept_no_face_satisfies_is_406() {
    let server = server(&[]);
    for accept in [
        "text/html",
        "*/*;q=0",
        "application/json;q=0, application/vnd.graph-motor.snapshot;q=0",
        "application/json;q=2",
    ] {
        let reply = with_accept(&server, accept).await;
        assert_eq!(
            named(&reply),
            (StatusCode::NOT_ACCEPTABLE, "NotAcceptable".into()),
            "{accept}"
        );
    }
}

#[tokio::test]
async fn every_error_name_is_in_the_contract() {
    let contract = String::from_utf8(common::fixture("docs/contract/service-api.md")).unwrap();
    for name in [
        "BadRequest",
        "Unauthorized",
        "NotFound",
        "NotAcceptable",
        "IngestTooLarge",
        "Busy",
        "Timeout",
        "Internal",
        "UnknownLayoutId",
        "IndexOutOfRange",
        "IngestInvalid",
        "ContractInvalid",
        "LayoutFailed",
        "PostFailed",
    ] {
        assert!(
            contract.contains(&format!("`{name}`")),
            "{name} is not in service-api.md"
        );
    }
}
