//! `roundtrip --seeds N`: the sweep, its findings, and the ledger record.
//!
//! Each seed checks the grid pipeline's snapshot, every other registered layout's, and a
//! contract exercise drawing every kind, adversarial floats and ids from the seed. Three
//! layouts — grid, circular, packing — are also held to their own stated convention
//! restated by hand ([`super::hand_oracles`]), and the layered drawing to its structural
//! invariants ([`super::dag`]), which is the evidence `layout.grid`,
//! `layout.circular.radial`, `layout.packing.circle` and `layout.dag.sugiyama` are gated
//! on (tidy tree and
//! treemap are gated on `harness/oracle-layouts.mjs` instead: see
//! `crate::capabilities::registry`).

use super::short_name;
use crate::evidence;
use graph_core::registry;
use seed::{progress_line, seed_finding};
use serde_json::json;
use std::process::ExitCode;

mod seed;

/// The two layouts `harness/oracle-layouts.mjs` gates instead of a hand oracle here:
/// still swept below for the wire-format round trip, just not held to an independent
/// formula in this crate.
const OTHER_LAYOUTS: [&str; 2] = ["tree.tidy", "treemap.squarified"];

/// What the sweep found wrong, by check, and which notes cases it drew. Every list empty
/// and every notes case drawn is a pass.
#[derive(Debug, Default)]
struct Findings {
    /// Snapshots whose faces did not round-trip.
    faces: Vec<String>,
    /// Seeds whose grid is off its conventions.
    grid: Vec<String>,
    /// Seeds whose circular/radial layout is off its conventions.
    circular: Vec<String>,
    /// Seeds whose circle packing breaks its own promise (finite, positive, tangent).
    packing: Vec<String>,
    /// Seeds whose layered drawing breaks a structural invariant.
    dag: Vec<String>,
    /// Exercise snapshots per notes case (`exercise::count_notes_cases`).
    notes: [u64; 5],
    /// Exercise snapshots that are 3D, z column and all. Counted rather than assumed, so a
    /// seed rule that stopped drawing 3D fails the sweep instead of quietly narrowing it.
    three_d: u64,
    /// Snapshots the sweep actually put through both faces, over every seed.
    checked: u64,
}

/// The notes cases, in `Findings::notes` order.
const NOTES_CASES: [&str; 5] = ["0.2-labelled", "0.3 k=0", "code 1", "code 2", "code 3"];

/// Every layout the sweep runs, by short name, in registry order: taken from the registry
/// itself rather than listed, so a layout added there is swept here without being added
/// here, and [`OTHER_LAYOUTS`] is checked against it rather than trusted.
fn swept_layouts() -> Vec<&'static str> {
    let names: Vec<&'static str> = registry::LAYOUTS
        .iter()
        .map(|layout| short_name(layout.id))
        .collect();
    for d3_only in OTHER_LAYOUTS {
        assert!(names.contains(&d3_only), "{d3_only} is not registered");
    }
    names
}

/// Snapshots `seeds` seeds check: every registered layout, plus the contract exercise.
fn snapshot_total(seeds: u32) -> u64 {
    u64::from(seeds) * (1 + registry::LAYOUTS.len() as u64)
}

impl Findings {
    /// Every check clean, every notes case drawn, and exactly the snapshots the registry
    /// promises actually checked — so a layout swept by accident less is a failure.
    fn pass(&self, seeds: u32) -> bool {
        self.checked == snapshot_total(seeds)
            && [
                &self.faces,
                &self.grid,
                &self.circular,
                &self.packing,
                &self.dag,
            ]
            .into_iter()
            .all(Vec::is_empty)
            && self.notes.iter().all(|&c| c > 0)
            // 3D is proved here, so the sweep has to actually contain some: `seed % 3 == 2`
            // draws a third of the exercise snapshots in 3D, and a seed count below 3
            // cannot draw one.
            && self.three_d > 0
    }
}

/// `roundtrip --seeds N`.
pub fn run(seeds: u32) -> ExitCode {
    let swept = evidence::Stamp::take().and_then(|stamp| Ok((stamp, sweep_with(seeds, progress)?)));
    let (stamp, found) = match swept {
        Ok(swept) => swept,
        Err(err) => {
            eprintln!("roundtrip: could not run: {err}");
            return ExitCode::from(2);
        }
    };
    print_findings(seeds, &found);
    if let Err(err) = evidence::record(&stamp, "roundtrip", body(seeds, &found)) {
        eprintln!("roundtrip: not recorded: {err}");
        return ExitCode::from(2);
    }
    let pass = found.pass(seeds);
    println!("{}", if pass { "PASS" } else { "FAIL" });
    verdict(&found, seeds)
}

/// `run`'s progress sink: one line per seed on standard error, so a stall in the middle
/// of a long sweep is a named seed in the log rather than an empty file.
fn progress(line: &str) {
    eprintln!("{line}");
}

/// The run's exit code, the one thing a gate reads: `0` only when every check is clean.
fn verdict(found: &Findings, seeds: u32) -> ExitCode {
    if found.pass(seeds) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// The record the ledger reads, built here so a test can hold it to the exact JSON: no
/// count, name or flag in it can move without that going red.
fn body(seeds: u32, found: &Findings) -> serde_json::Value {
    let hand =
        |unexplained: usize| json!({ "cases": seeds, "declared": 0, "unexplained": unexplained });
    let notes: serde_json::Map<_, _> = (NOTES_CASES.iter().zip(found.notes))
        .map(|(case, count)| ((*case).to_owned(), json!(count)))
        .collect();
    json!({
        "seeds": seeds, "pass": found.pass(seeds), "snapshots": snapshot_total(seeds),
        "faces_failed": found.faces.len(), "notes_cases": notes,
        "three_d_exercise": found.three_d,
        "functions": {
            "layout.grid": hand(found.grid.len()),
            "layout.circular.radial": hand(found.circular.len()),
            "layout.packing.circle": hand(found.packing.len()),
            "layout.dag.sugiyama": hand(found.dag.len()),
        }
    })
}

#[cfg(test)]
fn sweep(seeds: u32) -> Result<Findings, String> {
    sweep_with(seeds, |_| {})
}

/// `sweep`, with one progress line per seed handed to `progress` — standard error, so the
/// findings on standard output stay a single block a reader can diff. The line is written
/// *before* the seed is worked, so the last line in a log is the seed a stalled run is
/// sitting in: a sweep that prints nothing until the end cannot say where it stopped.
fn sweep_with(seeds: u32, mut progress: impl FnMut(&str)) -> Result<Findings, String> {
    if seeds == 0 {
        return Err("0 seeds: a sweep over nothing proves nothing".into());
    }
    let mut found = Findings::default();
    let swept = swept_layouts();
    for seed in 0..seeds {
        progress(&progress_line(seed, seeds));
        seed_finding(seed, &swept, &mut found)?;
    }
    Ok(found)
}

fn print_findings(seeds: u32, found: &Findings) {
    let mut text = String::new();
    write_findings(&mut text, seeds, found);
    print!("{text}");
}

/// `print_findings`'s text, built in memory so the counts it reports can be checked.
fn write_findings(out: &mut String, seeds: u32, found: &Findings) {
    use std::fmt::Write as _;
    let snapshots = snapshot_total(seeds);
    let _ = writeln!(
        out,
        "roundtrip: seeds={seeds} snapshots={snapshots} (every registered layout + contract exercise)"
    );
    let faces_ok = found.checked - found.faces.len() as u64;
    let _ = writeln!(
        out,
        "  binary <-> JSON byte-exact on {faces_ok}/{snapshots} snapshots"
    );
    for (name, findings) in [
        ("layout.grid", &found.grid),
        ("layout.circular.radial", &found.circular),
        ("layout.packing.circle", &found.packing),
    ] {
        let ok = u64::from(seeds) - findings.len() as u64;
        let _ = writeln!(
            out,
            "  {name} on its stated conventions on {ok}/{seeds} seeds"
        );
    }
    let dag_ok = u64::from(seeds) - found.dag.len() as u64;
    let _ = writeln!(
        out,
        "  layout.dag.sugiyama on its structural invariants on {dag_ok}/{seeds} seeds"
    );
    let _ = writeln!(
        out,
        "  3D exercise snapshots (dim 1, z column) round-tripped: {}",
        found.three_d
    );
    let cases = NOTES_CASES.iter().zip(found.notes);
    let drawn: Vec<String> = cases.map(|(case, n)| format!("{case} {n}")).collect();
    let _ = writeln!(
        out,
        "  notes cases drawn (exercise, each needed): {}",
        drawn.join(", ")
    );
    let all_failures = found
        .faces
        .iter()
        .chain(&found.grid)
        .chain(&found.circular)
        .chain(&found.packing)
        .chain(&found.dag);
    for line in all_failures.take(6) {
        let _ = writeln!(out, "  FAILED {line}");
    }
}

#[cfg(test)]
mod tests;
