//! `graph-cli tick --n 100000 --ticks 10`: the wall time of single live-session ticks on
//! the scale model. `bench` times the whole 112-tick stage, which averages the per-tick
//! cost away; this is the steady workload a profiler (`scripts/orch/profile.sh`) points at.
//!
//! Barnes-Hut ticks run serially, through `ForceSession::step(1)`. Particle-mesh ticks run
//! through `ParticleMeshRun::step_with` on `--workers` threads ([`Threads`]; 1 is the serial
//! path), so a tick's thread scaling is read here without the whole stage `bench --tiers` times. `--warm` ticks run untimed first, so the quadtree and the scratch
//! buffers are at capacity before the first timed tick.
//!
//! Caveat: a tick's cost follows alpha, because the layout's spread sets the tree's depth,
//! so a short warm measures the early, most expensive ticks; raise `--warm` to measure a
//! settling layout. Wall clock on a loaded host is inflated, which is why the load average
//! is printed beside the numbers rather than assumed idle.

use super::campaign::median;
use super::scale::{MAX_SCALE_NODES, scale_model};
use super::tiers::markdown::loadavg;
use crate::exec_native::Threads;
use graph_core::layout::force::{ForceParams, ForceSession, ParticleMeshRun};
use graph_core::{REFERENCE_DEGREE, Topology, index_model};
use std::process::ExitCode;
use std::time::Instant;

/// The force layout a `graph-cli tick` run steps.
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum Layout {
    /// `layout.force.barnes_hut`.
    BarnesHut,
    /// `layout.force.particle_mesh`.
    ParticleMesh,
}

/// What one `graph-cli tick` invocation measures.
#[derive(clap::Args)]
pub struct Plan {
    /// The layout stepped.
    #[arg(long, value_enum, default_value_t = Layout::BarnesHut)]
    pub layout: Layout,
    /// Node count of the scale model (whole 100 000-node components past the model's cap).
    #[arg(long, default_value_t = 100_000,
          value_parser = clap::value_parser!(u32).range(2..=i64::from(MAX_SCALE_NODES)))]
    pub n: u32,
    /// Ticks timed, one at a time.
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..))]
    pub ticks: u32,
    /// Ticks run untimed before the first timed one.
    #[arg(long, default_value_t = 2)]
    pub warm: u32,
    /// Seed of the scale model.
    #[arg(long, default_value_t = 0)]
    pub seed: u32,
    /// Threads a particle-mesh tick's passes split across; Barnes-Hut ignores it.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=256))]
    pub workers: u32,
}

/// The table's header, printed once above the row.
pub const HEADER: &str = "| layout | n | m | index ms | warm ms | ticks | workers | tick min ms | tick median ms | tick max ms | alpha | load start | load end |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|";

/// Exit 0 with the table on standard output, or 2 when the model could not be built.
pub fn run(plan: &Plan) -> ExitCode {
    match measure(plan) {
        Ok(row) => {
            println!("{HEADER}\n{row}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("tick: could not run: {e}");
            ExitCode::from(2)
        }
    }
}

/// One row: index the model, warm a session, then time `plan.ticks` single ticks.
pub fn measure(plan: &Plan) -> Result<String, String> {
    let load_start = loadavg();
    let (nodes, edges) = scale_model(plan.seed, plan.n, REFERENCE_DEGREE);
    let started = Instant::now();
    let topology = index_model(&nodes, &edges).map_err(|e| format!("n={}: {e}", plan.n))?;
    let index_ms = ms_since(started);
    drop((nodes, edges));
    let started = Instant::now();
    let mut session = Stepper::start(plan, &topology).map_err(|e| format!("n={}: {e}", plan.n))?;
    session.step(plan.warm);
    let warm_ms = ms_since(started);
    if std::env::var_os("GM_TICK_DIAG").is_some() {
        if let Stepper::ParticleMesh(run, _) = &session {
            diag(run.xs(), run.ys());
        }
    }
    let samples: Vec<f64> = (0..plan.ticks)
        .map(|_| {
            let tick = Instant::now();
            session.step(1);
            ms_since(tick)
        })
        .collect();
    let (min, max) = samples
        .iter()
        .fold((f64::INFINITY, 0.0_f64), |(lo, hi), &s| {
            (lo.min(s), hi.max(s))
        });
    Ok(format!(
        "| {} | {} | {} | {index_ms:.1} | {warm_ms:.1} | {} | {} | {min:.2} | {:.2} | {max:.2} | {:.4} | {load_start} | {} |",
        session.id(),
        plan.n,
        topology.edge_count(),
        plan.ticks,
        session.workers(),
        median(samples),
        session.alpha(),
        loadavg(),
    ))
}

/// A run of either layout, stepped one tick at a time.
enum Stepper {
    BarnesHut(Box<ForceSession>),
    ParticleMesh(Box<ParticleMeshRun>, u32),
}

impl Stepper {
    fn start(plan: &Plan, topology: &Topology) -> Result<Stepper, String> {
        let params = ForceParams::default();
        let started = match plan.layout {
            Layout::BarnesHut => ForceSession::from_frozen(topology, &params)
                .map(|session| Self::BarnesHut(Box::new(session))),
            Layout::ParticleMesh => ParticleMeshRun::from_frozen(topology, &params)
                .map(|run| Self::ParticleMesh(Box::new(run), plan.workers)),
        };
        started.map_err(|e| e.to_string())
    }

    fn step(&mut self, ticks: u32) {
        match self {
            Self::BarnesHut(session) => {
                session.step(ticks);
            }
            Self::ParticleMesh(run, workers) => run.step_with(&Threads, *workers, ticks),
        }
    }

    fn alpha(&self) -> f64 {
        match self {
            Self::BarnesHut(session) => session.alpha(),
            Self::ParticleMesh(run, _) => run.alpha(),
        }
    }

    fn workers(&self) -> u32 {
        match self {
            Self::BarnesHut(_) => 1,
            Self::ParticleMesh(_, workers) => *workers,
        }
    }

    fn id(&self) -> &'static str {
        match self {
            Self::BarnesHut(_) => "layout.force.barnes_hut",
            Self::ParticleMesh(..) => "layout.force.particle_mesh",
        }
    }
}

fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1e3
}

fn diag(xs: &[f64], ys: &[f64]) {
    use std::collections::HashMap;
    let d = 32.0_f64;
    let mut grid: HashMap<(i64, i64), Vec<u32>> = HashMap::new();
    let (mut lox, mut hix, mut loy, mut hiy) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for i in 0..xs.len() {
        let (x, y) = (xs[i], ys[i]);
        lox = lox.min(x); hix = hix.max(x); loy = loy.min(y); hiy = hiy.max(y);
        grid.entry(((x / d).floor() as i64, (y / d).floor() as i64)).or_default().push(i as u32);
    }
    let mut hits: Vec<u32> = vec![0; xs.len()];
    let mut cands: u64 = 0;
    for (&(cx, cy), members) in &grid {
        for &i in members {
            for dy in -1..=1 { for dx in -1..=1 {
                if let Some(other) = grid.get(&(cx + dx, cy + dy)) {
                    cands += other.len() as u64;
                    for &j in other {
                        if j != i {
                            let (ex, ey) = (xs[i as usize] - xs[j as usize], ys[i as usize] - ys[j as usize]);
                            if ex * ex + ey * ey < d * d { hits[i as usize] += 1; }
                        }
                    }
                }
            }}
        }
    }
    let mut cell_sizes: Vec<usize> = grid.values().map(Vec::len).collect();
    cell_sizes.sort_unstable();
    let mut h = hits.clone(); h.sort_unstable();
    let n = xs.len();
    let total: u64 = hits.iter().map(|&v| u64::from(v)).sum();
    eprintln!("diag n={n} bounds x[{lox:.0},{hix:.0}] y[{loy:.0},{hiy:.0}] cells={} cell max={} p99={} | cand/node={:.1} hits/node={:.1} p50={} p99={} max={}",
        cell_sizes.len(), cell_sizes[cell_sizes.len()-1], cell_sizes[cell_sizes.len()*99/100],
        cands as f64 / n as f64, total as f64 / n as f64, h[n/2], h[n*99/100], h[n-1]);
}
