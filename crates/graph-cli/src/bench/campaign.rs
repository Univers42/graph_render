//! The Phase 9 measurement campaign (`prompts/phase-09-scale-bench.md` §6): build time,
//! tick time, settle time, memory as **two** numbers, snapshot bytes, and the crossover
//! N against a frame budget.
//!
//! The campaign is the phase's justification, so it is held to two rules the phase
//! prompt states and the older `bench` rows do not:
//!
//! - **A median over `--repeat` runs, never one timing.** A single wall-clock number is
//!   noise; a loaded host inflates it. `median` is pinned by a test, including the even
//!   case, so "which element of the sorted run" cannot drift silently.
//! - **Memory is reported as columns and arena separately, never as one total.** The
//!   arena is data-dependent and unbounded, so a single total hides which half grew.
//!
//! Time is the harness's own wall clock (`Instant`), which is exactly what D8 forbids
//! *inside the motor*: nothing in `graph-core` reads a clock, and the gate greps for
//! it. Measuring the motor is not part of the motor.
//!
//! Ponytail: the campaign is itself a sampler — one machine class, one seed, one
//! `reference_degree`. Results are not portable across hardware, and a `N` past the
//! largest size measured is an extrapolation, not a number.

use super::Plan;
use graph_contract::canonical_json::to_json;
use graph_core::registry::Capability;
use graph_core::{REFERENCE_DEGREE, Topology, index_model, run_with, seeded_model};
use std::path::Path;
use std::time::Instant;

/// Ticks a settle costs: d3's `alphaDecay(0.06)` down to its `alphaMin` of 0.001.
pub const SETTLE_TICKS: u32 = 112;

/// The frame budget `prompt.md` §5.2 states, and the gate's `--budget-ms`.
pub use self::budget::FRAME_BUDGET_MS;

mod budget {
    /// The frame budget `prompt.md` §5.2 states, and the gate's `--budget-ms`.
    pub const FRAME_BUDGET_MS: f64 = 16.67;
}

/// The 33 B/node column table `prompt.md` §5.1, the row the report's columns/node
/// is read against.
pub const COLUMN_TABLE_B: u64 = 33;

/// One measured (layout, n).
#[derive(Debug, Clone)]
pub struct Sample {
    /// Nodes in the synthetic model.
    pub n: u32,
    /// Edges kept after indexing.
    pub edges: u32,
    /// Ingest to indexed topology, median of `--repeat`.
    pub build_ms: f64,
    /// One layout run over the finished topology, median of `--repeat`.
    pub tick_ms: f64,
    /// Node and edge columns plus the three CSRs, excluding the string arena.
    pub columns: u64,
    /// The string arena alone, which is the unbounded half.
    pub arena: u64,
    /// Binary snapshot bytes.
    pub bin: u64,
    /// Canonical JSON snapshot bytes.
    pub json: u64,
    /// Past the layout's registered `scale_ceiling`, so the row is labelled.
    pub past_ceiling: bool,
}

impl Sample {
    /// `tick_ms` times [`SETTLE_TICKS`].
    pub fn settle_ms(&self) -> f64 {
        settle_ms(self.tick_ms)
    }

    /// Columns per node: the number to read against the 33 B/node table.
    pub fn columns_per_node(&self) -> f64 {
        if self.n == 0 {
            return 0.0;
        }
        self.columns as f64 / f64::from(self.n)
    }
}

/// The median of `samples`, pinned to the mean of the two central values when even.
pub fn median(mut samples: Vec<f64>) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples.sort_by(f64::total_cmp);
    let mid = samples.len() / 2;
    if samples.len() % 2 == 1 {
        return samples[mid];
    }
    (samples[mid - 1] + samples[mid]) / 2.0
}

/// `tick_ms` times [`SETTLE_TICKS`].
pub fn settle_ms(tick_ms: f64) -> f64 {
    tick_ms * f64::from(SETTLE_TICKS)
}

/// The largest `(n, ms)` whose ms fits `budget_ms`, or `None` if none does.
///
/// Ties are broken by the larger `n` and the input is walked in `n` order, so the
/// answer does not depend on the order the ladder was measured in.
pub fn largest_fitting(samples: &[(u32, f64)], budget_ms: f64) -> Option<u32> {
    let mut ordered: Vec<(u32, f64)> = samples.to_vec();
    ordered.sort_by_key(|(n, _)| *n);
    ordered
        .iter()
        .filter(|(_, ms)| *ms <= budget_ms)
        .map(|(n, _)| *n)
        .next_back()
}

/// Every `(n, median tick ms)` an arm measured, for [`largest_fitting`].
pub fn ladder(plan: &Plan) -> Vec<(u32, f64)> {
    let entries = super::resolve(&plan.layouts).unwrap_or_default();
    let arm = entries.first().copied().unwrap_or(&DEFAULT_ARM);
    plan.sizes
        .iter()
        .filter_map(|&n| measure(plan, arm, n).ok().map(|s| (n, s.tick_ms)))
        .collect()
}

/// The layout the crossover reports when no `--layout` names one: the force layout,
/// the only one whose headline number is a tick.
static DEFAULT_ARM: Capability = graph_core::registry::LAYOUTS[3];

/// The campaign: one [`Sample`] per (layout, n), in the order the plan lists them.
pub fn run(plan: &Plan) -> Result<Vec<(&'static str, Vec<Sample>)>, String> {
    let entries = super::resolve(&plan.layouts)?;
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let mut samples = Vec::with_capacity(plan.sizes.len());
        for &n in &plan.sizes {
            samples.push(measure(plan, entry, n)?);
        }
        out.push((entry.id, samples));
    }
    Ok(out)
}

/// One layout at one size: build, tick, the two memory numbers, the two snapshot sizes.
fn measure(plan: &Plan, entry: &Capability, n: u32) -> Result<Sample, String> {
    let (nodes, edges) = seeded_model(plan.seed, n, REFERENCE_DEGREE);
    let build_ms = time_repeated(plan.repeat, || {
        index_model(&nodes, &edges)
            .map(|_| ())
            .map_err(|e| format!("n={n}: {e}"))
    })?;
    let topology = index_model(&nodes, &edges).map_err(|e| format!("n={n}: {e}"))?;
    let tick_ms = time_repeated(plan.repeat, || {
        (entry.run)(&topology)
            .map(|_| ())
            .map_err(|e| format!("n={n}: {e}"))
    })?;
    let (bin, json) = snapshot_bytes(entry, &nodes, &edges)?;
    Ok(Sample {
        n,
        edges: topology.edge_count(),
        build_ms,
        tick_ms,
        columns: columns_bytes(&topology),
        arena: topology.strings().byte_len() as u64,
        bin,
        json,
        past_ceiling: u64::from(n) > entry.meta.scale_ceiling,
    })
}

/// `repeat` runs of `f`, its median wall milliseconds. One run is one run, not zero.
fn time_repeated<F: FnMut() -> Result<(), String>>(repeat: u32, mut f: F) -> Result<f64, String> {
    let runs = repeat.max(1);
    let mut samples = Vec::with_capacity(runs as usize);
    for _ in 0..runs {
        let started = Instant::now();
        f()?;
        samples.push(started.elapsed().as_secs_f64() * 1e3);
    }
    Ok(median(samples))
}

/// Columns and CSRs, excluding the arena: the number the 33 B/node table is about.
fn columns_bytes(topology: &Topology) -> u64 {
    let csrs =
        topology.out().byte_len() + topology.inbound().byte_len() + topology.hierarchy().byte_len();
    (topology.nodes().byte_len() + topology.edges().byte_len() + csrs) as u64
}

/// The snapshot's two faces, sized. The layout runs again here, untimed.
fn snapshot_bytes(
    entry: &Capability,
    nodes: &[graph_core::NodeRecord],
    edges: &[graph_core::EdgeRecord],
) -> Result<(u64, u64), String> {
    let id = entry.id;
    let run = run_with(nodes, edges, id, entry.run).map_err(|e| format!("{id}: {e}"))?;
    Ok((
        run.snapshot.to_bytes().len() as u64,
        to_json(&run.snapshot).len() as u64,
    ))
}

/// The campaign's markdown: every row, both memory halves, and the ceiling labels.
pub fn markdown(plan: &Plan, rows: &[(&'static str, Vec<Sample>)], budget_ms: f64) -> String {
    let mut out = String::from(
        "# Phase 9 bench — native arm\n\n\
         Generated by `graph-cli bench --repeat` (prompts/phase-09-scale-bench.md §6). \
         Medians over the stated repeat count; a single timing is noise. Settle is tick × \
         112 (d3 `alphaDecay(0.06)` to `alphaMin` 0.001). Memory is columns and arena \
         **separately** — the arena is data-dependent and unbounded, so one total would \
         hide which half grew. Past a registered `scale_ceiling` rows are labelled.\n\n",
    );
    out += &format!(
        "seed {} · repeat {} · budget {budget_ms} ms · native, one machine class\n\n",
        plan.seed, plan.repeat
    );
    for (id, samples) in rows {
        out += &format!("## {id}\n\n");
        out += "| n | edges | build ms | tick ms | settle ms | columns B | arena B | bin B | json B |\n";
        out += "|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n";
        for s in samples {
            let label = if s.past_ceiling {
                " (past ceiling)"
            } else {
                ""
            };
            out += &format!(
                "| {}{label} | {} | {:.2} | {:.3} | {:.1} | {} | {} | {} | {} |\n",
                s.n,
                s.edges,
                s.build_ms,
                s.tick_ms,
                s.settle_ms(),
                s.columns,
                s.arena,
                s.bin,
                s.json
            );
        }
        let per_node: Vec<f64> = samples.iter().map(Sample::columns_per_node).collect();
        out += &format!(
            "\ncolumns/node {per_node:?} against the {COLUMN_TABLE_B} B/node table \
             (`prompt.md` §5.1)\n\n"
        );
    }
    out
}

/// The crossover: the largest `N` whose tick still fits the budget, per arm.
pub fn crossover_markdown(plan: &Plan, budget_ms: f64) -> String {
    let samples = ladder(plan);
    let mut out = String::from(
        "# Phase 9 crossover — largest N per arm that fits the frame budget\n\n\
         The headline deliverable is this number, not a pass or fail. Rust is not faster \
         than the TypeScript oracle at any N is a stop-and-ask, and WASM losing at N=220 is \
         a row, not a verdict.\n\n",
    );
    out += &format!(
        "| arm | largest N fitting {budget_ms} ms | ladder (n, median tick ms) |\n|---|---:|---|\n"
    );
    out += &format!(
        "| native (graph-core, this process) | {} | {samples:?} |\n",
        largest_fitting(&samples, budget_ms).map_or("none".into(), |n| n.to_string())
    );
    for (arm, why) in UNMEASURED_ARMS {
        out += &format!("| {arm} | not measured | {why} |\n");
    }
    out.push_str(
        "\nMachine class: whatever `graph-cli bench` ran on. Not portable: the harness is a \
         sampler of one machine, one seed, one reference degree.\n",
    );
    out
}

/// Arms this slice does not measure, and the reason each is empty rather than zero.
const UNMEASURED_ARMS: [(&str, &str); 2] = [
    (
        "wasm32",
        "needs Phase 4's real WASM ABI (harness/wasm-run.mjs) — p4 is not merged",
    ),
    (
        "TypeScript oracle",
        "harness/oracle-tick-bench.mjs is not in this slice",
    ),
];

/// The campaign's entry point: measure, print a row per cell, and write the report the
/// plan asks for. `true` unless a run inside its ceiling failed.
pub fn report(plan: &Plan) -> Result<bool, String> {
    if plan.crossover {
        return report_crossover(plan);
    }
    let rows = run(plan)?;
    for (id, samples) in &rows {
        for s in samples {
            let label = if s.past_ceiling {
                " (past ceiling)"
            } else {
                ""
            };
            println!(
                "{id} n={}{label} build {:.2} ms  tick {:.3} ms  settle {:.1} ms  columns {}  arena {}  bin {}  json {}",
                s.n,
                s.build_ms,
                s.tick_ms,
                s.settle_ms(),
                s.columns,
                s.arena,
                s.bin,
                s.json
            );
        }
    }
    if let Some(path) = &plan.out {
        write_report(path, &markdown(plan, &rows, plan.budget_ms))?;
    }
    Ok(true)
}

/// `--crossover`: the largest N per arm, to `--out` or to standard output.
fn report_crossover(plan: &Plan) -> Result<bool, String> {
    let text = crossover_markdown(plan, plan.budget_ms);
    match &plan.out {
        Some(path) => write_report(path, &text).map(|()| true),
        None => {
            print!("{text}");
            Ok(true)
        }
    }
}

/// Writes `text` to `path`, or returns the refusal.
pub fn write_report(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}
