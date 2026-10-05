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

pub mod db;
pub mod fixtures;
pub mod grep;
pub mod motor;
pub mod reply;
pub mod step;
pub mod wire;
pub use grep::grep_this_crate;
pub use motor::*;
pub use reply::Reply;

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
    vars.insert(
        "GRAPH_HUB_KEYS_FILE".into(),
        keys_file.display().to_string(),
    );
    vars.insert(
        "GRAPH_HUB_GRANTS_FILE".into(),
        grants_file.display().to_string(),
    );
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
    let app = App::from_settings(&settings, sink).expect("the test state");
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
