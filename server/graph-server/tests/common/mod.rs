//! What the integration tests share: a server state built the way the binary builds it, with a
//! key minted in the test (never committed), a captured log, and one-shot requests through
//! `tower::ServiceExt::oneshot`.
#![allow(dead_code, reason = "each test binary uses a part of this module")]

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use graph_server::app::{App, Hooks, LogSink};
use graph_server::config::Settings;
use graph_server::keys;
use http_body_util::BodyExt;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

/// A server under test.
pub struct Server {
    pub router: Router,
    pub app: Arc<App>,
    /// The minted key, for the `Authorization` header.
    pub key: String,
    /// Every log line written so far.
    pub log: Arc<Mutex<Vec<String>>>,
    /// The test's own scratch directory.
    pub dir: PathBuf,
}

/// One answer.
pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

impl Reply {
    /// The error body's `error` field.
    pub fn code(&self) -> String {
        let body: serde_json::Value =
            serde_json::from_slice(&self.body).expect("a JSON error body");
        body["error"].as_str().expect("an error name").to_owned()
    }

    /// A header as text, `""` when absent.
    pub fn header(&self, name: &str) -> &str {
        let value = self.headers.get(name);
        value.and_then(|v| v.to_str().ok()).unwrap_or("")
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
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// A server with a minted key and `env` on top of the defaults.
pub fn server(env: &[(&str, &str)]) -> Server {
    server_with(env, Hooks::default())
}

/// [`server`] with test hooks.
pub fn server_with(env: &[(&str, &str)], hooks: Hooks) -> Server {
    let dir = scratch();
    let minted = keys::keygen("tester").expect("a key from /dev/urandom");
    let keys_file = dir.join("keys");
    std::fs::write(&keys_file, format!("{}\n", minted.line)).expect("the key file");
    let mut vars: BTreeMap<String, String> = BTreeMap::new();
    vars.insert(
        "GRAPH_API_KEYS_FILE".into(),
        keys_file.display().to_string(),
    );
    for (name, value) in env {
        vars.insert((*name).to_owned(), (*value).to_owned());
    }
    let lookup = |name: &str| vars.get(name).map(Into::into);
    let settings = Settings::from_env(&lookup).expect("the test settings");
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    let sink: LogSink = Arc::new(move |line: &str| sink.lock().unwrap().push(line.to_owned()));
    let mut app = App::from_settings(&settings, sink).expect("the test state");
    app.hooks = hooks;
    let app = Arc::new(app);
    let router = graph_server::router(Arc::clone(&app));
    Server {
        router,
        app,
        key: minted.key,
        log,
        dir,
    }
}

impl Server {
    /// Sends `request` and reads the whole answer.
    pub async fn send(&self, request: Request<Body>) -> Reply {
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("infallible");
        let (parts, body) = response.into_parts();
        let body = body.collect().await.expect("the response body").to_bytes();
        Reply {
            status: parts.status,
            headers: parts.headers,
            body,
        }
    }

    /// `GET path` with the key.
    pub async fn get(&self, path: &str) -> Reply {
        self.send(self.request("GET", path).body(Body::empty()).unwrap())
            .await
    }

    /// `POST /v1/layout?query` with the key and `doc`.
    pub async fn layout(&self, query: &str, doc: impl Into<Body>) -> Reply {
        let uri = format!("/v1/layout?{query}");
        self.send(self.request("POST", &uri).body(doc.into()).unwrap())
            .await
    }

    /// A request builder carrying the key.
    pub fn request(&self, method: &str, uri: &str) -> axum::http::request::Builder {
        let bearer = format!("Bearer {}", self.key);
        Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", bearer)
    }

    /// The log lines so far, parsed.
    pub fn lines(&self) -> Vec<serde_json::Value> {
        let lines = self.log.lock().unwrap();
        lines
            .iter()
            .map(|l| serde_json::from_str(l).expect("a JSON line"))
            .collect()
    }
}

/// A studio ingest document of `n` nodes and `m` edges, no loop and no repeated pair while
/// `m <= n * (n - 1) / 2` in the first `n / 2` offsets.
pub fn doc(n: usize, m: usize) -> String {
    let node = |i: usize| {
        format!(
            r#"{{"id":"n{i}","kind":"record","database_id":null,"source":"t","label":"N{i}","group":null,"weight":1.0,"version":0.0,"has_note":false,"icon":null}}"#
        )
    };
    let edge = |e: usize| {
        let (from, offset) = (e % n, 1 + e / n);
        let to = (from + offset) % n;
        format!(
            r#"{{"id":"e{e}","source":"n{from}","target":"n{to}","kind":"relation","label":"","strength":0.5,"directed":false,"record_id":null,"child_first":false}}"#
        )
    };
    let nodes: Vec<String> = (0..n).map(node).collect();
    let edges: Vec<String> = (0..m).map(edge).collect();
    format!(
        r#"{{"version":1,"nodes":[{}],"edges":[{}]}}"#,
        nodes.join(","),
        edges.join(",")
    )
}

/// A committed fixture, by its path under the repository root.
pub fn fixture(path: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join(path)).unwrap_or_else(|_| panic!("fixture {path}"))
}
