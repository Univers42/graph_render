//! The 4-way hash gate (`prompt.md` §7.1): for every stage and seed, native run 1,
//! native run 2, wasm32 run 1 and wasm32 run 2 must produce the same SHA-256 (D7) —
//! cross-target and run-to-run in one check. Each run is its own process, so nothing (an
//! allocator address, a hash seed) can leak from one run into the next.
//!
//! The stages are the pipeline's (`graph_core::run_pipeline`): the topology, then every
//! registered layout, each hashed on its own, so a divergence names the stage it began
//! in. The wasm arm is the real `graph_wasm.wasm` driven by `harness/wasm-run.mjs` under
//! Node, which hashes with its built-in crypto: two independent SHA-256
//! implementations, so a broken hasher cannot agree with itself and pass.
//!
//! An honest run records its result in `target/gates/hashgate.json`, and a negative
//! control (one [`Knob`] set) in that knob's own record, for the capabilities ledger.

mod compare;

use crate::evidence;
use crate::runner::{build_wasm, file_sha256, node_harness, run_lines, sha256_hex};
use compare::{Arm, diverged, per_stage};
use graph_core::{Grid, GridParams, REFERENCE_DEGREE, gate_node_count, run_pipeline, seeded_model};
use serde_json::json;
use std::env::VarError;
use std::process::{Command, ExitCode};

/// Every stage the gate hashes, in the order both arms print them: the topology, then
/// every layout of `graph_core::registry::LAYOUTS`.
pub const STAGES: [&str; 2] = ["topology", "layout.grid"];

/// A negative control (`prompt.md` §7.2): a variable that perturbs the native arm only,
/// so a wired mutation surfaces as exactly the cross-target divergence the gate must
/// catch. Each moves one stage's input: the grid ignores weights, so the reference
/// degree cannot reach `layout.grid`, and the grid's spacing is what backs that stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Knob {
    /// `GM_MUTATE_REFERENCE_DEGREE`: the degree the topology's weights are taken against.
    ReferenceDegree,
    /// `GM_MUTATE_GRID_SPACING`: the grid's spacing.
    GridSpacing,
}

impl Knob {
    /// Every knob.
    pub const ALL: [Self; 2] = [Self::ReferenceDegree, Self::GridSpacing];

    /// The variable that sets it.
    pub const fn env(self) -> &'static str {
        match self {
            Self::ReferenceDegree => "GM_MUTATE_REFERENCE_DEGREE",
            Self::GridSpacing => "GM_MUTATE_GRID_SPACING",
        }
    }

    /// The record its run writes.
    pub const fn record(self) -> &'static str {
        match self {
            Self::ReferenceDegree => "hashgate-control-reference-degree",
            Self::GridSpacing => "hashgate-control-grid-spacing",
        }
    }
}

/// What the native arm runs with: the compiled-in defaults, or one knob's perturbation.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Setting {
    reference_degree: u32,
    grid: GridParams,
    control: Option<Knob>,
}

/// Reads the knobs through `read`. At most one may be set, and a set one must parse:
/// a typo falling back to the default would let the control pass as green. A spacing the
/// grid refuses is left for the grid to refuse, so the rule lives in one place.
fn setting(read: impl Fn(&str) -> Result<String, VarError>) -> Result<Setting, String> {
    let mut setting = Setting {
        reference_degree: REFERENCE_DEGREE,
        grid: GridParams::default(),
        control: None,
    };
    for knob in Knob::ALL {
        let text = match read(knob.env()) {
            Err(VarError::NotPresent) => continue,
            Err(err) => return Err(format!("{}: {err}", knob.env())),
            Ok(text) => text,
        };
        if let Some(other) = setting.control {
            let (a, b) = (other.env(), knob.env());
            return Err(format!("{a} and {b} are both set: one control at a time"));
        }
        setting.control = Some(knob);
        let bad = |e: &dyn std::fmt::Display| format!("{}={text:?}: {e}", knob.env());
        match knob {
            Knob::ReferenceDegree => {
                setting.reference_degree = text.trim().parse().map_err(|e| bad(&e))?;
            }
            Knob::GridSpacing => setting.grid.spacing = text.trim().parse().map_err(|e| bad(&e))?,
        }
    }
    Ok(setting)
}

fn env_setting() -> Result<Setting, String> {
    setting(|name| std::env::var(name))
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
    let mut blocks = vec![String::new(); STAGES.len()];
    for seed in 0..seeds {
        let stages = stage_bytes(seed, setting).map_err(|err| format!("seed {seed}: {err}"))?;
        for (block, (stage, bytes)) in blocks.iter_mut().zip(stages) {
            block.push_str(&format!("{stage} {seed} {}\n", sha256_hex(&bytes)));
        }
    }
    Ok(blocks.concat())
}

/// Every stage's id and bytes for `seed`, in pipeline order.
fn stage_bytes(seed: u32, setting: &Setting) -> Result<[(&'static str, Vec<u8>); 2], String> {
    let (nodes, edges) = seeded_model(seed, gate_node_count(seed), setting.reference_degree);
    let run = run_pipeline::<Grid>(&nodes, &edges, &setting.grid).map_err(|e| e.to_string())?;
    Ok(run.stages())
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

fn report(stamp: &evidence::Stamp, control: Option<Knob>, seeds: u32, arms: &[Arm]) -> ExitCode {
    let mutation = control.map_or("none", Knob::env);
    println!(
        "hashgate: stages={} seeds={seeds} control={mutation}",
        STAGES.join(",")
    );
    let lines = match diverged(seeds, arms) {
        Ok(lines) => lines,
        Err(err) => {
            eprintln!("hashgate: arms not comparable: {err}");
            return ExitCode::from(2);
        }
    };
    print_arms(arms, &lines);
    let stages = per_stage(seeds, &lines);
    for (stage, equal) in STAGES.iter().zip(&stages.equal) {
        println!("  {stage}: 4-way equal on {equal}/{seeds} seeds");
    }
    let bad = stages.diverged_seeds;
    println!("  4-way equal on {}/{seeds} seeds", seeds - bad);
    if let Err(err) = record(stamp, control, seeds, (&stages.equal, bad == 0)) {
        eprintln!("hashgate: not recorded: {err}");
        return ExitCode::from(2);
    }
    if bad == 0 {
        println!("PASS");
        ExitCode::SUCCESS
    } else {
        println!("FAIL: {bad} of {seeds} seeds diverge");
        ExitCode::from(1)
    }
}

/// Each arm's digest, then every arm's line for the first three diverging lines.
fn print_arms(arms: &[Arm], lines: &[usize]) {
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
}

/// Writes this run's result for the ledger: `hashgate.json` for an honest run, the
/// knob's own record for a negative control. A run that cannot record exits 2: its
/// verdict would otherwise stand with no evidence behind it.
fn record(
    stamp: &evidence::Stamp,
    control: Option<Knob>,
    seeds: u32,
    (equal, pass): (&[u32], bool),
) -> Result<(), String> {
    let name = control.map_or("hashgate", Knob::record);
    let stages: serde_json::Map<_, _> = STAGES
        .iter()
        .zip(equal)
        .map(|(stage, equal)| ((*stage).to_owned(), json!(equal)))
        .collect();
    let mutation = control.map(Knob::env);
    let body = json!({ "seeds": seeds, "pass": pass, "equal": stages, "mutation": mutation });
    evidence::write(stamp, name, body).map(drop)
}

#[cfg(test)]
mod tests;
