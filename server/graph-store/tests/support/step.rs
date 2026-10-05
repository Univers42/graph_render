//! The file handshake between a test process and `scripts/orch/hub-pg.sh`.
//!
//! `scripts/orch/gr` bind-mounts the repository read-write at `/w`, so a path under `target/`
//! is the same directory on the host and in the test container. That is the whole channel: only
//! `hub-pg.sh` can kill a container or promote a standby, and it cannot reach into the test's
//! memory. So a container-level case is not one test that polls: the row sequences it as
//! `hub-pg.sh <verb> && run --test <case> -- --ignored --exact <phase>`, and the state the phases
//! share travels as one file per case.
//!
//! Caveat: a phase that runs before the one that wrote the file sees nothing and says so by
//! panicking on the missing field. That is deliberate — a skipped phase is not a pass, and a
//! missing file is the shape a re-ordered row takes.

/// The directory the phase-state files live in, or the git top level's `target/hub-steps`.
///
/// WHY the default is anchored at the manifest directory rather than left relative: cargo runs a
/// test binary with its CWD at the *package* root, so a bare `target/hub-steps` resolves to
/// `server/graph-store/target/hub-steps`, which the test container has to create as root. The rows
/// pass `GM_HUB_STEP_DIR` themselves and both sides then name the same directory.
pub fn dir() -> String {
    std::env::var("GM_HUB_STEP_DIR")
        .unwrap_or_else(|_| format!("{}/../../target/hub-steps", env!("CARGO_MANIFEST_DIR")))
}

/// Read `<dir>/<case>.json` as its lines, trimmed, with the blank ones dropped.
///
/// An absent file reads as no lines rather than as an error: the caller decides which fields it
/// cannot do without, and `support::case::get` names the one that is missing.
pub fn read(case: &str) -> Vec<String> {
    let path = format!("{}/{case}.json", dir());
    match std::fs::read_to_string(&path) {
        Ok(text) => text
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Write `lines` to `<dir>/<case>.json`, one per line, for the next phase to read.
pub fn write(case: &str, lines: &[String]) {
    let dir = dir();
    let _ = std::fs::create_dir_all(&dir);
    let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let _ = std::fs::write(format!("{dir}/{case}.json"), body);
}
