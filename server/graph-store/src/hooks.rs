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

    /// Pause between the restore detector's flush-LSN read and its high-water snapshot.
    ///
    /// This is the seam S2 needs. With the snapshot taken FIRST (the correct order) a commit that
    /// lands after the snapshot cannot make the run look like a restore, because the high-water the
    /// run compares against predates the write. `hw-after-lsn` reverses the order, and the only
    /// way to make that reversal observable without a race is to hold the run open in the gap and
    /// let a writer commit into it.
    ///
    /// Caveat: the pause is bounded by [`crate::hooks::await_done`]-style polling, so a test that
    /// signals nothing makes this wait out its bound and continue — the run then succeeds and the
    /// control looks green for the wrong reason. Every caller signals within the bound.
    pub async fn pause_before_snapshot(&self) {
        #[cfg(feature = "test-hooks")]
        wait_for("detector-before-snapshot").await;
    }
}

/// Announce that this run has reached `<name>` and wait for the peer's `<name>.done`.
///
/// The two halves are what make the seam usable: the peer cannot commit into a gap it does not know
/// has opened, and the run cannot proceed until the commit it is waiting for has happened.
///
/// Caveat: 20 polls of 100 ms is TWO seconds, and it is deliberately short. The seam sits on the
/// restore detector's per-connection path, so under a break EVERY connection pays this wait — at the
/// library's 600-poll bound a detector suite took eight minutes. Two seconds is far longer than the
/// peer needs (a peer only has to notice a file that already exists) and it keeps the suite fast. A
/// peer that never signals makes this wait out its bound and continue, so a forgotten signal reads
/// as a slow case rather than a failed one — which is why the one test that drives this seam
/// announces itself and asserts the outcome, rather than trusting the seam to block.
#[cfg(feature = "test-hooks")]
async fn wait_for(name: &str) {
    let dir = std::env::var("GM_HUB_STEP_DIR").unwrap_or_else(|_| "target/hub-steps".to_string());
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(format!("{dir}/{name}.ready"), b"");
    let done = format!("{dir}/{name}.done");
    for _ in 0..20 {
        if std::path::Path::new(&done).exists() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
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
