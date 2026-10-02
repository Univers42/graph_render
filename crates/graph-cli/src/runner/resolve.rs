//! Turning the program names a gate runs into absolute paths to files that exist and can
//! be executed, and pinning the one path a run will use.
//!
//! Why this is not just `Command::new("cargo")`: a gate hashes whatever the wasm build and
//! the Node arm produce, so the binary behind a name has to be a real file this process
//! found, not a name that resolves to something else at spawn time or not at all. A name
//! that resolves to nothing is reported, because "could not run" (exit 2) and a stale
//! artifact hashed under the build's name (exit 0) are opposites.
//!
//! The search is here in Rust rather than shelled out to `which`/`command -v`: spawning a
//! shell to answer a question about the filesystem adds an interpreter between the answer
//! and the filesystem, and no dependency for a dozen lines of work.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Resolves `program` to an absolute path to an existing file, or `Err` naming the program
/// and the `PATH` searched.
///
/// `path` is the search list to use, or `None` for this process's `PATH`. Passing the list
/// in rather than reading the environment is what lets a test drive the search without
/// changing the environment every other test in the binary shares.
///
/// Two rules, in order:
/// 1. `program` containing a path separator is a path, not a name: it is taken as given
///    (relative to the current directory) and never searched for. It must exist and be a
///    file — a path the caller spelled out that is not there is refused by name, because
///    silently falling back to `PATH` would run something the caller did not ask for.
/// 2. Otherwise each entry of `path` is tried in order, an empty entry meaning the current
///    directory (the shell's own rule). A candidate that is missing or not executable is
///    skipped, not an error: `PATH` is a list of places, not one place.
pub fn on_path(program: &str, path: Option<&OsStr>) -> Result<PathBuf, String> {
    let path = path.unwrap_or_else(|| OsStr::new(""));
    if program.contains(std::path::MAIN_SEPARATOR) || program.contains('/') {
        return absolute_spellout(program);
    }
    for dir in std::env::split_paths(path) {
        let dir = if dir.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            dir
        };
        let candidate = dir.join(program);
        if is_usable(&candidate) {
            return absolutise(&candidate);
        }
    }
    Err(format!(
        "no executable {program:?} on PATH {:?}; searched {path:?}",
        path_env_display(path)
    ))
}

/// `PATH` as a readable string for a refusal message; a non-UTF-8 list is reported lossily
/// rather than not at all, since the point of the message is to be actionable.
fn path_env_display(path: &OsStr) -> String {
    std::env::split_paths(path)
        .map(|dir| dir.display().to_string())
        .collect::<Vec<_>>()
        .join(":")
}

/// A path the caller spelled out with a separator in it: absolutised, and refused unless it
/// is an existing file. No execute-bit test here, because the caller named this file and a
/// spawn failure on it is the honest report; testing the bit would refuse a directory-tree
/// mistake with a message about permissions instead of about the path.
fn absolute_spellout(program: &str) -> Result<PathBuf, String> {
    let candidate = absolutise(Path::new(program))?;
    if candidate.is_file() {
        Ok(candidate)
    } else {
        Err(format!(
            "{program:?} is not an existing file ({})",
            candidate.display()
        ))
    }
}

/// `candidate` as an absolute path, anchored on the current directory when it is relative —
/// so the returned path means the same thing to a caller whose own directory differs, which
/// is exactly the mix-up a relative target dir caused.
fn absolutise(candidate: &Path) -> Result<PathBuf, String> {
    if candidate.is_absolute() {
        return Ok(normalise(candidate));
    }
    let cwd = std::env::current_dir().map_err(|e| format!("the current directory: {e}"))?;
    Ok(normalise(&cwd.join(candidate)))
}

/// Drops `.` components so the refused and accepted paths read the same as the `PATH`
/// entries they came from; symlinks are deliberately *not* resolved, because the file the
/// gate runs is the one found, not the one it points at.
#[cfg(unix)]
fn normalise(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(not(unix))]
fn normalise(path: &Path) -> PathBuf {
    path.to_path_buf()
}

/// Whether `candidate` is a file this process may execute: existing, a file rather than a
/// directory, and carrying the execute bit on unix.
fn is_usable(candidate: &Path) -> bool {
    if !candidate.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match candidate.metadata() {
            Ok(meta) => meta.permissions().mode() & 0o111 != 0,
            Err(_) => false,
        }
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// The target directory cargo writes and the caller reads, for a configured
/// `CARGO_TARGET_DIR` of `configured` (or the workspace default when it is `None`).
///
/// A relative value is joined onto `root` and *returned* joined: cargo is invoked with
/// `current_dir(root)`, so a relative `--target-dir` is resolved against `root` by cargo,
/// while a relative return value would be resolved against the CLI's own directory. Those
/// are different directories, and the mismatch let the gate hash a pre-existing artifact from
/// another crate's target dir instead of the one this build wrote. Absolute values are
/// returned as given, because cargo resolves those against nothing.
pub fn target_dir(configured: Option<&OsStr>, root: &Path) -> PathBuf {
    let Some(value) = configured.map(PathBuf::from) else {
        return root.join("target");
    };
    if value.is_absolute() {
        value
    } else {
        root.join(value)
    }
}

/// `cargo`, resolved once per process from `$CARGO` or the bare name, through [`on_path`].
///
/// Cached because the gate launches four arms and every one of them must be run by the same
/// binary: resolving per call would let a `PATH` edited between arms — or a wrapper shadowing
/// `cargo` between arms — produce arms hashed by different tools, which is the failure the
/// hash gate exists to catch. A refusal is cached too, and honestly: an environment that
/// cannot name `cargo` once cannot name it later in the same run, and re-searching would only
/// make the failure move.
static CARGO: OnceLock<Result<PathBuf, String>> = OnceLock::new();

/// [`CARGO`], or the refusal explaining why there is none.
pub fn cargo() -> Result<&'static Path, String> {
    CARGO.get_or_init(resolve_cargo)
        .as_deref()
        .map_err(|err| err.clone())
}

/// `$CARGO` if set (that value is honoured — a wrapper cargo installed is a legitimate
/// cargo), else the bare name `cargo`; either way resolved by [`on_path`].
fn resolve_cargo() -> Result<PathBuf, String> {
    let name = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    on_path(&name.to_string_lossy(), std::env::var_os("PATH").as_deref())
}

/// `node`, resolved once per process from `PATH` through [`on_path`], cached for the same
/// reason as [`CARGO`]: the Node arms must be produced by one interpreter.
static NODE: OnceLock<Result<PathBuf, String>> = OnceLock::new();

/// [`NODE`], or the refusal explaining why there is none.
pub fn node() -> Result<&'static Path, String> {
    NODE.get_or_init(|| on_path("node", std::env::var_os("PATH").as_deref()))
        .as_deref()
        .map_err(|err| err.clone())
}

#[cfg(test)]
#[path = "resolve/tests.rs"]
mod tests;
