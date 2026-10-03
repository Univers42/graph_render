//! What the hash gate reports: the detail block it prints when arms disagree, the record
//! it leaves for the ledger, and the exit code it hands back. Each is built as a value
//! (`String`, `serde_json::Value`, `ExitCode`) rather than printed in place, so a test can
//! hold every word, key and count of them exactly.
//!
//! **The verdict lives here, not in `super`, because it is the one place the record, the
//! printed counts and the exit code can disagree** (RG-06). They are all derived from the
//! same [`Tally`] and the same C20 count, and [`passed`] is the single predicate that both
//! the record's `"pass"` and [`exit`] read — so a C20 mismatch cannot leave the record
//! saying `"pass": true` while the process exits 0. Every length the three share is checked
//! here too (RG-07): a stage missing from the tally is a refusal, not a shorter map, and an
//! arm shorter than the one the detail block indexes is a refusal, not a truncation.

use super::compare::{C20_ARM, arm};
use super::tiered::THREADED_STAGES;
use super::transport;
use super::{Arm, Knob, LAYOUT, TRANSPORT, Tally, diverged, per_stage, stages};
use crate::evidence;
use crate::runner::sha256_hex;
use serde_json::json;
use std::process::ExitCode;

/// How many diverged lines the detail block shows; the rest are counted, not printed.
const SHOWN: usize = 3;

/// The only arm name the threaded tier uses (`tier::Tier::arm_name`), so an arm that
/// recomputed something over threads is recognisable by name alone.
const THREADED_ARM: &str = "native threads ";

/// **The one verdict predicate.** A run passed when no seed diverged in any stage *and*
/// the C20 tally reached every seed; the record's `"pass"` and [`exit`] both read this, so
/// the two cannot be kept in agreement by every future caller remembering to re-check the
/// half it does not itself compute (RG-06).
pub fn passed(tally: &Tally, c20: u32, seeds: u32) -> bool {
    tally.diverged_seeds == 0 && c20 == seeds
}

/// The run's exit code, from the same `pass` the record carries.
pub fn exit(pass: bool) -> ExitCode {
    if pass {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// Each arm's digest, then every arm's own line for the first [`SHOWN`] diverged lines.
///
/// Fire-and-forget, for a caller that has already compared its arms — `forcecheck`'s own
/// gate reaches this through `compare::diverged`, which refuses a short arm before any of
/// it runs, so nothing here can index past the end. The hash gate uses
/// [`checked_arm_report`] instead, because *its* record is the evidence the ledger reads.
pub fn arm_report(out: &mut String, arms: &[Arm], lines: &[usize]) {
    let _ = checked_arm_report(out, arms, lines);
}

/// [`arm_report`], refusing rather than truncating.
///
/// **Every arm must have printed the same number of lines** before anything is indexed by
/// that number (RG-07). `compare::diverged` refuses that today, so this is the belt to its
/// braces: the alternative was a `panic!` on `arms[0].1[i]` and a *silently truncated*
/// digest list, and neither is a thing a gate that hashes evidence should be able to do
/// with an arm that exited 0 having printed nothing.
pub fn checked_arm_report(out: &mut String, arms: &[Arm], lines: &[usize]) -> Result<(), String> {
    use std::fmt::Write as _;
    let printed = one_length(arms)?;
    for (name, output) in arms {
        let digest = sha256_hex(output.join("\n").as_bytes());
        let _ = writeln!(out, "  {name:<13} digest {digest}");
    }
    for &i in lines.iter().take(SHOWN) {
        if i >= printed {
            return Err(format!(
                "line {i} is past the {printed} lines every arm printed"
            ));
        }
        let stage = arms[0].1[i].rsplit_once(' ').map_or("", |p| p.0);
        let _ = writeln!(out, "  DIVERGED {stage}:");
        for (name, output) in arms {
            let _ = writeln!(out, "    {name:<13} {}", output[i]);
        }
    }
    Ok(())
}

/// The number of lines every arm printed, refused unless they all printed that many.
fn one_length(arms: &[Arm]) -> Result<usize, String> {
    let (reference, first) = arms
        .first()
        .map(|(name, lines)| (*name, lines.len()))
        .ok_or("no arms to report: a detail block with no arm in it reports no comparison")?;
    for (name, lines) in arms {
        if lines.len() != first {
            return Err(format!(
                "{name} printed {} lines, {reference} printed {first}",
                lines.len()
            ));
        }
    }
    Ok(first)
}

/// The record itself: every stage name and its count of seeds the arms agreed on, the arm
/// count, the C20 transport tally, the mutation this run perturbed, and the verdict.
///
/// `arms` is in the record because the per-stage counts mean nothing without it: `equal:
/// 8` is a different claim at 4 arms than at 9, and the capabilities ledger reads this
/// file without re-running the gate.
///
/// **Refused, not truncated, when the tally and the stage list disagree** (RG-07): a `zip`
/// over the shorter side would drop a stage from the record silently, and a record missing
/// a stage reads as a record in which that stage was equal.
pub fn body(
    control: Option<Knob>,
    seeds: u32,
    tally: &Tally,
    c20: u32,
    arms: &[Arm],
) -> Result<serde_json::Value, String> {
    let stages = stages();
    if tally.equal.len() != stages.len() {
        return Err(format!(
            "the tally holds {} stage counts, the gate hashes {} stages",
            tally.equal.len(),
            stages.len()
        ));
    }
    let equal: serde_json::Map<_, _> = stages
        .iter()
        .zip(&tally.equal)
        .map(|(stage, equal)| ((*stage).to_owned(), json!(equal)))
        .collect();
    Ok(json!({
        "seeds": seeds,
        "arms": arms.len(),
        "arm_names": arms.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        "pass": passed(tally, c20, seeds),
        "equal": equal,
        "transport": {"stage": TRANSPORT, "reference": LAYOUT, "equal": c20},
        "mutation": control.map(Knob::env),
    }))
}

/// Compares the arms, prints what it found, and returns the exit code. The gate's own body,
/// split out of `super` by the house's line cap and because it is the reporting concern.
pub fn verdict(
    stamp: &evidence::Stamp,
    control: Option<Knob>,
    seeds: u32,
    arms: &[Arm],
) -> ExitCode {
    let mutation = control.map_or("none", Knob::env);
    println!(
        "hashgate: stages={} seeds={seeds} control={mutation}",
        stages().join(",")
    );
    let lines = match diverged(seeds, &stages(), arms) {
        Ok(lines) => lines,
        Err(err) => {
            eprintln!("hashgate: arms not comparable: {err}");
            return ExitCode::from(2);
        }
    };
    let mut detail = String::new();
    if let Err(err) = checked_arm_report(&mut detail, arms, &lines) {
        eprintln!("hashgate: arms not reportable: {err}");
        return ExitCode::from(2);
    }
    print!("{detail}");
    let tally = per_stage(seeds, stages().len(), &lines);
    for (stage, equal) in stages().iter().zip(&tally.equal) {
        println!("  {stage}: {} on {equal}/{seeds} seeds", ways(stage, arms));
    }
    conclude(stamp, control, seeds, &tally, arms)
}

/// How many arms really recomputed `stage`, named rather than implied (RG-39).
///
/// **A stage outside [`THREADED_STAGES`] is hashed from the scalar arm's own bytes** by the
/// threaded arms, so their agreement there is the arm compared with itself; a stage inside
/// it is recomputed by every `native threads N` arm. Both are said out loud, because
/// "9-way equal" reads as nine computations and is five.
pub(crate) fn ways(stage: &str, arms: &[Arm]) -> String {
    let all = arms.len();
    let threaded = arms
        .iter()
        .filter(|(name, _)| name.starts_with(THREADED_ARM))
        .count();
    if threaded == 0 {
        return format!("{all}-way equal");
    }
    if THREADED_STAGES.contains(&stage) {
        return format!("{all}-way equal ({threaded} of them a real threaded arm)");
    }
    format!("{all}-way equal ({threaded} of them hashed the scalar arm's own bytes)")
}

/// The C20 tally, the record, and the exit code: the last of the gate's work, split out of
/// [`verdict`] by the house's 40-line-per-function limit.
fn conclude(
    stamp: &evidence::Stamp,
    control: Option<Knob>,
    seeds: u32,
    tally: &Tally,
    arms: &[Arm],
) -> ExitCode {
    // C20, counted from the wasm arm's own lines (run 1; run 2 is 4-way equal to it):
    // the real ABI's snapshot against the retained shim's, per seed. The arm is **named**,
    // not indexed: `arms[2]` was a positional read of a list `compare` accepts at any
    // length, and the tally was the one place a shorter list became a panic rather than a
    // refusal (RG-36).
    let c20 = match arm(arms, C20_ARM).and_then(|lines| transport::agree_with_shim(seeds, lines)) {
        Ok(agreed) => agreed,
        Err(err) => {
            eprintln!("hashgate: the C20 tally could not be read: {err}");
            return ExitCode::from(2);
        }
    };
    println!("  {TRANSPORT}: the real ABI matched {LAYOUT} on {c20}/{seeds} seeds");
    let bad = tally.diverged_seeds;
    println!(
        "  {}-way equal on {}/{seeds} seeds",
        arms.len(),
        seeds - bad
    );
    if let Err(err) = record(stamp, control, seeds, tally, c20, arms) {
        eprintln!("hashgate: not recorded: {err}");
        return ExitCode::from(2);
    }
    if c20 != seeds {
        println!(
            "FAIL: {TRANSPORT} diverges from {LAYOUT} on {} seeds",
            seeds - c20
        );
    } else if bad == 0 {
        println!("PASS");
    } else {
        println!("FAIL: {bad} of {seeds} seeds diverge");
    }
    exit(passed(tally, c20, seeds))
}

/// Writes this run's result for the ledger: `hashgate.json` for an honest run, the
/// knob's own record for a negative control. A run that cannot record exits 2: its
/// verdict would otherwise stand with no evidence behind it. A run that is refused
/// because a passing record stands is only a warning — the gate ran, and its exit code
/// is the verdict (`evidence::record` draws that line for every gate). The `transport`
/// tally goes in the same record, because it *is* the hash gate's verdict — the C20
/// count `capabilities/verdict.rs` reads for the `transport.wasm.columnar` row.
fn record(
    stamp: &evidence::Stamp,
    control: Option<Knob>,
    seeds: u32,
    tally: &Tally,
    c20: u32,
    arms: &[Arm],
) -> Result<(), String> {
    let name = control.map_or("hashgate", Knob::record);
    evidence::record(stamp, name, body(control, seeds, tally, c20, arms)?)
}
