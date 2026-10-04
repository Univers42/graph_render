//! `/embed/<h>/*` (Verdict condition 10), row `svc-embed`: public, versioned, immutable, a JS
//! MIME and nosniff; every path that is not a file of the tree is a 404, a symlink escape
//! included (the row's negative control follows symlinks).

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{Server, scratch, server};
use std::path::Path;

const VERSION: &str = "0123456789abcdef";

/// A tree with a file, a nested wasm, a dotfile, and two symlinks out of the tree.
fn embed_server() -> Server {
    let dir = scratch();
    let root = dir.join(VERSION);
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::create_dir_all(dir.join("outside")).unwrap();
    std::fs::write(dir.join("VERSION"), format!("{VERSION}\n")).unwrap();
    std::fs::write(root.join("graph-studio.js"), "export {};\n").unwrap();
    std::fs::write(root.join("sub/graph_wasm.wasm"), b"\0asm\x01\0\0\0").unwrap();
    std::fs::write(root.join(".env.js"), "secret").unwrap();
    std::fs::write(dir.join("outside/secret.js"), "secret").unwrap();
    std::fs::write(dir.join("outside.js"), "secret").unwrap();
    let escape = |target: &str, link: &str| {
        std::os::unix::fs::symlink(dir.join(target), root.join(link)).unwrap();
    };
    escape("outside.js", "escape.js");
    escape("outside", "linked");
    server(&[("GRAPH_EMBED_DIR", &dir.display().to_string())])
}

/// A `GET` with no key: the embed tree is public.
async fn fetch(server: &Server, path: &str) -> common::Reply {
    let request = Request::builder().uri(path).body(Body::empty()).unwrap();
    server.send(request).await
}

#[tokio::test]
async fn a_file_is_served_public_and_immutable() {
    let server = embed_server();
    let reply = fetch(&server, &format!("/embed/{VERSION}/graph-studio.js")).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(&reply.body[..], b"export {};\n");
    let expected = [
        ("content-type", "text/javascript"),
        ("x-content-type-options", "nosniff"),
        ("cache-control", "public, max-age=31536000, immutable"),
        ("cross-origin-resource-policy", "cross-origin"),
    ];
    for (name, value) in expected {
        assert_eq!(reply.header(name), value, "{name}");
    }
    let reply = fetch(&server, &format!("/embed/{VERSION}/sub/graph_wasm.wasm")).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.header("content-type"), "application/wasm");
}

#[tokio::test]
async fn every_path_outside_the_files_is_404() {
    let server = embed_server();
    let paths = [
        "/embed/graph-studio.js".to_owned(),
        "/embed/VERSION".to_owned(),
        "/embed/fedcba9876543210/graph-studio.js".to_owned(),
        format!("/embed/{VERSION}"),
        format!("/embed/{VERSION}/"),
        format!("/embed/{VERSION}/sub"),
        format!("/embed/{VERSION}/sub/"),
        format!("/embed/{VERSION}/.env.js"),
        format!("/embed/{VERSION}/../outside.js"),
        format!("/embed/{VERSION}/%2e%2e/outside.js"),
        format!("/embed/{VERSION}/sub/..%2f..%2foutside.js"),
        format!("/embed/{VERSION}/sub%2f..%2fgraph-studio.js"),
        format!("/embed/{VERSION}/escape.js"),
        format!("/embed/{VERSION}/linked/secret.js"),
    ];
    for path in &paths {
        let reply = fetch(&server, path).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(reply.code(), "NotFound", "{path}");
    }
}

#[tokio::test]
async fn without_an_embed_dir_every_embed_path_is_404() {
    let server = server(&[]);
    let reply = fetch(&server, &format!("/embed/{VERSION}/graph-studio.js")).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}

#[test]
fn a_malformed_version_refuses_the_start() {
    for version in ["0123456789ABCDEF\n", "0123456789abcdef", "0123\n", ""] {
        let dir = scratch();
        std::fs::create_dir_all(dir.join(VERSION)).unwrap();
        std::fs::write(dir.join("VERSION"), version).unwrap();
        assert!(starts(&dir).is_err(), "{version:?}");
    }
    let dir = scratch();
    std::fs::write(dir.join("VERSION"), format!("{VERSION}\n")).unwrap();
    assert!(starts(&dir).is_err(), "VERSION names a missing directory");
}

fn starts(embed_dir: &Path) -> Result<(), String> {
    let dir = embed_dir.display().to_string();
    let lookup = |name: &str| match name {
        "GRAPH_AUTH" => Some("off".into()),
        "GRAPH_EMBED_DIR" => Some(dir.clone().into()),
        _ => None,
    };
    let settings = graph_server::config::Settings::from_env(&lookup).map_err(|e| e.to_string())?;
    let sink: graph_server::app::LogSink = std::sync::Arc::new(|_: &str| {});
    graph_server::app::App::from_settings(&settings, sink).map(drop)
}
