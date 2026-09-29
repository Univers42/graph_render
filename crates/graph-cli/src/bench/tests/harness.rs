//! The two JS arms' self-checks, **each with the negative control beside it**.
//!
//! A self-check that cannot fail is not a check. So for each harness the test suite writes
//! a temporary copy with one pinned constant changed and runs *that*, asserting it goes
//! red — which is also what proves the constant the self-check asserts is the one the
//! native arm compares against.

use std::path::Path;

use crate::bench::staging::harness_copy_with_in;

/// Runs `harness/<name>` under Node with `args`.
fn harness_lines(name: &str, args: &[&str]) -> Vec<String> {
    let root = crate::runner::workspace_root();
    let mut node = std::process::Command::new("node");
    node.current_dir(&root).arg(root.join("harness").join(name));
    node.args(args);
    crate::runner::run_lines(&mut node).expect("the harness runs")
}

fn run_node(script: &Path, args: &[&str]) -> std::process::ExitStatus {
    let mut node = std::process::Command::new("node");
    node.current_dir(crate::runner::workspace_root())
        .arg(script)
        .args(args);
    crate::runner::run_status(&mut node, std::time::Duration::from_secs(300)).expect("node runs")
}

/// The Phase 9 oracle arm's own self-check, run under Node: the pure arithmetic of the
/// campaign (median over repeats, the largest N that fits a budget) and the graph it
/// shares with the native arm, pinned in the harness that has to agree with us.
#[test]
fn the_oracle_tick_harness_self_check_passes() {
    let lines = harness_lines("oracle-tick-bench.mjs", &["--self-check"]);
    assert!(
        lines.iter().any(|l| l.contains("self-check ok")),
        "expected the self-check to report itself: {lines:?}"
    );
}

/// The negative control for the row above: a harness whose pinned constant has been
/// changed must go red.
#[test]
fn a_broken_copy_of_the_oracle_harness_fails_its_own_self_check() {
    let mutant = harness_copy_with_in(
        "oracle-tick-bench.mjs",
        "const SETTLE_TICKS = 112;",
        "const SETTLE_TICKS = 113;",
    );
    let ran = run_node(&mutant.0, &["--self-check"]);
    assert!(
        !ran.success(),
        "the mutant's self-check passed: the check cannot fail"
    );
    mutant.cleanup();
}

/// The wasm32 arm's own self-check: the ingest document it hands the motor is the strict
/// provisional shape `graph-wasm`'s `ingest.rs` reads, and its campaign arithmetic is the
/// same pinned pair. Runs without the wasm binary, so it runs anywhere Node runs.
#[test]
fn the_wasm_tick_harness_self_check_passes() {
    let lines = harness_lines("wasm-tick-bench.mjs", &["--self-check"]);
    assert!(
        lines.iter().any(|l| l.contains("self-check ok")),
        "expected the wasm self-check to report itself: {lines:?}"
    );
}

/// The negative control for the row above: a changed constant in a copy must go red.
#[test]
fn a_broken_copy_of_the_wasm_tick_harness_fails_its_own_self_check() {
    let mutant = harness_copy_with_in(
        "wasm-tick-bench.mjs",
        "const SETTLE_TICKS = 112;",
        "const SETTLE_TICKS = 111;",
    );
    let ran = run_node(&mutant.0, &["--self-check"]);
    assert!(
        !ran.success(),
        "the mutant's self-check passed: the check cannot fail"
    );
}
