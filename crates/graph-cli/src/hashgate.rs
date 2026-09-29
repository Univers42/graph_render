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
mod knob;
mod report;

use crate::evidence;
use crate::runner::{build_wasm, file_sha256, node_harness, run_lines, sha256_hex};
use compare::{Arm, Tally, diverged, per_stage};
use graph_core::layout::force::BarnesHut;
use graph_core::layout::forceatlas2::ForceAtlas2;
use graph_core::{
    EdgeRecord, Geometry, Grid, NodeRecord, Stage, StageError, Sugiyama, Topology, gate_node_count,
    registry, run_pipeline, run_with, seeded_model,
};
pub use knob::Knob;
use knob::{Setting, env_setting};
use std::process::{Command, ExitCode};

/// Every stage the gate hashes, in the order both arms print them: the topology, then
/// every layout of `graph_core::registry::LAYOUTS`. Kept as a literal list rather than
/// built from `LAYOUTS` at runtime so its length is usable as an array size below;
/// [`tests::the_stages_are_the_topology_then_every_registered_layout`] is the guard that
/// keeps it honest as the registry grows.
pub const STAGES: [&str; 11] = [
    "topology",
    "layout.grid",
    "layout.tree.tidy",
    "layout.treemap.squarified",
    "layout.circular.radial",
    "layout.packing.circle",
    "layout.spectral",
    "layout.mds.pivot",
    "layout.force.barnes_hut",
    "layout.forceatlas2",
    "layout.dag.sugiyama",
];

/// `STAGES.len()`, named for the fixed-size arrays it sizes.
const STAGE_COUNT: usize = STAGES.len();

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

/// Every stage's id and bytes for `seed`, in pipeline order: the topology and the grid
/// from the knob-aware [`run_pipeline`] (so the spacing control still reaches it), then
/// every parameterless registered layout via [`run_with`], then the layered drawing, again
/// through [`run_pipeline`] so its layer-spacing control reaches it — all on the same nodes
/// and edges.
fn stage_bytes(
    seed: u32,
    setting: &Setting,
) -> Result<[(&'static str, Vec<u8>); STAGE_COUNT], String> {
    let count = gate_node_count(seed) + setting.extra_nodes;
    let (nodes, edges) = seeded_model(seed, count, setting.reference_degree);
    let grid = run_pipeline::<Grid>(&nodes, &edges, &setting.grid).map_err(|e| e.to_string())?;
    let mut out: [(&'static str, Vec<u8>); STAGE_COUNT] = STAGES.map(|id| (id, Vec::new()));
    out[0].1 = grid.topology;
    out[1].1 = grid.snapshot.to_bytes();
    let last = STAGE_COUNT - 1;
    for slot in &mut out[2..last] {
        slot.1 = match slot.0 {
            BarnesHut::ID => run_force(&nodes, &edges, |t| BarnesHut::run(t, &setting.force))?,
            ForceAtlas2::ID => run_force(&nodes, &edges, |t| ForceAtlas2::run(t, &setting.fa2))?,
            id => {
                let layout = registry::find(id).ok_or_else(|| format!("{id}: not registered"))?;
                let run =
                    run_with(&nodes, &edges, layout.id, layout.run).map_err(|e| e.to_string())?;
                run.snapshot.to_bytes()
            }
        };
    }
    let dag = run_pipeline::<Sugiyama>(&nodes, &edges, &setting.sugiyama);
    out[last].1 = dag.map_err(|e| e.to_string())?.snapshot.to_bytes();
    Ok(out)
}

/// A force stage run at the knob-aware parameters rather than the registry's compiled-in
/// default, so [`Knob::ForceTheta`] and [`Knob::Fa2ScalingRatio`] can reach them. The
/// wasm arm always runs the compiled-in default (it cannot see these variables), which
/// is exactly the divergence a wired control must surface.
fn run_force(
    nodes: &[NodeRecord],
    edges: &[EdgeRecord],
    layout: impl FnOnce(&Topology) -> Result<Geometry, StageError>,
) -> Result<Vec<u8>, String> {
    let run = run_with(nodes, edges, "", layout).map_err(|e| e.to_string())?;
    Ok(run.snapshot.to_bytes())
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
    let lines = match diverged(seeds, &STAGES, arms) {
        Ok(lines) => lines,
        Err(err) => {
            eprintln!("hashgate: arms not comparable: {err}");
            return ExitCode::from(2);
        }
    };
    let mut detail = String::new();
    report::arm_report(&mut detail, arms, &lines);
    print!("{detail}");
    let stages = per_stage(seeds, STAGES.len(), &lines);
    for (stage, equal) in STAGES.iter().zip(&stages.equal) {
        println!("  {stage}: 4-way equal on {equal}/{seeds} seeds");
    }
    let bad = stages.diverged_seeds;
    println!("  4-way equal on {}/{seeds} seeds", seeds - bad);
    if let Err(err) = record(stamp, control, seeds, &stages) {
        eprintln!("hashgate: not recorded: {err}");
        return ExitCode::from(2);
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
/// verdict would otherwise stand with no evidence behind it.
fn record(
    stamp: &evidence::Stamp,
    control: Option<Knob>,
    seeds: u32,
    tally: &Tally,
) -> Result<(), String> {
    let name = control.map_or("hashgate", Knob::record);
    evidence::write(stamp, name, report::body(control, seeds, tally)).map(drop)
}

#[cfg(test)]
mod tests;
