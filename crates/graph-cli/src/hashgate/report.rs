//! What the hash gate reports: the detail block it prints when arms disagree, the record
//! it leaves for the ledger, and the exit code it hands back. Each is built as a value
//! (`String`, `serde_json::Value`, `ExitCode`) rather than printed in place, so a test can
//! hold every word, key and count of them exactly.

use super::{Arm, Knob, LAYOUT, TRANSPORT, Tally, stages};
use crate::runner::sha256_hex;
use serde_json::json;
use std::process::ExitCode;

/// How many diverged lines the detail block shows; the rest are counted, not printed.
const SHOWN: usize = 3;

/// The run's exit code, the one thing a gate reads: success only when no seed diverged.
pub fn exit(diverged_seeds: u32) -> ExitCode {
    if diverged_seeds == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// Each arm's digest, then every arm's own line for the first [`SHOWN`] diverged lines.
pub fn arm_report(out: &mut String, arms: &[Arm], lines: &[usize]) {
    use std::fmt::Write as _;
    for (name, output) in arms {
        let digest = sha256_hex(output.join("\n").as_bytes());
        let _ = writeln!(out, "  {name:<13} digest {digest}");
    }
    for &i in lines.iter().take(SHOWN) {
        let _ = writeln!(
            out,
            "  DIVERGED {}:",
            arms[0].1[i].rsplit_once(' ').map_or("", |p| p.0)
        );
        for (name, output) in arms {
            let _ = writeln!(out, "    {name:<13} {}", output[i]);
        }
    }
}

/// The record itself: every stage name and its count of seeds the arms agreed on, the arm
/// count, the C20 transport tally, the mutation this run perturbed, and the verdict.
///
/// `arms` is in the record because the per-stage counts mean nothing without it: `equal:
/// 8` is a different claim at 4 arms than at 9, and the capabilities ledger reads this
/// file without re-running the gate.
pub fn body(
    control: Option<Knob>,
    seeds: u32,
    tally: &Tally,
    c20: u32,
    arms: &[Arm],
) -> serde_json::Value {
    let stages: serde_json::Map<_, _> = stages()
        .iter()
        .zip(&tally.equal)
        .map(|(stage, equal)| ((*stage).to_owned(), json!(equal)))
        .collect();
    json!({
        "seeds": seeds,
        "arms": arms.len(),
        "arm_names": arms.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        "pass": tally.diverged_seeds == 0 && c20 == seeds,
        "equal": stages,
        "transport": {"stage": TRANSPORT, "reference": LAYOUT, "equal": c20},
        "mutation": control.map(Knob::env),
    })
}
