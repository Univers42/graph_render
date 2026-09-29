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
mod tier;
mod transport;

use crate::evidence;
use crate::runner::{build_wasm, file_sha256, node_harness, run_lines, sha256_hex};
use compare::{Arm, Tally, diverged, per_stage};
use graph_core::Stage;
use graph_core::layout::force::BarnesHut;
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

pub(crate) use crate::exec_native::Threads;
pub(crate) use tier::Tiers;
/// `--tiers` as `main.rs`'s flag parser reads it: the arm list's own [`tier::parse`], so
/// the accepted words and the arms they add are one definition.
pub(crate) fn parse_tiers(text: &str) -> Result<Tiers, String> {
    tier::parse(text)
}

/// Runs every arm over seeds `0..seeds` and compares them line by line.
pub fn run(seeds: u32, tiers: Tiers) -> ExitCode {
    let started = env_setting().and_then(|setting| {
        let stamp = evidence::Stamp::take()?;
        Ok((setting.control, stamp, collect_arms(seeds, tiers)?))
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

/// `stage seed sha256` lines for a Barnes-Hut stage run over `workers` `std::thread`s, and
/// **every other stage byte-identical to the serial arm's**.
///
/// The stages are the gate's own registry list, in its own order, so the comparison is
/// line-for-line against the scalar arm. The force stage is computed by
/// `BarnesHut::run_with(..., &Threads, workers)` — the same call the scalar arm reaches
/// through `Stage::run` with one worker, so the arm checks a *schedule* rather than a
/// second implementation.
/// **Stage-major, seed-minor**, exactly as [`arm_lines`] prints them: every seed of one
/// stage, then every seed of the next. That ordering is not cosmetic — `compare::diverged`
/// reads line `i` as stage `i / seeds`, seed `i % seeds`, so a seed-major arm would be
/// refused as malformed at its first line. The two functions must keep the same shape;
/// `the_threaded_arm_prints_its_stages_in_the_same_order_as_the_scalar_one` holds them to
/// it.
fn threads_lines(seeds: u32, setting: &Setting, workers: u32) -> Result<Vec<String>, String> {
    let mut blocks = vec![String::new(); stages().len()];
    for seed in 0..seeds {
        let bytes = stage_bytes_threaded(seed, setting, workers)
            .map_err(|err| format!("seed {seed}: {err}"))?;
        for (block, (id, bytes)) in blocks.iter_mut().zip(bytes) {
            block.push_str(&format!("{id} {seed} {}\n", sha256_hex(&bytes)));
        }
    }
    Ok(blocks.concat().lines().map(str::to_owned).collect())
}

/// [`stage_bytes`](stages::stage_bytes) with the Barnes-Hut stage run threaded, and with
/// `setting.split_sum` — the negative control — carried into it.
///
/// The control reaches the *stage*, not just the arm, so `GM_MUTATE_SPLIT_SUM=1 --tiers all`
/// diverges the threaded arms from the scalar one on the force stage and nowhere else. That
/// is the shape the phase prompt asks the control to have: a mutation a threaded arm cannot
/// survive, so the gate's red is proof the arms were compared.
fn stage_bytes_threaded(
    seed: u32,
    setting: &Setting,
    workers: u32,
) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    let count = graph_core::gate_node_count(seed) + setting.extra_nodes;
    let (nodes, edges) = graph_core::seeded_model(seed, count, setting.reference_degree);
    let topology = graph_core::index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (id, bytes) in stages::stage_bytes(seed, setting)? {
        let bytes = if id == BarnesHut::ID {
            let geometry = BarnesHut::run_under(
                &topology,
                &setting.force,
                &Threads,
                workers,
                setting.split_sum,
            )
            .map_err(|e| e.to_string())?;
            graph_core::layout::snapshot(&topology, geometry)
                .map(|snapshot| snapshot.to_bytes())
                .map_err(|e| e.to_string())?
        } else {
            bytes
        };
        out.push((id, bytes));
    }
    Ok(out)
}

fn collect_arms(seeds: u32, tiers: Tiers) -> Result<Vec<Arm>, String> {
    let exe = std::env::current_exe().map_err(|e| format!("locating graph-cli: {e}"))?;
    let wasm = build_wasm(&[])?;
    let count = seeds.to_string();
    let native = || run_lines(Command::new(&exe).args(["hashgate-arm", "--seeds", &count]));
    let wasm32 = || run_lines(node_harness(&wasm).args(["hash", &count]).args(stages()));
    let mut arms = vec![
        ("native run 1", native()?),
        ("native run 2", native()?),
        ("wasm32 run 1", wasm32()?),
        ("wasm32 run 2", wasm32()?),
    ];
    arms.extend(tier::arms(seeds, tiers)?);
    println!("hashgate: {} arms, tiers {}", arms.len(), tiers.as_str());
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
    let ways = arms.len();
    for (stage, equal) in stages().iter().zip(&tally.equal) {
        println!("  {stage}: {ways}-way equal on {equal}/{seeds} seeds");
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
    let ways = arms.len();
    println!("  {ways}-way equal on {}/{seeds} seeds", seeds - bad);
    if let Err(err) = record(stamp, control, seeds, tally, c20, arms) {
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
    arms: &[Arm],
) -> Result<(), String> {
    let name = control.map_or("hashgate", Knob::record);
    evidence::write(stamp, name, report::body(control, seeds, tally, c20, arms)).map(drop)
}

#[cfg(test)]
mod tests;
