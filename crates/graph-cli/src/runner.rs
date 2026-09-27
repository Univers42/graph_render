//! Running the other programs a gate needs — cargo, node, the harness — and hashing
//! what they produce. Shared by the hash gate and the determinism probe.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Builds `graph_wasm.wasm` in release mode and returns its path.
pub fn build_wasm() -> Result<PathBuf, String> {
    let root = workspace_root();
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let args = [
        "build",
        "--quiet",
        "--release",
        "-p",
        "graph-wasm",
        "--target",
        "wasm32-unknown-unknown",
    ];
    let status = Command::new(cargo).current_dir(&root).args(args).status();
    match status {
        Ok(s) if s.success() => {}
        Ok(s) => return Err(format!("building graph-wasm for wasm32 failed: {s}")),
        Err(e) => return Err(format!("running cargo: {e}")),
    }
    let target =
        std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from);
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
    let output = command
        .output()
        .map_err(|e| format!("spawning {command:?}: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{command:?} exited {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    let stdout = String::from_utf8(output.stdout).map_err(|e| format!("non-UTF-8 output: {e}"))?;
    Ok(stdout.lines().map(str::to_owned).collect())
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
    fn workspace_root_holds_the_harness_and_file_sha256_hashes_its_bytes() {
        let harness = workspace_root().join("harness").join("wasm-run.mjs");
        assert!(harness.is_file(), "{}", harness.display());
        let bytes = std::fs::read(&harness).expect("readable");
        assert_eq!(file_sha256(&harness), Ok(sha256_hex(&bytes)));
        assert!(file_sha256(Path::new("/nonexistent/gm")).is_err());
    }
}
