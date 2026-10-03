//! `graph-cli mb-fidelity` end to end: the gate that measures how far each many-body solver
//! stands from the exact all-pairs sum, and its own negative control going red.
//!
//! Two controls, one per direction, and neither is an assertion about a number in a table:
//!
//! * **positive** — `--require bh:0.5` exits 0. A finer opening angle than the frozen one
//!   is a *smaller* error by construction, so this can only pass if the error really is
//!   measured and compared against `bh:0.9`'s on the same set.
//! * **negative** — `--require bh:2.0` exits 1. A coarser opening angle is a larger error,
//!   so this can only go red if the comparison is real. A gate that always exits 0 fails
//!   this one; a gate that always exits 1 fails the other.
//!
//! Both assert the exit code rather than the table's contents, because the table's numbers
//! are the *result* — the job's report carries them and a test that pinned them would turn a
//! measurement into a fixed expectation. The self-check row is asserted separately, because
//! that one is a claim the run makes about its own reference and must hold for the rest to
//! mean anything.
//!
//! 1000 nodes, because the exact reference is `O(n²)` and these are the gate's own arms:
//! `--n 1000` runs in seconds where `--n 50000` runs the same code with a hundred times the
//! pairs. The honest multi-size sweep is `docs/measurements/perf-mb-fidelity.md`.

mod common;

use common::stdout;
use std::process::Output;

/// A gates directory per test: these write the same record name and the harness runs them in
/// parallel, so a shared directory would have each test reading another's evidence. That
/// matters more here than in most files: `evidence::write` *refuses* a failing record while a
/// passing one stands, so a shared directory would let the positive run's record mask the
/// negative run's and the red would go missing.
fn gates_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("gm-mb-fidelity-{name}-{}", std::process::id()))
}

fn graph_cli(dir: &std::path::Path, args: &[&str]) -> Output {
    common::graph_cli(dir, args, None)
}

fn said(output: &Output) -> String {
    format!(
        "{}{}",
        stdout(output),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn record(dir: &std::path::Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(format!("{name}.json"))).expect("recorded")
}

/// Positive control: a finer `theta` than the frozen default is a smaller force error, so
/// requiring it exits 0. Also the honest run of the gate: the self-check row, the table, and
/// the recorded evidence.
#[test]
fn a_finer_opening_angle_than_the_frozen_one_passes_and_records() {
    let dir = gates_dir("positive");
    let run = graph_cli(&dir, &["mb-fidelity", "--n", "1000", "--require", "bh:0.5"]);
    let out = said(&run);
    assert_eq!(run.status.code(), Some(0), "{out}");
    assert!(
        out.contains("| n | set | solver | rms | p50 | p99 | pass |"),
        "{out}"
    );
    assert!(out.contains("| 1000 | start | bh:0.5 |"), "{out}");
    assert!(out.contains("| 1000 | settled | bh:0.5 |"), "{out}");
    assert!(out.trim_end().ends_with("PASS"), "{out}");
    let recorded = record(&dir, "mb-fidelity");
    assert!(recorded.contains("\"pass\": true"), "{recorded}");
    assert!(recorded.contains("\"self_check_held\": true"), "{recorded}");
    assert!(recorded.contains("\"baseline\": \"bh:0.9\""), "{recorded}");
}

/// The self-check is the run's claim about its own reference: `bh:0` opens every cell, so its
/// rows are the exact sum in a different order and must agree with it to rounding. Asserted
/// here because the positive control's exit 0 already depends on it holding — a run that
/// trusted a broken reference would still exit 0, and this row is what says the reference
/// was checked.
#[test]
fn the_zero_theta_self_check_agrees_with_the_exact_sum() {
    let dir = gates_dir("self-check");
    let run = graph_cli(&dir, &["mb-fidelity", "--n", "1000"]);
    let out = said(&run);
    assert_eq!(run.status.code(), Some(0), "{out}");
    for set in ["start", "settled"] {
        let row = out
            .lines()
            .find(|line| line.starts_with(&format!("| 1000 | {set} | bh:0 |")))
            .unwrap_or_else(|| panic!("a bh:0 row for {set}: {out}"));
        // `1e-15`, read out of the printed `%.3e` rather than re-derived: the tolerance is
        // `1e-9` and the assertion is that the measured value is *far* under it, which a
        // printed row can say and a hardcoded constant could not survive a change to.
        let rms: f64 = row
            .split('|')
            .nth(4)
            .expect("an rms column")
            .trim()
            .parse()
            .unwrap_or_else(|_| panic!("rms is a number: {row}"));
        assert!(rms <= 1e-9, "bh:0 against the exact sum at {set}: {row}");
    }
}

/// Negative control: a coarser `theta` is a *larger* force error than the frozen default's,
/// so requiring it must go red. The row is asserted as failing as well as the exit code, so
/// the red is attributable to the comparison rather than to an unrelated error.
#[test]
fn a_coarser_opening_angle_than_the_frozen_one_fails() {
    let dir = gates_dir("negative");
    let run = graph_cli(&dir, &["mb-fidelity", "--n", "1000", "--require", "bh:2.0"]);
    let out = said(&run);
    assert_eq!(run.status.code(), Some(1), "{out}");
    for set in ["start", "settled"] {
        let row = out
            .lines()
            .find(|line| line.starts_with(&format!("| 1000 | {set} | bh:2 |")))
            .unwrap_or_else(|| panic!("a bh:2 row for {set}: {out}"));
        assert!(row.trim_end().ends_with("| no |"), "{row}");
    }
    assert!(out.contains("FAIL: n=1000 start bh:2"), "{out}");
}
