//! The file handshake between a test process and `scripts/orch/hub-pg.sh`.
//!
//! `scripts/orch/gr` bind-mounts the repository read-write at `/w`, so a path under `target/`
//! is the same directory on the host and in the test container. That is the whole channel: only
//! `hub-pg.sh` can kill a container or promote a standby, and it cannot reach into the test's
//! memory.
//!
//! Caveat: the handshake is a poll on a path. It cannot prove the peer saw the signal, only that
//! the file appeared, so a case that depends on ordering states it in its own assertions too.

/// The directory the handshake files live in, or `target/hub-steps`.
pub fn dir() -> String {
    std::env::var("GM_HUB_STEP_DIR").unwrap_or_else(|_| "target/hub-steps".to_string())
}

/// Signal the peer that this session has reached its point.
pub fn signal(name: &str) {
    let dir = dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(format!("{dir}/{name}.ready"), b"");
}

/// Wait for the peer's `<name>.done`, bounded.
///
/// Caveat: 600 polls of 100 ms is a guess above a container restart. A peer that never signals
/// makes this wait out its bound; the caller's own assertions are what decide the case.
pub async fn await_done(name: &str) {
    let path = format!("{}/{name}.done", dir());
    for _ in 0..600 {
        if std::path::Path::new(&path).exists() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// Write `lines` to `<dir>/<name>`, one per line, for the next case to read.
pub fn write(name: &str, lines: &[String]) {
    let dir = dir();
    let _ = std::fs::create_dir_all(&dir);
    let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let _ = std::fs::write(format!("{dir}/{name}"), body);
}
