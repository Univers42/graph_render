//! The Phase 9 measurement campaign (`prompts/phase-09-scale-bench.md` §6): build time,
//! run time, the tick it implies, settle time, memory as **two** numbers, snapshot bytes,
//! and the crossover N against a frame budget.
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
//! **The tick is derived, not measured.** The force layout is a one-shot `Stage::run` of
//! [`SETTLE_TICKS`] ticks and the wasm ABI has no per-tick entry point, so a "tick" here
//! is `run / 112`. Both JavaScript harness arms divide the same way, which is the only
//! reason three crossover cells can be compared at all — and it is stated in every report
//! that prints one.
//!
//! Time is the harness's own wall clock (`Instant`), which is exactly what D8 forbids
//! *inside the motor*: nothing in `graph-core` reads a clock, and the gate greps for
//! it. Measuring the motor is not part of the motor.
//!
//! Ponytail: the campaign is itself a sampler — one machine class, one seed, one
//! `reference_degree`. Results are not portable across hardware, and a `N` past the
//! largest size measured is an extrapolation, not a number.

pub mod arms;
pub mod report;

use graph_contract::canonical_json::to_json;
use graph_core::registry::Capability;
use graph_core::{REFERENCE_DEGREE, Topology, index_model, run_with};
use std::time::Instant;

/// Ticks a settle costs: d3's `alphaDecay(0.06)` down to its `alphaMin` of 0.001.
pub const SETTLE_TICKS: u32 = 112;

/// The frame budget `prompt.md` §5.2 states, and the gate's `--budget-ms`.
pub const FRAME_BUDGET_MS: f64 = 16.67;

/// The 33 B/node column table `prompt.md` §5.1, the row the report's columns/node is read
/// against — and the row Phase 9 measured at 3.7× too low (`docs/measurements/
/// phase09-ceilings.md`).
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
    /// The layout's one-shot run to convergence, median of `--repeat`.
    pub run_ms: f64,
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
    /// The run as one tick: `run_ms / 112`, the per-tick-equivalent every arm reports.
    pub fn tick_ms(&self) -> f64 {
        self.run_ms / f64::from(SETTLE_TICKS)
    }

    /// The whole settle: the one-shot run *is* the 112 ticks, so this is the run itself.
    pub fn settle_ms(&self) -> f64 {
        self.run_ms
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

/// A tick's milliseconds as a whole settle: `tick_ms` times [`SETTLE_TICKS`].
///
/// The conversion both directions round, and it is pinned by
/// `settle_is_the_tick_times_the_112_ticks_alpha_decay_needs`. A [`Sample`] does not go
/// through it — a one-shot run already *is* the 112 ticks, so its settle is the run.
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

/// The layout the crossover reports when no `--layout` names one: the force layout,
/// the only one whose headline number is a tick.
static DEFAULT_ARM: Capability = graph_core::registry::LAYOUTS[3];

/// Every `(n, derived tick ms)` the native arm measured, for [`largest_fitting`].
pub fn ladder(plan: &super::Plan) -> Vec<(u32, f64)> {
    let entries = super::resolve(&plan.layouts).unwrap_or_default();
    let arm = entries.first().copied().unwrap_or(&DEFAULT_ARM);
    plan.sizes
        .iter()
        .filter_map(|&n| measure(plan, arm, n).ok().map(|s| (n, s.tick_ms())))
        .collect()
}

/// The campaign: one [`Sample`] per (layout, n), in the order the plan lists them.
pub fn run(plan: &super::Plan) -> Result<Vec<(&'static str, Vec<Sample>)>, String> {
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

/// One layout at one size: build, run, the two memory numbers, the two snapshot sizes.
fn measure(plan: &super::Plan, entry: &Capability, n: u32) -> Result<Sample, String> {
    let (nodes, edges) = super::scale::scale_model(plan.seed, n, REFERENCE_DEGREE);
    let build_ms = time_repeated(plan.repeat, || {
        index_model(&nodes, &edges)
            .map(|_| ())
            .map_err(|e| format!("n={n}: {e}"))
    })?;
    let topology = index_model(&nodes, &edges).map_err(|e| format!("n={n}: {e}"))?;
    let run_ms = time_repeated(plan.repeat, || {
        (entry.run)(&topology)
            .map(|_| ())
            .map_err(|e| format!("n={n}: {e}"))
    })?;
    let (bin, json) = snapshot_bytes(entry, &nodes, &edges)?;
    Ok(Sample {
        n,
        edges: topology.edge_count(),
        build_ms,
        run_ms,
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
