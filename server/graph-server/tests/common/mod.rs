//! What the integration tests share: a server state built the way the binary builds it, with a
//! key minted in the test (never committed), a captured log, and one-shot requests through
//! `tower::ServiceExt::oneshot`.
#![allow(
    dead_code,
    unused_imports,
    reason = "each test binary uses a part of this module"
)]

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use graph_server::app::{App, Hooks, LogSink};
use graph_server::config::Settings;
use graph_server::keys;
use http_body_util::BodyExt;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower::ServiceExt;

mod child;
mod documents;
pub use child::*;
pub use documents::*;

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

    /// The error body's `message` field.
    pub fn message(&self) -> String {
        let body: serde_json::Value =
            serde_json::from_slice(&self.body).expect("a JSON error body");
        body["message"].as_str().expect("a message").to_owned()
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
    // The default worker count follows the host's cgroup; a test names its own.
    vars.insert("GRAPH_WORKERS".into(), "2".into());
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
        send_to(self.router.clone(), request).await
    }

    /// [`Server::send`] on its own task.
    pub fn spawn(&self, request: Request<Body>) -> tokio::task::JoinHandle<Reply> {
        tokio::spawn(send_to(self.router.clone(), request))
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

    /// `POST /v1/layout?query` declaring `length` bytes, with a body that fails if read.
    pub async fn declared(&self, query: &str, length: usize) -> Reply {
        let uri = format!("/v1/layout?{query}");
        let request = self.request("POST", &uri).header("content-length", length);
        self.send(request.body(failing_body()).unwrap()).await
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

/// A hook that blocks every run while held, counting the runs that reached it.
#[derive(Clone, Default)]
pub struct Hold {
    pub held: Arc<AtomicBool>,
    pub reached: Arc<AtomicU64>,
}

impl Hold {
    /// A hold that starts held.
    pub fn new() -> Self {
        let hold = Self::default();
        hold.held.store(true, Ordering::SeqCst);
        hold
    }

    /// The hook for [`Hooks::before_run`]. It runs on the blocking thread, so it may sleep.
    pub fn hooks(&self) -> Hooks {
        let hold = self.clone();
        let hook = move || {
            hold.reached.fetch_add(1, Ordering::SeqCst);
            while hold.held.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        Hooks {
            before_run: Some(Arc::new(hook)),
        }
    }

    /// Lets every held run go on.
    pub fn release(&self) {
        self.held.store(false, Ordering::SeqCst);
    }

    /// A guard that releases on drop. A failed assertion unwinds past `release`, and the
    /// runtime's drop then waits forever on the blocking threads still held.
    pub fn release_on_drop(&self) -> Release {
        Release(Arc::clone(&self.held))
    }

    /// Waits until `count` runs reached the hook.
    pub async fn reached(&self, count: u64) {
        until(|| self.reached.load(Ordering::SeqCst) >= count).await;
    }
}

/// Releases a [`Hold`] when dropped.
pub struct Release(Arc<AtomicBool>);

impl Drop for Release {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Polls `done` every 5 ms. Caveat: a 10 s bound, so a hung condition fails the test instead
/// of hanging it; a host slower than that reads as a failure.
pub async fn until(done: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(
            std::time::Instant::now() < deadline,
            "the condition never held"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
