//! Running the other programs a gate needs — cargo, node, the harness — and hashing
//! what they produce. Shared by the hash gate and the determinism probe.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// How long any one child (cargo, node, a gate arm) may run before it is killed. A hung
/// child is a gate that could not run (exit 2), never one that waits forever.
///
/// Phase 3 deviation: raised from 900s to 2700s. `layout.packing.circle`'s non-planar
/// fallback is O(n^2) per seed (`docs/decisions/planarity-fallback.md`), and a random
/// synthetic graph at gate density is essentially always non-planar, so `hashgate-arm
/// --seeds 1000` now legitimately needs close to 1800s to run all six stages honestly —
/// observed directly (`roundtrip --seeds 1000`, the same per-seed work, took ~1800s on
/// this host). 900s was sized for the two-stage (topology, grid) gate; this is not a
/// weakened check, only enough wall clock for the same check to finish saying so.
///
/// Ponytail: the limit is a guess about how long an honest run takes, so it is a guess
/// about the host as much as about the work. Failing input: a hashgate or roundtrip child
/// still running at 2700s — `--seeds 1000` on a loaded or shared host, say. Direction: a
/// correct but slow run is reported as a failure ("killed after 2700000ms without
/// exiting", exit 2), which reads as a broken gate rather than a slow one. Escape hatch:
/// re-run on an idle host; fewer seeds is for exploration only, never for a gate row,
/// whose seed count is the row's own claim. Phase 9 removes the O(n^2) fallback this
/// budget exists for, and the limit goes back to what the two-stage gate needs.
pub const CHILD_TIMEOUT: Duration = Duration::from_secs(2700);

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn file_sha256(path: &Path) -> Result<String, String> {
    std::fs::read(path)
        .map(|b| sha256_hex(&b))
        .map_err(|e| format!("reading {}: {e}", path.display()))
}

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Where a harness copy is staged for a negative control, named `name`.
///
/// `target/`, which is a *direct* child of the workspace root, so the copy's own
/// `../src` and `../crates` imports — and its `resolve(import.meta.dirname, "..")` root —
/// still land on the real TypeScript oracle and SDK exactly as the original's do, and
/// which is outside every entry of [`crate::fingerprint::FINGERPRINTED`], so writing
/// there cannot move the tree fingerprint. Staging beside the original under `harness/`
/// did resolve, and did move it: the copy is a file the listing names for as long as it
/// exists, so `evidence::tests` — in the same test binary, running in parallel — read a
/// tree the binary was not built from and failed, while passing when run alone. One
/// level deeper (`target/harness/`) does not move the fingerprint but breaks every
/// import, which is a negative control passing vacuously on a module-not-found.
///
/// Only the negative controls stage a copy, and they are tests, so this is test-only: no
/// production path needs a staging location that is deliberately outside the tree.
#[cfg(test)]
pub fn harness_mutant(name: &str) -> PathBuf {
    workspace_root()
        .join("target")
        .join(name.replace(".mjs", ".mutant.mjs"))
}

/// Builds `graph_wasm.wasm` in release mode with `features` and returns its path.
///
/// The target directory is passed to cargo, not guessed after the fact: the path this
/// returns is the one cargo just wrote, so a stale artifact elsewhere cannot be hashed.
pub fn build_wasm(features: &[&str]) -> Result<PathBuf, String> {
    let root = workspace_root();
    let target =
        std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from);
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .current_dir(&root)
        .args(["build", "--quiet", "--release"]);
    command.args(["-p", "graph-wasm", "--target", "wasm32-unknown-unknown"]);
    command.arg("--target-dir").arg(&target);
    if !features.is_empty() {
        command.args(["--features", &features.join(",")]);
    }
    let status = run_status(&mut command, CHILD_TIMEOUT)?;
    if !status.success() {
        return Err(format!("building graph-wasm for wasm32 failed: {status}"));
    }
    Ok(target
        .join("wasm32-unknown-unknown")
        .join("release")
        .join("graph_wasm.wasm"))
}

/// `node harness/wasm-run.mjs <wasm>`, ready for the mode arguments.
pub fn node_harness(wasm: &Path) -> Command {
    let mut command = Command::new("node");
    command
        .arg(workspace_root().join("harness").join("wasm-run.mjs"))
        .arg(wasm);
    command
}

/// Runs `command` to completion and returns its stdout lines, or why it failed.
pub fn run_lines(command: &mut Command) -> Result<Vec<String>, String> {
    run_lines_within(command, CHILD_TIMEOUT)
}

/// [`run_lines`] with an explicit time limit; a child still running at `limit` is killed.
pub fn run_lines_within(command: &mut Command, limit: Duration) -> Result<Vec<String>, String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| format!("spawning {command:?}: {e}"))?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let status = wait_within(&mut child, limit).map_err(|e| format!("{command:?}: {e}"))?;
    let (stdout, stderr) = (joined(stdout)?, joined(stderr)?);
    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr);
        return Err(format!("{command:?} exited {status}: {}", stderr.trim()));
    }
    let stdout = String::from_utf8(stdout).map_err(|e| format!("non-UTF-8 output: {e}"))?;
    Ok(stdout.lines().map(str::to_owned).collect())
}

/// Runs `command` with inherited output and returns its exit status, within `limit`.
pub fn run_status(command: &mut Command, limit: Duration) -> Result<ExitStatus, String> {
    let mut child = command
        .spawn()
        .map_err(|e| format!("spawning {command:?}: {e}"))?;
    wait_within(&mut child, limit).map_err(|e| format!("{command:?}: {e}"))
}

/// Waits for `child`, killing it once `limit` has passed.
fn wait_within(child: &mut Child, limit: Duration) -> Result<ExitStatus, String> {
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

type Drain = Option<std::thread::JoinHandle<std::io::Result<Vec<u8>>>>;

/// Reads a child's pipe to the end on its own thread, so a full pipe cannot stall the child.
fn drain(pipe: Option<impl Read + Send + 'static>) -> Drain {
    pipe.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).map(|_| bytes)
        })
    })
}

fn joined(drain: Drain) -> Result<Vec<u8>, String> {
    match drain {
        None => Ok(Vec::new()),
        Some(handle) => handle
            .join()
            .map_err(|_| "a pipe reader panicked".to_owned())?
            .map_err(|e| format!("reading a child's output: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_the_fips_180_2_vector() {
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(sha256_hex(b"abc"), abc);
    }

    #[test]
    fn run_lines_returns_stdout_lines_and_refuses_a_failing_command() {
        let mut ok = Command::new("sh");
        ok.args(["-c", "printf 'a\\nb\\n'"]);
        assert_eq!(run_lines(&mut ok), Ok(vec!["a".to_owned(), "b".to_owned()]));
        let mut failing = Command::new("sh");
        failing.args(["-c", "echo why >&2; exit 3"]);
        let err = run_lines(&mut failing).expect_err("exit 3 is a failure");
        assert!(err.contains("why"), "{err}");
    }

    #[test]
    fn a_child_past_its_time_limit_is_killed_and_reported() {
        let mut slow = Command::new("sh");
        slow.args(["-c", "sleep 5"]);
        let started = Instant::now();
        let err = run_lines_within(&mut slow, Duration::from_millis(200)).expect_err("killed");
        assert!(err.contains("killed after 200ms"), "{err}");
        assert!(started.elapsed() < Duration::from_secs(4));
        let mut quick = Command::new("sh");
        quick.args(["-c", "exit 0"]);
        assert_eq!(
            run_lines_within(&mut quick, Duration::from_secs(60)),
            Ok(vec![])
        );
    }

    #[test]
    fn a_large_output_does_not_stall_the_child() {
        let mut loud = Command::new("sh");
        loud.args(["-c", "yes gm | head -n 200000"]);
        let lines = run_lines_within(&mut loud, Duration::from_secs(60)).expect("runs");
        assert_eq!(lines.len(), 200_000);
    }

    #[test]
    fn the_child_budget_is_the_one_the_deviation_names() {
        // Pinned with a reason: the O(n^2) circle-packing fallback (`runner.rs`'s
        // `CHILD_TIMEOUT` doc) needs more than the two-stage gate's 900s, and a gate row
        // that silently lost that budget would stop being the check it claims to be.
        assert_eq!(CHILD_TIMEOUT, Duration::from_secs(2700));
        assert!(CHILD_TIMEOUT > Duration::from_secs(900));
    }

    #[test]
    fn workspace_root_holds_the_harness_and_file_sha256_hashes_its_bytes() {
        let harness = workspace_root().join("harness").join("wasm-run.mjs");
        assert!(harness.is_file(), "{}", harness.display());
        let bytes = std::fs::read(&harness).expect("readable");
        assert_eq!(file_sha256(&harness), Ok(sha256_hex(&bytes)));
        assert!(file_sha256(Path::new("/nonexistent/gm")).is_err());
    }
}
