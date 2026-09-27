//! The 4-way hash gate (`prompt.md` §7.1): for every stage and seed, native run 1,
//! native run 2, wasm32 run 1 and wasm32 run 2 must produce the same SHA-256 (D7) —
//! cross-target and run-to-run in one check. Each run is its own process, so nothing (an
//! allocator address, a hash seed) can leak from one run into the next.
//!
//! The wasm arm is the real `graph_wasm.wasm` driven by `harness/wasm-run.mjs` under
//! Node, which hashes with its built-in crypto: two independent SHA-256
//! implementations, so a broken hasher cannot agree with itself and pass.
//!
//! An honest run records its result in `target/gates/hashgate.json` and a negative
//! control in `hashgate-control.json`, for the capabilities ledger to read.

mod compare;

use crate::evidence;
use crate::runner::{build_wasm, file_sha256, node_harness, run_lines, sha256_hex};
use compare::{Arm, diverged, per_stage};
use serde_json::json;
use std::process::{Command, ExitCode};

/// Every stage the gate hashes, in the order both arms print them.
pub const STAGES: [&str; 2] = ["synthetic", "topology"];
/// Set by the negative control (`prompt.md` §7.2).
const MUTATE_ENV: &str = "GM_MUTATE_REFERENCE_DEGREE";

/// Runs all four arms over seeds `0..seeds` and compares them line by line.
pub fn run(seeds: u32) -> ExitCode {
    match collect_arms(seeds) {
        Ok(arms) => report(seeds, &arms),
        Err(err) => {
            eprintln!("hashgate: could not run: {err}");
            ExitCode::from(2)
        }
    }
}

/// Body of the hidden `hashgate-arm` subcommand: one native run, every stage.
pub fn arm(seeds: u32) -> ExitCode {
    let degree = match reference_degree() {
        Ok(degree) => degree,
        Err(err) => {
            eprintln!("hashgate-arm: {err}");
            return ExitCode::from(2);
        }
    };
    let mut lines = String::new();
    for stage in STAGES {
        for seed in 0..seeds {
            match stage_bytes(stage, seed, degree) {
                Ok(bytes) => lines.push_str(&format!("{stage} {seed} {}\n", sha256_hex(&bytes))),
                Err(err) => {
                    eprintln!("hashgate-arm: {stage} seed {seed}: {err}");
                    return ExitCode::from(2);
                }
            }
        }
    }
    print!("{lines}");
    ExitCode::SUCCESS
}

fn stage_bytes(stage: &str, seed: u32, degree: u32) -> Result<Vec<u8>, String> {
    match stage {
        "synthetic" => graph_core::synthetic_snapshot(seed, degree).map_err(|e| e.to_string()),
        "topology" => graph_core::topology_stage(seed, degree).map_err(|e| e.to_string()),
        other => Err(format!("unknown stage {other}")),
    }
}

/// The native arm's reference degree. The negative control overrides it here and only
/// here: the wasm arm runs the compiled-in constant and never sees the variable, so a
/// wired mutation surfaces as exactly the cross-target divergence the gate must catch.
/// A set but unparseable value is an error, never a silent fallback to the constant —
/// that fallback would let a typo in the control pass as green.
fn reference_degree() -> Result<u32, String> {
    parse_reference_degree(std::env::var(MUTATE_ENV))
}

fn parse_reference_degree(value: Result<String, std::env::VarError>) -> Result<u32, String> {
    match value {
        Err(std::env::VarError::NotPresent) => Ok(graph_core::REFERENCE_DEGREE),
        Err(err) => Err(format!("{MUTATE_ENV}: {err}")),
        Ok(text) => text
            .trim()
            .parse()
            .map_err(|e| format!("{MUTATE_ENV}={text:?}: {e}")),
    }
}

fn collect_arms(seeds: u32) -> Result<Vec<Arm>, String> {
    let exe = std::env::current_exe().map_err(|e| format!("locating graph-cli: {e}"))?;
    let wasm = build_wasm(&[])?;
    let count = seeds.to_string();
    let native = || run_lines(Command::new(&exe).args(["hashgate-arm", "--seeds", &count]));
    let wasm32 = || run_lines(node_harness(&wasm).args(["hash", &count]).args(STAGES));
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

fn report(seeds: u32, arms: &[Arm]) -> ExitCode {
    println!("hashgate: stages={} seeds={seeds}", STAGES.join(","));
    let lines = match diverged(seeds, arms) {
        Ok(lines) => lines,
        Err(err) => {
            eprintln!("hashgate: arms not comparable: {err}");
            return ExitCode::from(2);
        }
    };
    for (name, output) in arms {
        let digest = sha256_hex(output.join("\n").as_bytes());
        println!("  {name:<13} digest {digest}");
    }
    for &i in lines.iter().take(3) {
        println!(
            "  DIVERGED {}:",
            arms[0].1[i].rsplit_once(' ').map_or("", |p| p.0)
        );
        for (name, output) in arms {
            println!("    {name:<13} {}", output[i]);
        }
    }
    let stages = per_stage(seeds, &lines);
    for (stage, equal) in STAGES.iter().zip(&stages.equal) {
        println!("  {stage}: 4-way equal on {equal}/{seeds} seeds");
    }
    let bad = stages.diverged_seeds;
    println!("  4-way equal on {}/{seeds} seeds", seeds - bad);
    record(seeds, &stages.equal, bad == 0);
    if bad == 0 {
        println!("PASS");
        ExitCode::SUCCESS
    } else {
        println!("FAIL: {bad} of {seeds} seeds diverge");
        ExitCode::from(1)
    }
}

/// Writes this run's result for the ledger: `hashgate.json` for an honest run,
/// `hashgate-control.json` for the negative control. A run that cannot record says so;
/// the ledger then finds no evidence, which is the safe direction.
fn record(seeds: u32, equal: &[u32], pass: bool) {
    let control = std::env::var_os(MUTATE_ENV).is_some();
    let name = if control {
        "hashgate-control"
    } else {
        "hashgate"
    };
    let stages: serde_json::Map<_, _> = STAGES
        .iter()
        .zip(equal)
        .map(|(stage, equal)| ((*stage).to_owned(), json!(equal)))
        .collect();
    let body = json!({ "seeds": seeds, "pass": pass, "equal": stages });
    if let Err(err) = evidence::write(name, body) {
        eprintln!("hashgate: not recorded: {err}");
    }
}

#[cfg(test)]
mod tests;
