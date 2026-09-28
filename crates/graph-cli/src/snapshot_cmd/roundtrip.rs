//! `roundtrip --seeds N`: the sweep, its findings, and the ledger record.
//!
//! Each seed checks the grid pipeline's snapshot, every other registered layout's, and a
//! contract exercise drawing every kind, adversarial floats and ids from the seed. Three
//! layouts — grid, circular, packing — are also held to their own stated convention
//! restated by hand ([`super::hand_oracles`]), which is the evidence `layout.grid`,
//! `layout.circular.radial` and `layout.packing.circle` are gated on (tidy tree and
//! treemap are gated on `harness/oracle-layouts.mjs` instead: see
//! `crate::capabilities::registry`).

use super::{exercise, hand_oracles, pipeline};
use crate::evidence;
use graph_core::{gate_node_count, registry};
use serde_json::json;
use std::process::ExitCode;

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
    /// Exercise snapshots per notes case (`exercise::count_notes_cases`).
    notes: [u64; 5],
}

/// The notes cases, in `Findings::notes` order.
const NOTES_CASES: [&str; 5] = ["0.2-labelled", "0.3 k=0", "code 1", "code 2", "code 3"];

impl Findings {
    fn pass(&self) -> bool {
        [&self.faces, &self.grid, &self.circular, &self.packing]
            .into_iter()
            .all(Vec::is_empty)
            && self.notes.iter().all(|&c| c > 0)
    }
}

/// `roundtrip --seeds N`.
pub fn run(seeds: u32) -> ExitCode {
    let swept = evidence::Stamp::take().and_then(|stamp| Ok((stamp, sweep(seeds)?)));
    let (stamp, found) = match swept {
        Ok(swept) => swept,
        Err(err) => {
            eprintln!("roundtrip: could not run: {err}");
            return ExitCode::from(2);
        }
    };
    let pass = found.pass();
    print_findings(seeds, &found);
    let hand =
        |unexplained: usize| json!({ "cases": seeds, "declared": 0, "unexplained": unexplained });
    let notes: serde_json::Map<_, _> = (NOTES_CASES.iter().zip(found.notes))
        .map(|(case, count)| ((*case).to_owned(), json!(count)))
        .collect();
    let snapshots = u64::from(seeds) * (1 + registry::LAYOUTS.len() as u64);
    let body = json!({
        "seeds": seeds, "pass": pass, "snapshots": snapshots,
        "faces_failed": found.faces.len(), "notes_cases": notes,
        "functions": {
            "layout.grid": hand(found.grid.len()),
            "layout.circular.radial": hand(found.circular.len()),
            "layout.packing.circle": hand(found.packing.len()),
        }
    });
    if let Err(err) = evidence::write(&stamp, "roundtrip", body) {
        eprintln!("roundtrip: not recorded: {err}");
        return ExitCode::from(2);
    }
    println!("{}", if pass { "PASS" } else { "FAIL" });
    ExitCode::from(if pass { 0 } else { 1 })
}

fn sweep(seeds: u32) -> Result<Findings, String> {
    if seeds == 0 {
        return Err("0 seeds: a sweep over nothing proves nothing".into());
    }
    let mut found = Findings::default();
    for seed in 0..seeds {
        let nodes = gate_node_count(seed);
        let grid = pipeline(seed, nodes, "grid")?.snapshot;
        let circular = pipeline(seed, nodes, "circular.radial")?.snapshot;
        let packing = pipeline(seed, nodes, "packing.circle")?.snapshot;
        let exercise = exercise::snapshot(seed)?;
        exercise::count_notes_cases(&exercise, &mut found.notes);
        let mut faces = vec![("grid", &grid), ("exercise", &exercise)];
        faces.push(("circular.radial", &circular));
        faces.push(("packing.circle", &packing));
        let others: Vec<_> = OTHER_LAYOUTS
            .iter()
            .map(|&name| pipeline(seed, nodes, name).map(|r| (name, r.snapshot)))
            .collect::<Result<_, _>>()?;
        for (name, snapshot) in &others {
            faces.push((name, snapshot));
        }
        for (what, snapshot) in faces {
            if let Err(why) = super::faces_agree(snapshot) {
                found.faces.push(format!("seed {seed} {what}: {why}"));
            }
        }
        if let Err(why) = hand_oracles::grid(&grid) {
            found.grid.push(format!("seed {seed}: {why}"));
        }
        if let Err(why) = hand_oracles::circular(seed, nodes, &circular) {
            found.circular.push(format!("seed {seed}: {why}"));
        }
        if let Err(why) = hand_oracles::packing(&packing) {
            found.packing.push(format!("seed {seed}: {why}"));
        }
    }
    Ok(found)
}

fn print_findings(seeds: u32, found: &Findings) {
    let snapshots = u64::from(seeds) * (1 + registry::LAYOUTS.len() as u64);
    println!(
        "roundtrip: seeds={seeds} snapshots={snapshots} (every registered layout + contract exercise)"
    );
    let faces_ok = snapshots - found.faces.len() as u64;
    println!("  binary <-> JSON byte-exact on {faces_ok}/{snapshots} snapshots");
    for (name, findings) in [
        ("layout.grid", &found.grid),
        ("layout.circular.radial", &found.circular),
        ("layout.packing.circle", &found.packing),
    ] {
        let ok = u64::from(seeds) - findings.len() as u64;
        println!("  {name} on its stated conventions on {ok}/{seeds} seeds");
    }
    let cases = NOTES_CASES.iter().zip(found.notes);
    let drawn: Vec<String> = cases.map(|(case, n)| format!("{case} {n}")).collect();
    println!(
        "  notes cases drawn (exercise, each needed): {}",
        drawn.join(", ")
    );
    let all_failures = found
        .faces
        .iter()
        .chain(&found.grid)
        .chain(&found.circular)
        .chain(&found.packing);
    for line in all_failures.take(6) {
        println!("  FAILED {line}");
    }
}

#[cfg(test)]
mod tests {
    use super::sweep;

    #[test]
    fn the_sweep_records_nothing_wrong_and_refuses_zero_seeds() {
        let found = sweep(12).expect("runs");
        assert!(found.faces.is_empty() && found.grid.is_empty(), "{found:?}");
        assert!(
            found.circular.is_empty() && found.packing.is_empty(),
            "{found:?}"
        );
        assert!(found.pass(), "every notes case drawn: {:?}", found.notes);
        assert!(
            !sweep(4).expect("runs").pass(),
            "four seeds cannot draw every case"
        );
        assert!(sweep(0).expect_err("empty").starts_with("0 seeds"));
    }
}
