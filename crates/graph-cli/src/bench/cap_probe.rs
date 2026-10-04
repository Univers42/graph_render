//! `graph-cli cap-probe --id <id> --n <n>`: one run of one layout or post at one size, its
//! wall time and its peak resident memory, printed as one `key=value` line. The service's
//! work caps (`docs/measurements/service-caps.md`) are read off a ladder of these, one
//! process per rung (`scripts/caps-ladder.sh`), so each rung's peak is its own and a rung
//! killed by its timeout costs only that process.
//!
//! - A layout id (`graph_core::registry`) times its `run` and then both snapshot faces the
//!   service can answer with: binary bytes and canonical JSON.
//! - A post id is graph-wasm's `post::CAPABILITIES`, the table the ABI and the service
//!   expose. `--input`'s layout runs first and is not timed; the timer covers the post and
//!   both faces of the snapshot it produces.
//! - `--edges-per-node r` appends Relation edges to the seeded model until m = ⌈r·n⌉.
//!   The seeded model alone has m ≈ 1.55 n.
//! - `ms` is the whole timed window; `run_ms` is the run (or the post) alone.
//! - `peak_mib` is the process's lifetime peak: the larger of VmHWM read just before the
//!   timed window and `run_peak_mib`. It covers the model's records, the topology, the run
//!   and both faces; the records stay alive through the run, so the run cannot reuse their
//!   freed pages. `run_peak_mib` is VmHWM after a reset (`/proc/self/clear_refs` = 5)
//!   taken just before the timed window, and `base_mib` is VmRSS at that reset.
//!
//! Caveat: one run, wall clock, on whatever load the host carries; the line prints the
//! 1-minute load average so a reader can see it. VmHWM is the kernel's high-water mark, not
//! a sample, but `run_peak_mib - base_mib` under-reports a run that reuses pages the
//! model's build freed and left resident: `peak_mib` is the bound to budget with. A failed
//! reset (`hwm_reset=false`) leaves `run_peak_mib` equal to the lifetime peak.

mod densify;
mod rss;
#[cfg(test)]
mod tests;

use super::tiers::markdown::loadavg;
use graph_contract::canonical_json::to_json;
use graph_core::layout::Geometry;
use graph_core::registry::{self, Capability};
use graph_core::{REFERENCE_DEGREE, Topology, index_model, seeded_model};
use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;

pub use densify::densify;

/// The largest node count the probe builds: the 2^20 top of the service-caps ladder.
pub const MAX_PROBE_NODES: u32 = 1 << 20;

/// What one `graph-cli cap-probe` run measures.
#[derive(clap::Args)]
pub struct Plan {
    /// A layout id, or a post id from graph-wasm's `post::CAPABILITIES`.
    #[arg(long)]
    pub id: String,
    /// Node count of the seeded model.
    #[arg(long, value_parser = clap::value_parser!(u32).range(2..=i64::from(MAX_PROBE_NODES)))]
    pub n: u32,
    /// Edges per node: Relation edges appended until m = ceil(r·n). At or below the seeded
    /// model's own ratio (≈ 1.55) nothing is appended.
    #[arg(long, value_parser = edges_per_node)]
    pub edges_per_node: Option<f64>,
    /// For a post: the layout whose geometry the post reads, run before the timer starts.
    #[arg(long, default_value = "layout.circular.radial")]
    pub input: String,
    /// Seed of the synthetic model.
    #[arg(long, default_value_t = 0)]
    pub seed: u32,
}

/// What the timed window runs: a registered layout, or a post over a layout's geometry.
enum Subject {
    Layout(&'static Capability),
    Post {
        index: u32,
        input: &'static Capability,
    },
}

/// One probe's numbers, before they are printed.
struct Measured {
    m: u32,
    run_ms: f64,
    ms: f64,
    build_peak_kib: u64,
    base_kib: u64,
    run_peak_kib: u64,
    hwm_reset: bool,
}

/// `graph-cli cap-probe`: exit 0 ran · 1 the run itself failed · 2 could not run.
pub fn run(plan: &Plan) -> ExitCode {
    let subject = match resolve(plan) {
        Ok(subject) => subject,
        Err(err) => {
            eprintln!("cap-probe: {err}");
            return ExitCode::from(2);
        }
    };
    let (status, code) = match probe(plan, &subject) {
        Ok(measured) => (format!("{} status=ok", fields(&measured)), 0),
        Err(err) => (format!("status=failed reason={err:?}"), 1),
    };
    let input = matches!(subject, Subject::Post { .. }).then_some(plan.input.as_str());
    println!(
        "cap-probe id={} input={} n={} load1={} {status}",
        plan.id,
        input.unwrap_or("-"),
        plan.n,
        loadavg().split_whitespace().next().unwrap_or("unavailable"),
    );
    ExitCode::from(code)
}

fn edges_per_node(text: &str) -> Result<f64, String> {
    let ratio: f64 = text.parse().map_err(|e| format!("{text}: {e}"))?;
    if ratio.is_finite() && (0.0..=64.0).contains(&ratio) {
        Ok(ratio)
    } else {
        Err(format!("{text}: edges per node must be within 0..=64"))
    }
}

fn resolve(plan: &Plan) -> Result<Subject, String> {
    if let Some(entry) = registry::find(&plan.id) {
        return Ok(Subject::Layout(entry));
    }
    let index = graph_wasm::post::CAPABILITIES
        .iter()
        .position(|entry| entry.id == plan.id)
        .ok_or_else(|| format!("{}: neither a layout nor a post id", plan.id))?;
    let input = registry::find(&plan.input)
        .ok_or_else(|| format!("--input {}: not a registered layout", plan.input))?;
    let index = u32::try_from(index).map_err(|e| e.to_string())?;
    Ok(Subject::Post { index, input })
}

fn probe(plan: &Plan, subject: &Subject) -> Result<Measured, String> {
    let (nodes, mut edges) = seeded_model(plan.seed, plan.n, REFERENCE_DEGREE);
    if let Some(ratio) = plan.edges_per_node {
        densify(&nodes, &mut edges, ratio);
    }
    let topology = index_model(&nodes, &edges).map_err(|e| format!("n={}: {e}", plan.n))?;
    let input = match subject {
        Subject::Layout(_) => None,
        Subject::Post { input, .. } => Some((input.run)(&topology).map_err(|e| e.to_string())?),
    };
    let build_peak_kib = rss::status_kib("VmHWM").unwrap_or(0);
    let hwm_reset = rss::reset_peak();
    let base_kib = rss::status_kib("VmRSS").unwrap_or(0);
    let started = Instant::now();
    let run_ms = window(subject, &topology, input.as_ref(), started)?;
    let ms = elapsed_ms(started);
    let run_peak_kib = rss::status_kib("VmHWM").unwrap_or(0);
    black_box((&nodes, &edges));
    Ok(Measured {
        m: topology.edge_count(),
        run_ms,
        ms,
        build_peak_kib,
        base_kib,
        run_peak_kib,
        hwm_reset,
    })
}

/// The timed window: the run, then both snapshot faces. Returns the run's own millis.
fn window(
    subject: &Subject,
    topology: &Topology,
    input: Option<&Geometry>,
    started: Instant,
) -> Result<f64, String> {
    let snapshot = match (subject, input) {
        (Subject::Layout(entry), _) => {
            let geometry = (entry.run)(topology).map_err(|e| e.to_string())?;
            graph_core::layout::snapshot(topology, geometry)
        }
        (Subject::Post { index, .. }, Some(geometry)) => {
            graph_wasm::post::snapshot(*index, topology, geometry)
                .ok_or("the post index is past the table")?
        }
        (Subject::Post { .. }, None) => return Err("a post needs its input geometry".into()),
    };
    let run_ms = elapsed_ms(started);
    let snapshot = snapshot.map_err(|e| e.to_string())?;
    black_box(snapshot.to_bytes());
    black_box(to_json(&snapshot));
    Ok(run_ms)
}

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1e3
}

fn mib(kib: u64) -> f64 {
    kib as f64 / 1024.0
}

fn fields(m: &Measured) -> String {
    format!(
        "m={} run_ms={:.1} ms={:.1} peak_mib={:.1} base_mib={:.1} run_peak_mib={:.1} hwm_reset={}",
        m.m,
        m.run_ms,
        m.ms,
        mib(m.build_peak_kib.max(m.run_peak_kib)),
        mib(m.base_kib),
        mib(m.run_peak_kib),
        m.hwm_reset
    )
}
