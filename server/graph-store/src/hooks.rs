//! The three test-only seams §5.1 and §5.3 need to force an interleaving that timing alone
//! would not produce.
//!
//! Without the `test-hooks` feature every method here is an empty `async` function, and the
//! optimizer removes the calls; row `hooks-gated-store` proves both halves by reading
//! `--print cfg`.

/// The seams. Empty in behaviour unless `test-hooks` is on.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hooks;

impl Hooks {
    /// A store with no seam active.
    pub const fn new() -> Hooks {
        Hooks
    }

    /// Pause after the workspace row lock (step 1) and before any write, so a second session can
    /// be made to deadlock against this one.
    pub async fn pause_after_lock(&self) {}

    /// Pause after the change header rows are written and before `COMMIT`, so a second session
    /// can prune between the header read and the operation read.
    pub async fn pause_before_commit(&self, _seq: u64) {}

    /// Pause after the change headers are read and before their operations, for the same reason.
    pub async fn pause_after_headers(&self, _seq: u64) {}
}

/// Wait for another session to write `<dir>/<name>.done`.
///
/// This is the file handshake `hub-pg.sh` needs: the test process and the container-level verbs
/// (`replica-promote`, `kill`, `copy-data`) cannot share memory, and `scripts/orch/gr` mounts
/// the repository read-write at `/w`, so a path under `target/` is the same directory on both
/// sides.
///
/// Caveat: the bound below is a guess. A peer that never signals makes this wait out its bound
/// and then continue, so a lost handshake reads as a slow case rather than a failed one.
pub async fn await_done(dir: &str, name: &str) {
    let path = format!("{dir}/{name}.done");
    for _ in 0..600 {
        if std::path::Path::new(&path).exists() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// Signal another session that this one has reached its point.
///
/// Caveat: the write is a create, not an append, so a signal for a name that already exists is
/// invisible to the waiter. Names are unique per case for that reason.
pub fn signal(dir: &str, name: &str) {
    let _ = std::fs::write(format!("{dir}/{name}.ready"), b"");
}