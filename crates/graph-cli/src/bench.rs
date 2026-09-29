//! `graph-cli bench --n 220,10000,100000`: wall time and Kruskal stress-1 of the Phase 6
//! layouts (or the `--layout`s named) on the synthetic model, one row per size and layout.
//!
//! The phase prompt is blunt about what a speed claim may rest on: d3's `forceManyBody`
//! already uses a quadtree, so Barnes-Hut is a constant-factor win, not an asymptotic
//! one, and the justification is a measured number at N = 220 / 10 000 / 100 000 —
//! N = 220 first, reported even when unflattering. `--vs-d3` times a real
//! `d3-force@3.0.0` run of the same graph (`harness/stress-d3.mjs`) beside ours.
//!
//! - It times the layout's registered `run` over an already indexed topology and nothing
//!   else: building the model and formatting output are outside the timer.
//! - It refuses a size past a layout's own registered `scale_ceiling` (exit 0, printed
//!   with the ceiling it came from) unless `--past-ceiling` asks for the run, which is
//!   then labelled. A stage error inside the ceiling is exit 1; past it, it is reported.
//! - Stress-1 uses the optimal uniform scale, so layouts of different extent compare.
//!
//! Ponytail: stress is taken from `SOURCES` evenly spaced BFS sources, not all pairs, so
//! it under-samples a graph whose distant regions hold no source; unreachable pairs are
//! skipped, so a disconnected layout is scored on its components only. Time is one run,
//! wall clock, native, not a median: a loaded host inflates it, and nothing here speaks
//! for wasm32 (Phase 9's campaign).

pub mod campaign;
pub mod scale;

#[cfg(test)]
mod tests;

use crate::stress::cases;
use graph_contract::geometry::NodeGeometry;
use graph_core::layout::Geometry;
use graph_core::registry::{self, Capability};
use graph_core::{REFERENCE_DEGREE, Topology, index_model, seeded_model};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

/// The layouts benched when no `--layout` is given: Phase 6's.
const PHASE6: [&str; 4] = [
    "layout.spectral",
    "layout.mds.pivot",
    "layout.force.barnes_hut",
    "layout.forceatlas2",
];

/// The layout `--vs-d3` compares against d3-force.
const D3_ARM: &str = "layout.force.barnes_hut";

const SOURCES: usize = 16;

/// What one `graph-cli bench` invocation runs.
pub struct Plan {
    pub sizes: Vec<u32>,
    pub layouts: Vec<String>,
    pub seed: u32,
    pub past_ceiling: bool,
    pub vs_d3: bool,
    pub dry_run: bool,
    /// Phase 9: runs per cell; the campaign reports their median, never one timing.
    pub repeat: u32,
    /// Phase 9: where the campaign's markdown goes, or nothing.
    pub out: Option<PathBuf>,
    /// Phase 9: report the largest N per arm that fits the frame budget.
    pub crossover: bool,
    /// Phase 9: the frame budget, in milliseconds.
    pub budget_ms: f64,
    /// Phase 9: write the scale fixture for `--seed` and the first `--n` here, and
    /// measure nothing.
    pub emit_scale_fixture: Option<PathBuf>,
}

/// `graph-cli bench`: exit 0 ran (refusals included) · 1 a run inside its ceiling failed ·
/// 2 could not run.
pub fn run(plan: &Plan) -> ExitCode {
    match bench(plan) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(err) => {
            eprintln!("bench: {err}");
            ExitCode::from(2)
        }
    }
}

fn bench(plan: &Plan) -> Result<bool, String> {
    if let Some(path) = &plan.emit_scale_fixture {
        return emit_fixture(plan, path.as_path());
    }
    if plan.crossover || plan.out.is_some() {
        return campaign::report::report(plan);
    }
    let entries = resolve(&plan.layouts)?;
    let mut pass = true;
    for &n in &plan.sizes {
        println!("n={n}");
        let (nodes, edges) = seeded_model(plan.seed, n, REFERENCE_DEGREE);
        let topology = index_model(&nodes, &edges).map_err(|e| format!("n={n}: {e}"))?;
        for entry in &entries {
            pass &= row(plan, entry, &topology)?;
        }
    }
    Ok(pass)
}

/// `--emit-scale-fixture`: the fixture written, its size reported. The generator is the
/// deliverable, so this is also the command `fixtures/scale/README.md` quotes.
fn emit_fixture(plan: &Plan, path: &Path) -> Result<bool, String> {
    let n = *plan.sizes.first().ok_or("--emit-scale-fixture needs --n")?;
    let json = scale::emit(path, n, plan.seed)?;
    println!(
        "emitted {} (n={n}, seed {}, {} bytes) — the generator, not this file, is the artefact",
        path.display(),
        plan.seed,
        json.len()
    );
    Ok(true)
}

/// The registered entries `names` asks for, or Phase 6's when it is empty.
fn resolve(names: &[String]) -> Result<Vec<&'static Capability>, String> {
    let wanted: Vec<&str> = if names.is_empty() {
        PHASE6.to_vec()
    } else {
        names.iter().map(String::as_str).collect()
    };
    wanted
        .into_iter()
        .map(|id| registry::find(id).ok_or_else(|| format!("{id}: not registered")))
        .collect()
}

/// One layout at one size: `false` only when a run inside its ceiling failed.
fn row(plan: &Plan, entry: &Capability, topology: &Topology) -> Result<bool, String> {
    let (id, ceiling, n) = (entry.id, entry.meta.scale_ceiling, topology.node_count());
    let past = u64::from(n) > ceiling;
    let label = if past { "  (past scale_ceiling)" } else { "" };
    if past && !plan.past_ceiling {
        println!("  {id:<26} refused: n={n} is past its scale_ceiling of {ceiling}");
        return Ok(true);
    }
    if plan.dry_run {
        println!("  {id:<26} would run (scale_ceiling {ceiling})");
        return Ok(true);
    }
    let started = Instant::now();
    let result = (entry.run)(topology);
    let ms = started.elapsed().as_secs_f64() * 1e3;
    let geometry = match result {
        Ok(geometry) => geometry,
        Err(err) => {
            println!("  {id:<26} failed: {err}{label}");
            return Ok(past);
        }
    };
    let quality = stress_of(topology, &geometry).map_or("n/a".into(), |s| format!("{s:.4}"));
    let edges = topology.edge_count();
    println!("  {id:<26} {ms:>10.2} ms  stress-1 {quality}  (edges={edges}){label}");
    if plan.vs_d3 && id == D3_ARM {
        let d3 = d3_ms(plan.seed, topology, &geometry)?;
        println!(
            "  {:<26} {d3:>10.2} ms  (d3-force@3.0.0, same graph)",
            "d3-force"
        );
    }
    Ok(true)
}

/// d3-force's wall time over the same graph, from the same golden-spiral start.
fn d3_ms(seed: u32, topology: &Topology, geometry: &Geometry) -> Result<f64, String> {
    let case = cases::case(seed, topology, geometry);
    let scratch = std::env::temp_dir().join(format!("gm-bench-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
    let timed = cases::d3(&scratch, std::slice::from_ref(&case));
    std::fs::remove_dir_all(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
    Ok(timed?[0].1)
}

fn stress_of(topology: &Topology, geometry: &Geometry) -> Option<f64> {
    let NodeGeometry::Point { x, y } = &geometry.nodes else {
        return None;
    };
    let edges = topology.edges();
    Some(stress(x, y, &edges.source, &edges.target))
}

/// Kruskal stress-1 of `x, y` against hop distance over `source`-`target` edges.
fn stress(x: &[f32], y: &[f32], source: &[u32], target: &[u32]) -> f64 {
    let n = x.len();
    let mut adjacent = vec![Vec::new(); n];
    for (&s, &t) in source.iter().zip(target) {
        adjacent[s as usize].push(t as usize);
        adjacent[t as usize].push(s as usize);
    }
    let (mut de, mut ee, mut dd) = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut hops = vec![u32::MAX; n];
    let mut queue = VecDeque::with_capacity(n);
    for from in (0..n).step_by((n / SOURCES).max(1)) {
        hops.fill(u32::MAX);
        hops[from] = 0;
        queue.push_back(from);
        while let Some(v) = queue.pop_front() {
            for &w in &adjacent[v] {
                if hops[w] == u32::MAX {
                    hops[w] = hops[v] + 1;
                    queue.push_back(w);
                }
            }
        }
        for (to, &h) in hops.iter().enumerate() {
            if to == from || h == u32::MAX {
                continue;
            }
            let dist = f64::from(h);
            let (dx, dy) = (f64::from(x[to] - x[from]), f64::from(y[to] - y[from]));
            let euclid = dx.hypot(dy);
            de += dist * euclid;
            ee += euclid * euclid;
            dd += dist * dist;
        }
    }
    if ee == 0.0 || dd == 0.0 {
        return 1.0;
    }
    // sum (d - a e)^2 = dd - 2 a de + a^2 ee = dd - de^2/ee at the optimal a = de/ee.
    ((dd - de / ee * de) / dd).max(0.0).sqrt()
}
