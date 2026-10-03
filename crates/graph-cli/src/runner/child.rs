//! How a child process is *run*, as opposed to what is run: pipes drained on their own
//! thread so a full one cannot stall the child, a deadline that kills, and the one call that
//! hands back a child's raw stdout bytes.
//!
//! Split out of `runner.rs` by the house's 300-line cap, and because the three runners want
//! three different things from one child: `build_wasm` wants the exit status **and** the
//! bytes (a cargo build that failed still printed messages worth reading), `run_lines` wants
//! the bytes as text and treats a non-zero exit as an error, and `run_status` wants neither
//! the bytes nor a pipe. Three wrappers over one spawn is the shape; this is the spawn.

use super::{Command, Duration, ExitStatus};
use std::io::Read;
use std::process::{Child, Stdio};
use std::time::Instant;

/// Runs `command` bounded by `limit`, and returns its exit status together with its **stdout
/// bytes**.
///
/// The one runner that hands a *failed* status back rather than an `Err`, because the caller
/// reads the bytes either way: `build_wasm` needs cargo's JSON messages out of a build that
/// may well have failed, and decides for itself whether the failure matters.
/// [`super::run_lines`] makes the opposite choice for the same child — a non-zero exit is an
/// error there, because its caller wants lines and not a status.
///
/// **stderr is inherited, not piped.** `Ponytail: what this gets wrong — a failing cargo
/// build's diagnostics go to the terminal instead of into the caller's message, so a caller
/// that discards this `Err` reports the status and nothing else. Failing input: a non-zero
/// exit from `build_wasm`, whose refusal then reads "building graph-wasm for wasm32 failed:
/// exit status: 101" with cargo's own complaint printed above it. Direction: strictly more
/// information than a captured-and-discarded pipe, never less, and no second pipe to stall.
/// Escape hatch: re-run the same build by hand, which is what the terminal output invites.
pub(super) fn run_captured(
    command: &mut Command,
    limit: Duration,
) -> Result<(ExitStatus, Vec<u8>), String> {
    command.stdout(Stdio::piped()).stderr(Stdio::inherit());
    let mut child = command
        .spawn()
        .map_err(|e| format!("spawning {command:?}: {e}"))?;
    let stdout = drain(child.stdout.take());
    let status = wait_within(&mut child, limit).map_err(|e| format!("{command:?}: {e}"))?;
    Ok((status, joined(stdout)?))
}

/// Waits for `child`, killing it once `limit` has passed.
pub(super) fn wait_within(child: &mut Child, limit: Duration) -> Result<ExitStatus, String> {
    let deadline = Instant::now() + limit;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| format!("waiting: {e}"))? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "killed after {}ms without exiting",
                limit.as_millis()
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(super) type Drain = Option<std::thread::JoinHandle<std::io::Result<Vec<u8>>>>;

/// Reads a child's pipe to the end on its own thread, so a full pipe cannot stall the child.
pub(super) fn drain(pipe: Option<impl Read + Send + 'static>) -> Drain {
    pipe.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).map(|_| bytes)
        })
    })
}

pub(super) fn joined(drain: Drain) -> Result<Vec<u8>, String> {
    match drain {
        None => Ok(Vec::new()),
        Some(handle) => handle
            .join()
            .map_err(|_| "a pipe reader panicked".to_owned())?
            .map_err(|e| format!("reading a child's output: {e}")),
    }
}

#[cfg(test)]
#[path = "child/tests.rs"]
mod tests;
