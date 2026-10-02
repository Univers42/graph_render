//! Tests for the resolver: each one a rule `on_path` and `target_dir` must honour.
//!
//! Split from `resolve.rs` by the house 300-line limit, and because these are the only tests
//! in the binary that need the filesystem under `TMPDIR`. Nothing else in the crate writes
//! there, and every directory name here is built from the process id and a counter, so no
//! two tests — and no two concurrent runs — can collide on one.

use super::*;
use std::sync::atomic::{AtomicU32, Ordering};

/// A unique empty directory under `TMPDIR`, named from the process id and a counter —
/// unique rather than random, so a failing run is reproducible and no test needs a seed.
fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let path = std::env::temp_dir().join(format!(
        "gm-resolve-{}-{tag}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).expect("temp dir");
    path
}

/// Writes `name` in `dir` with mode `0o755`: a file the search accepts.
fn executable_in(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, b"#!/bin/sh\n").expect("write");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    path
}

#[test]
fn a_program_on_the_path_is_resolved_to_an_absolute_path() {
    let dir = temp_dir("onpath");
    let written = executable_in(&dir, "gm-fake-cargo");
    let found = on_path("gm-fake-cargo", Some(dir.as_os_str()));
    std::fs::remove_dir_all(&dir).ok();
    let found = found.expect("the program is on this PATH");
    assert!(found.is_absolute(), "{found:?}");
    assert_eq!(found, written);
}

#[test]
fn a_missing_program_is_refused_by_name() {
    let dir = temp_dir("missing");
    let err = on_path("gm-no-such-program", Some(dir.as_os_str()))
        .expect_err("nothing to find on this PATH");
    std::fs::remove_dir_all(&dir).ok();
    assert!(err.contains("gm-no-such-program"), "{err}");
    assert!(err.contains("PATH"), "{err}");
}

#[test]
#[cfg(unix)]
fn a_non_executable_file_is_not_on_the_path() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("noexec");
    let file = dir.join("gm-readable-not-executable");
    std::fs::write(&file, b"data").expect("write");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    let err = on_path("gm-readable-not-executable", Some(dir.as_os_str()))
        .expect_err("readable is not executable");
    std::fs::remove_dir_all(&dir).ok();
    assert!(err.contains("gm-readable-not-executable"), "{err}");
}

#[test]
fn a_relative_program_name_is_used_as_a_path() {
    // Rule 1 held apart from rule 2 by making the bare name *findable*: a spelled-out path
    // must be refused for not existing even when a `PATH` entry offers exactly that name.
    let dir = temp_dir("spellout");
    executable_in(&dir, "gm-spelled-out");
    let err = on_path("./gm-spelled-out", Some(dir.as_os_str()))
        .expect_err("a spelled-out path is not searched for on PATH");
    std::fs::remove_dir_all(&dir).ok();
    assert!(err.contains("./gm-spelled-out"), "{err}");
}

#[test]
fn a_spelled_out_existing_file_is_accepted_without_consulting_the_path() {
    let dir = temp_dir("spellout-ok");
    let written = executable_in(&dir, "gm-here");
    let spelled = format!("{}/gm-here", dir.display());
    let found = on_path(&spelled, Some(OsStr::new("")));
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(found.expect("the file exists"), written);
}

#[test]
fn a_relative_target_dir_is_resolved_against_the_workspace_root() {
    let root = Path::new("/gm/workspace");
    assert_eq!(
        target_dir(Some(OsStr::new("build/wasm")), root),
        root.join("build/wasm"),
        "a relative CARGO_TARGET_DIR is written and read under the root cargo runs in"
    );
    assert_eq!(
        target_dir(Some(OsStr::new("/abs/target")), root),
        PathBuf::from("/abs/target"),
        "an absolute value is cargo's to resolve, not ours"
    );
    assert_eq!(target_dir(None, root), root.join("target"));
}

#[test]
fn the_resolved_cargo_and_node_answers_are_one_per_process() {
    // What the cache buys: two arms of one run cannot be run by two different binaries.
    assert_eq!(cargo(), cargo());
    assert_eq!(node(), node());
}