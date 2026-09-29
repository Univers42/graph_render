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
mod report;

use crate::evidence;
use crate::runner::{build_wasm, file_sha256, node_harness, run_lines, sha256_hex};
use compare::{Arm, Tally, diverged, per_stage};
use graph_core::{
    Grid, GridParams, REFERENCE_DEGREE, Sugiyama, SugiyamaParams, gate_node_count, registry,
    run_pipeline, run_with, seeded_model,
};
use std::env::VarError;
use std::process::{Command, ExitCode};

/// Every stage the gate hashes, in the order both arms print them: the topology, then
/// every layout of `graph_core::registry::LAYOUTS`. Kept as a literal list rather than
/// built from `LAYOUTS` at runtime so its length is usable as an array size below;
/// [`tests::the_stages_are_the_topology_then_every_registered_layout`] is the guard that
/// keeps it honest as the registry grows.
pub const STAGES: [&str; 7] = [
    "topology",
    "layout.grid",
    "layout.tree.tidy",
    "layout.treemap.squarified",
    "layout.circular.radial",
    "layout.packing.circle",
    "layout.dag.sugiyama",
];

/// `STAGES.len()`, named for the fixed-size arrays it sizes.
const STAGE_COUNT: usize = STAGES.len();

/// A negative control (`prompt.md` §7.2): a variable that perturbs the native arm only,
/// so a wired mutation surfaces as exactly the cross-target divergence the gate must
/// catch. The grid ignores weights, so the reference degree cannot reach `layout.grid`,
/// and the grid's spacing is what backs that stage. Treemap reads node weight, so the
/// reference degree backs it too. Tidy tree, circular and packing take no parameters
/// (`layout::tidy_tree`/`circular` are pinned with none, and adding one to gain a knob
/// would be the tail wagging the dog) and read only the topology, so [`Knob::NodeCount`]
/// perturbs that instead: one more node changes every stage that is a function of the
/// topology at all, backing every stage no other knob reaches. The layered drawing
/// ignores weights too; its layer spacing backs `layout.dag.sugiyama`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Knob {
    /// `GM_MUTATE_REFERENCE_DEGREE`: the degree the topology's weights are taken against.
    ReferenceDegree,
    /// `GM_MUTATE_GRID_SPACING`: the grid's spacing.
    GridSpacing,
    /// `GM_MUTATE_SUGIYAMA_LAYER_SPACING`: the layered drawing's Y step per layer.
    SugiyamaLayerSpacing,
    /// `GM_MUTATE_NODE_COUNT`: nodes added to the model, native arm only.
    NodeCount,
}

impl Knob {
    /// Every knob.
    pub const ALL: [Self; 4] = [
        Self::ReferenceDegree,
        Self::GridSpacing,
        Self::SugiyamaLayerSpacing,
        Self::NodeCount,
    ];

    /// The variable that sets it.
    pub const fn env(self) -> &'static str {
        match self {
            Self::ReferenceDegree => "GM_MUTATE_REFERENCE_DEGREE",
            Self::GridSpacing => "GM_MUTATE_GRID_SPACING",
            Self::SugiyamaLayerSpacing => "GM_MUTATE_SUGIYAMA_LAYER_SPACING",
            Self::NodeCount => "GM_MUTATE_NODE_COUNT",
        }
    }

    /// The record its run writes.
    pub const fn record(self) -> &'static str {
        match self {
            Self::ReferenceDegree => "hashgate-control-reference-degree",
            Self::GridSpacing => "hashgate-control-grid-spacing",
            Self::SugiyamaLayerSpacing => "hashgate-control-sugiyama-layer-spacing",
            Self::NodeCount => "hashgate-control-node-count",
        }
    }
}

/// What the native arm runs with: the compiled-in defaults, or one knob's perturbation.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Setting {
    reference_degree: u32,
    grid: GridParams,
    sugiyama: SugiyamaParams,
    /// Extra nodes added to the gate's model, native arm only ([`Knob::NodeCount`]).
    extra_nodes: u32,
    control: Option<Knob>,
}

/// Reads the knobs through `read`. At most one may be set, and a set one must parse:
/// a typo falling back to the default would let the control pass as green. A spacing the
/// grid refuses is left for the grid to refuse, so the rule lives in one place.
fn setting(read: impl Fn(&str) -> Result<String, VarError>) -> Result<Setting, String> {
    let mut setting = Setting {
        reference_degree: REFERENCE_DEGREE,
        grid: GridParams::default(),
        sugiyama: SugiyamaParams::default(),
        extra_nodes: 0,
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
            Knob::SugiyamaLayerSpacing => {
                setting.sugiyama.layer_spacing = text.trim().parse().map_err(|e| bad(&e))?;
            }
            Knob::NodeCount => setting.extra_nodes = text.trim().parse().map_err(|e| bad(&e))?,
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
        let layout = registry::find(slot.0).ok_or_else(|| format!("{}: not registered", slot.0))?;
        let run = run_with(&nodes, &edges, layout.id, layout.run).map_err(|e| e.to_string())?;
        slot.1 = run.snapshot.to_bytes();
    }
    let dag = run_pipeline::<Sugiyama>(&nodes, &edges, &setting.sugiyama);
    out[last].1 = dag.map_err(|e| e.to_string())?.snapshot.to_bytes();
    Ok(out)
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
