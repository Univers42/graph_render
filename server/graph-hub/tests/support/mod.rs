//! What the integration tests share: a hub state built the way the binary builds it, a captured
//! log, one-shot requests through `tower::ServiceExt::oneshot`, and a source grep.
//!
//! Task 1 builds the state with `App::new`; Task 2 replaces that with `App::from_settings` once
//! `config::Settings` exists, so the fixture is the one that grows, not a second fixture beside it.
#![allow(
    dead_code,
    unused_imports,
    reason = "each test binary uses a part of this module"
)]

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use graph_hub::app::{App, LogSink};
use http_body_util::BodyExt;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

pub mod motor;
pub use motor::*;

/// A hub under test.
pub struct Hub {
    /// The router under test.
    pub router: Router,
    /// The state behind it.
    pub app: Arc<App>,
    /// Every log line written so far.
    pub log: Arc<Mutex<Vec<String>>>,
    /// This test's own scratch directory.
    pub dir: PathBuf,
}

/// One answer.
pub struct Reply {
    /// The status line.
    pub status: StatusCode,
    /// The response head.
    pub headers: HeaderMap,
    /// The whole body.
    pub body: Bytes,
}

impl Reply {
    /// The status as a number, so an assertion reads `200` rather than a `StatusCode` name.
    pub fn code(&self) -> u16 {
        self.status.as_u16()
    }

    /// The body's `error` field.
    pub fn error(&self) -> String {
        let body: serde_json::Value =
            serde_json::from_slice(&self.body).expect("a JSON error body");
        body["error"].as_str().expect("an error name").to_owned()
    }

    /// The body's `message` field.
    pub fn message(&self) -> String {
        let body: serde_json::Value =
            serde_json::from_slice(&self.body).expect("a JSON error body");
        body["message"].as_str().expect("a message").to_owned()
    }

    /// The whole body as text, for Review Focus 1's byte-for-byte refusal comparisons.
    pub fn body(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// A header as text, `""` when absent.
    pub fn header(&self, name: &str) -> &str {
        let value = self.headers.get(name);
        value.and_then(|value| value.to_str().ok()).unwrap_or("")
    }
}

/// A fresh directory under cargo's test scratch root.
pub fn scratch() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = format!(
        "t{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    // The name is a pid and a counter, and a pid is reused: a leftover from an earlier run made
    // the fixture's own create fail with AlreadyExists. Start empty.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// A hub with `env` on top of the defaults.
pub fn hub_with_env(env: &[(&str, &str)]) -> Hub {
    let dir = scratch();
    let mut vars: BTreeMap<String, String> = BTreeMap::new();
    for (name, value) in env {
        vars.insert((*name).to_owned(), (*value).to_owned());
    }
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let sink: LogSink = Arc::new(move |line: &str| sink.lock().unwrap().push(line.to_owned()));
    let app = App::new(sink);
    Hub {
        router: graph_hub::router(Arc::clone(&app)),
        app,
        log,
        dir,
    }
}

/// The captured log of a hub, parsed. `vars` is unused until Task 2 reads the settings from it;
/// keeping it in the signature means Task 2 changes no call site.
pub fn env_of(env: &[(&str, &str)]) -> BTreeMap<String, String> {
    let mut vars: BTreeMap<String, String> = BTreeMap::new();
    for (name, value) in env {
        vars.insert((*name).to_owned(), (*value).to_owned());
    }
    vars
}

impl Hub {
    /// Sends `request` and reads the whole answer.
    pub async fn send(&self, request: Request<Body>) -> Reply {
        send_to(self.router.clone(), request).await
    }

    /// [`Hub::send`] on its own task.
    pub fn spawn(&self, request: Request<Body>) -> tokio::task::JoinHandle<Reply> {
        tokio::spawn(send_to(self.router.clone(), request))
    }

    /// `GET path`, with no credential: `/healthz` takes none.
    pub async fn get(&self, path: &str) -> Reply {
        self.send(
            Request::builder()
                .method("GET")
                .uri(path)
                .body(Body::empty())
                .expect("the test request"),
        )
        .await
    }

    /// `GET path` with the key.
    pub async fn get_with(&self, key: &str, path: &str) -> Reply {
        self.send(
            Request::builder()
                .method("GET")
                .uri(path)
                .header("authorization", format!("Bearer {key}"))
                .body(Body::empty())
                .expect("the test request"),
        )
        .await
    }

    /// The log lines so far, parsed.
    pub fn lines(&self) -> Vec<serde_json::Value> {
        let lines = self.log.lock().unwrap();
        lines
            .iter()
            .map(|line| serde_json::from_str(line).expect("a JSON line"))
            .collect()
    }
}

async fn send_to(router: Router, request: Request<Body>) -> Reply {
    let response = router.oneshot(request).await.expect("infallible");
    let (parts, body) = response.into_parts();
    let body = body.collect().await.expect("the response body").to_bytes();
    Reply {
        status: parts.status,
        headers: parts.headers,
        body,
    }
}

/// Every line of this crate's own source that holds `needle`.
///
/// Review Focus 5 pins a negative — `auth::check` appears nowhere in the hub — and only a source
/// grep can hold one: the code that must not be called compiles perfectly well whether or not
/// anyone calls it. Caveat: this greps `src/` and not `tests/`, so a test may name what it
/// forbids.
pub fn grep_this_crate(needle: &str) -> Vec<String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut hits = Vec::new();
    walk(&root, needle, &mut hits);
    // Sorted, so a failing assertion prints the same list on every run and in every order.
    hits.sort();
    hits
}

/// The grep's own recursion: one directory level at a time, since `src/` is two levels deep.
fn walk(dir: &Path, needle: &str, hits: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, needle, hits);
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (number, line) in text.lines().enumerate() {
            if line.contains(needle) {
                hits.push(format!("{}:{}", path.display(), number + 1));
            }
        }
    }
}
