//! The 4-way hash gate (`prompt.md` §7.1): for every stage and seed, native run 1,
//! native run 2, wasm32 run 1 and wasm32 run 2 must produce the same SHA-256 (D7) —
//! cross-target and run-to-run in one check. Each run is its own process, so nothing (an
//! allocator address, a hash seed) can leak from one run into the next.
//!
//! The stages are the pipeline's (`graph_core::run_pipeline`): the topology, then every
//! registered layout, each hashed on its own, so a divergence names the stage it began
//! in, then the transport — the real ABI over the same model, a stage of the gate in its
//! own right (C20). The wasm arm is the real `graph_wasm.wasm` driven by
//! `harness/wasm-run.mjs` under Node, which hashes with its built-in crypto: two
//! independent SHA-256 implementations, so a broken hasher cannot agree with itself and
//! pass.
//!
//! An honest run records its result in `target/gates/hashgate.json`, and a negative
//! control (one [`Knob`] set) in that knob's own record, for the capabilities ledger.

mod compare;
mod knob;
mod report;
mod stages;
mod transport;

use crate::evidence;
use crate::runner::{build_wasm, file_sha256, node_harness, run_lines, sha256_hex};
use compare::{Arm, Tally, diverged, per_stage};
use graph_core::layout::forceatlas2::Fa2Params;
pub use knob::Knob;
use knob::{Setting, env_setting};
pub(crate) use stages::{LAYOUT, TRANSPORT};
// `stage_bytes_for` is the test seam behind `stage_bytes` (`tests/stages.rs`), not a second
// call site: the gate itself always runs the real registry.
use stages::stage_bytes;
#[cfg(test)]
use stages::stage_bytes_for;
pub(crate) use stages::stages;
use std::process::{Command, ExitCode};

/// The differential's own negative control: `GM_MUTATE_FA2_SCALING_RATIO` applied to the
/// compiled-in ForceAtlas2 parameters, so `emit-fa2-fixtures` measures a perturbed port
/// against the very reference the honest run is measured against. Refused on a typo'd or
/// doubled knob, like every other read here, rather than falling back to the default and
/// passing as green.
pub(crate) fn fa2_perturbation() -> Result<Fa2Params, String> {
    Ok(env_setting()?.fa2)
}

/// Runs all four arms over seeds `0..seeds` and compares them line by line.
pub fn run(seeds: u32) -> ExitCode {
    let started = env_setting().and_then(|setting| {
        let stamp = evidence::Stamp::take()?;
        Ok((setting.control, stamp, collect_arms(seeds)?))
    });
    match started {
        Ok((control, stamp, arms)) => report(&stamp, control, seeds, &arms),
        Err(err) => {
            eprintln!("hashgate: could not run: {err}");
            ExitCode::from(2)
        }
    }
}

/// Body of the hidden `hashgate-arm` subcommand: one native run, every stage.
pub fn arm(seeds: u32) -> ExitCode {
    match env_setting().and_then(|setting| arm_lines(seeds, &setting)) {
        Ok(lines) => {
            print!("{lines}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("hashgate-arm: {err}");
            ExitCode::from(2)
        }
    }
}

/// `stage seed sha256` lines, stage by stage, seed by seed: the pipeline runs once per
/// seed and each of its stages lands in its own block.
fn arm_lines(seeds: u32, setting: &Setting) -> Result<String, String> {
    let mut blocks = vec![String::new(); stages().len()];
    for seed in 0..seeds {
        let stages = stage_bytes(seed, setting).map_err(|err| format!("seed {seed}: {err}"))?;
        for (block, (stage, bytes)) in blocks.iter_mut().zip(stages) {
            block.push_str(&format!("{stage} {seed} {}\n", sha256_hex(&bytes)));
        }
    }
    Ok(blocks.concat())
}

fn collect_arms(seeds: u32) -> Result<Vec<Arm>, String> {
    let exe = std::env::current_exe().map_err(|e| format!("locating graph-cli: {e}"))?;
    let wasm = build_wasm(&[])?;
    let count = seeds.to_string();
    let native = || run_lines(Command::new(&exe).args(["hashgate-arm", "--seeds", &count]));
    let wasm32 = || run_lines(node_harness(&wasm).args(["hash", &count]).args(stages()));
    let arms = vec![
        ("native run 1", native()?),
        ("native run 2", native()?),
        ("wasm32 run 1", wasm32()?),
        ("wasm32 run 2", wasm32()?),
    ];
    println!(
        "hashgate: wasm artifact {} sha256 {}",
        wasm.display(),
        file_sha256(&wasm)?
    );
    Ok(arms)
}

fn report(stamp: &evidence::Stamp, control: Option<Knob>, seeds: u32, arms: &[Arm]) -> ExitCode {
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
    report::arm_report(&mut detail, arms, &lines);
    print!("{detail}");
    let tally = per_stage(seeds, stages().len(), &lines);
    for (stage, equal) in stages().iter().zip(&tally.equal) {
        println!("  {stage}: 4-way equal on {equal}/{seeds} seeds");
    }
    conclude(stamp, control, seeds, &tally, arms)
}

/// The C20 tally, the record, and the exit code: the last of the gate's work, split out
/// of [`report`] by the house's 40-line-per-function limit.
fn conclude(
    stamp: &evidence::Stamp,
    control: Option<Knob>,
    seeds: u32,
    tally: &Tally,
    arms: &[Arm],
) -> ExitCode {
    // C20, counted from the wasm arm's own lines (run 1; run 2 is 4-way equal to it):
    // the real ABI's snapshot against the retained shim's, per seed.
    let c20 = match transport::agree_with_shim(seeds, &arms[2].1) {
        Ok(agreed) => agreed,
        Err(err) => {
            eprintln!("hashgate: the C20 tally could not be read: {err}");
            return ExitCode::from(2);
        }
    };
    println!("  {TRANSPORT}: the real ABI matched {LAYOUT} on {c20}/{seeds} seeds");
    let bad = tally.diverged_seeds;
    println!("  4-way equal on {}/{seeds} seeds", seeds - bad);
    if let Err(err) = record(stamp, control, seeds, tally, c20) {
        eprintln!("hashgate: not recorded: {err}");
        return ExitCode::from(2);
    }
    if c20 != seeds {
        println!(
            "FAIL: {TRANSPORT} diverges from {LAYOUT} on {} seeds",
            seeds - c20
        );
        return ExitCode::from(1);
    }
    if bad == 0 {
        println!("PASS");
    } else {
        println!("FAIL: {bad} of {seeds} seeds diverge");
    }
    report::exit(bad)
}

/// Writes this run's result for the ledger: `hashgate.json` for an honest run, the
/// knob's own record for a negative control. A run that cannot record exits 2: its
/// verdict would otherwise stand with no evidence behind it. The `transport` tally goes
/// in the same record, because it *is* the hash gate's verdict — the C20 count
/// `capabilities/verdict.rs` reads for the `transport.wasm.columnar` row.
fn record(
    stamp: &evidence::Stamp,
    control: Option<Knob>,
    seeds: u32,
    tally: &Tally,
    c20: u32,
) -> Result<(), String> {
    let name = control.map_or("hashgate", Knob::record);
    evidence::write(stamp, name, report::body(control, seeds, tally, c20)).map(drop)
}

#[cfg(test)]
mod tests;
