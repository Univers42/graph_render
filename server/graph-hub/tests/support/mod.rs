//! What the hub's integration tests share: a hub state built the way the binary builds it, the
//! credential files it reads, one-shot requests through `tower::ServiceExt::oneshot`, and a source
//! grep.
//!
//! Every fixture mints its own keys from `/dev/urandom` and never reads a committed one, and both
//! credential files are written at mode 0640 because `KeySet::load` (`server/graph-server/src/keys.rs:74`)
//! and `Grants::load` refuse anything wider (§5.2).
#![allow(
    dead_code,
    unused_imports,
    reason = "each test binary uses a part of this module"
)]

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use graph_hub::app::{App, LogSink};
use graph_server::keys::NewKey;
use http_body_util::BodyExt;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

pub mod motor;
pub use motor::*;

/// The name every fixture's first key carries, in both credential files and in the header.
pub const KEY_NAME: &str = "tester";

/// A hub under test, and the two files its credentials came from.
pub struct Hub {
    /// The router under test.
    pub router: Router,
    /// The state behind it.
    pub app: Arc<App>,
    /// The minted key of [`KEY_NAME`], for the `Authorization` header.
    pub key: String,
    /// Every log line written so far.
    pub log: Arc<Mutex<Vec<String>>>,
    /// This test's own scratch directory.
    pub dir: PathBuf,
    /// The key file, so a `SIGHUP` test can rewrite it.
    pub keys_file: PathBuf,
    /// The grants file, so a `SIGHUP` test can rewrite it.
    pub grants_file: PathBuf,
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
        self field("error")
    }

    /// The body's `message` field.
    pub fn message(&self) -> String {
        self field("message")
    }

    /// One field of a JSON body.
    fn field(&self, name: &str) -> String {
        let body: serde_json::Value =
            serde_json::from_slice(&self.body).expect("a JSON body");
        body[name].as_str().expect("a JSON string field").to_owned()
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
    // The name is a pid and a counter, and a pid is reused: a leftover from an earlier run made the
    // fixture's own create fail with AlreadyExists. Start empty.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// A hub whose one key is granted `admin` on every workspace, and `env` on top of the defaults.
pub fn hub_with_env(env: &[(&str, &str)]) -> Hub {
    hub_with_grants(&format!("{KEY_NAME} * admin\n"), env)
}

/// A hub whose grants file is exactly `grants`, and `env` on top of the defaults.
pub fn hub_with_grants(grants: &str, env: &[(&str, &str)]) -> Hub {
    let dir = scratch();
    let minted = mint(KEY_NAME);
    let keys_file = write_private(&dir.join("keys"), &format!("{}\n", minted.line));
    let grants_file = write_private(&dir.join("grants"), grants);
    let mut vars: BTreeMap<String, String> = BTreeMap::new();
    vars.insert("GRAPH_HUB_KEYS_FILE".into(), keys_file.display().to_string());
    vars.insert("GRAPH_HUB_GRANTS_FILE".into(), grants_file.display().to_string());
    for (name, value) in env {
        vars.insert((*name).to_owned(), (*value).to_owned());
    }
    hub_over(&vars, dir, minted.key, keys_file, grants_file)
}

/// A second key, minted and appended to the key file, for the tests that need two identities.
///
/// The file is rewritten at 0640 after the append: `std::fs::write` follows the umask, so an
/// append that widened the mode would make the *next* load refuse the file.
pub fn add_key(hub: &Hub, name: &str) -> String {
    let minted = mint(name);
    let mut text = std::fs::read_to_string(&hub.keys_file).expect("the fixture's key file");
    text.push_str(&format!("{}\n", minted.line));
    write_private(&hub.keys_file, &text);
    minted.key
}

/// Rewrite the grants file at 0640, as a `SIGHUP` test does between the reloads.
pub fn write_grants(hub: &Hub, text: &str) {
    write_private(&hub.grants_file, text);
}

/// Rewrite the key file at 0640, as a `SIGHUP` test does between the reloads.
pub fn write_keys(hub: &Hub, text: &str) {
    write_private(&hub.keys_file, text);
}

/// The hub over an assembled variable map, which is the shape every other fixture ends at.
fn hub_over(
    vars: &BTreeMap<String, String>,
    dir: PathBuf,
    key: String,
    keys_file: PathBuf,
    grants_file: PathBuf,
) -> Hub {
    let lookup = |name: &str| vars.get(name).map(Into::into);
    let settings = graph_hub::config::Settings::from_env(&lookup).expect("the test settings");
    let (log, sink) = log_sink();
    let app = Arc::new(App::from_settings(&settings, sink).expect("the test state"));
    Hub {
        router: graph_hub::router(Arc::clone(&app)),
        app,
        key,
        log,
        dir,
        keys_file,
        grants_file,
    }
}

/// One key from `/dev/urandom`. A fixture panic here is the right shape: a test that cannot mint a
/// key has nothing to say about authorization.
fn mint(name: &str) -> NewKey {
    graph_server::keys::keygen(name).expect("a key from /dev/urandom")
}

/// Write `text` to `path` at mode 0640 and return the path.
pub fn write_private(path: &Path, text: &str) -> PathBuf {
    std::fs::write(path, text).expect("the fixture's file");
    let readable = std::fs::Permissions::from_mode(0o640);
    std::fs::set_permissions(path, readable).expect("the fixture's file mode");
    path.to_path_buf()
}

/// A captured log sink, one buffer per hub.
pub fn log_sink() -> (Arc<Mutex<Vec<String>>>, LogSink) {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&lines);
    let sink: LogSink = Arc::new(move |line: &str| sink.lock().unwrap().push(line.to_owned()));
    (lines, sink)
}

impl Hub {
    /// Sends `request` and reads the whole answer.
    pub async fn send(&self, request: Request<Body>) -> Reply {
        send_to(self.router.clone(), request).await
    }

    /// [`Hub::send`] on its own task, so a test can hold one request open while another arrives.
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

    /// `GET path` with this fixture's key.
    pub async fn get_with(&self, path: &str) -> Reply {
        self.send(self.request("GET", path).body(Body::empty()).expect("the request"))
            .await
    }

    /// `GET path` with `key`, which may be a key nobody minted or a name nobody holds.
    pub async fn get_as(&self, key: &str, path: &str) -> Reply {
        self.send(self.request_as(key, "GET", path).body(Body::empty()).expect("the request"))
            .await
    }

    /// `POST path` with this fixture's key and `body`.
    pub async fn post(&self, path: &str, body: impl Into<Body>) -> Reply {
        self.send(self.request("POST", path).body(body.into()).expect("the request"))
            .await
    }

    /// `PUT path` with this fixture's key and `body`.
    pub async fn put(&self, path: &str, body: impl Into<Body>) -> Reply {
        self.send(self.request("PUT", path).body(body.into()).expect("the request"))
            .await
    }

    /// `GET path` with no credential, as `GET path` with a header this fixture did not set.
    pub async fn get_anonymous(&self, path: &str) -> Reply {
        self.get(path).await
    }

    /// A request builder carrying this fixture's key. No body: the caller adds one when it has one.
    pub fn request(&self, method: &str, uri: &str) -> axum::http::request::Builder {
        self.request_as(&self.key, method, uri)
    }

    /// A request builder carrying `key` under `Authorization: Bearer`.
    pub fn request_as(&self, key: &str, method: &str, uri: &str) -> axum::http::request::Builder {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {key}"))
    }

    /// A request builder carrying no credential at all.
    pub fn anonymous(&self, method: &str, uri: &str) -> axum::http::request::Builder {
        Request::builder().method(method).uri(uri)
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
/// Review Focus 5 pins a negative — the compute crate's credential check appears nowhere in the hub
/// — and only a source grep can hold one: code that must not be called compiles perfectly well
/// whether or not anyone calls it.
///
/// Caveat: this greps `src/` and not `tests/`, so a test may name what it forbids.
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