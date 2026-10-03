//! `graph-cli tick --n 100000 --ticks 10`: the wall time of single live-session ticks on
//! the scale model. `bench` times the whole 112-tick stage, which averages the per-tick
//! cost away; this is the steady workload a profiler (`scripts/orch/profile.sh`) points at.
//!
//! Both layouts tick a `ForceSession`. Barnes-Hut ticks run serially. Particle-mesh ticks
//! (`with_particle_mesh`) run on `--workers` threads ([`Threads`]; 1 is the serial
//! path), so a tick's thread scaling is read here without the whole stage `bench --tiers` times. `--warm` ticks run untimed first, so the quadtree and the scratch
//! buffers are at capacity before the first timed tick.
//!
//! `--grow <BATCH>` switches to the other thing a live session is asked to do: carry itself
//! onto a bigger topology, timed beside the indexing that topology costs
//! ([`grow`](self::grow)).
//!
//! Caveat: a tick's cost follows alpha, because the layout's spread sets the tree's depth,
//! so a short warm measures the early, most expensive ticks; raise `--warm` to measure a
//! settling layout. Wall clock on a loaded host is inflated, which is why the load average
//! is printed beside the numbers rather than assumed idle.

pub mod grow;

use super::campaign::median;
use super::scale::{MAX_SCALE_NODES, scale_model};
use super::tiers::markdown::loadavg;
use crate::exec_native::Threads;
use graph_core::layout::force::{ForceParams, ForceSession};
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
    /// Grow mode: carry the session from the topology without the last `BATCH` nodes onto
    /// the whole model and time that carry, instead of timing ticks.
    #[arg(long, value_name = "BATCH")]
    pub grow: Option<u32>,
}

/// The table's header, printed once above the row.
pub const HEADER: &str = "| layout | n | m | index ms | warm ms | ticks | workers | tick min ms | tick median ms | tick max ms | alpha | load start | load end |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|";

/// Exit 0 with the table on standard output, or 2 when the model could not be built.
pub fn run(plan: &Plan) -> ExitCode {
    if let Some(batch) = plan.grow {
        return grow::report(plan.n, plan.seed, batch);
    }
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

/// A session of either layout, stepped one tick at a time.
struct Stepper {
    session: Box<ForceSession>,
    /// Always 1 for Barnes-Hut, whose ticks this command times serially.
    workers: u32,
    id: &'static str,
}

impl Stepper {
    fn start(plan: &Plan, topology: &Topology) -> Result<Stepper, String> {
        let session = ForceSession::from_frozen(topology, &ForceParams::default())
            .map_err(|e| e.to_string())?;
        Ok(match plan.layout {
            Layout::BarnesHut => Stepper {
                session: Box::new(session),
                workers: 1,
                id: "layout.force.barnes_hut",
            },
            Layout::ParticleMesh => Stepper {
                session: Box::new(session.with_particle_mesh()),
                workers: plan.workers,
                id: "layout.force.particle_mesh",
            },
        })
    }

    fn step(&mut self, ticks: u32) {
        self.session.step_with(&Threads, self.workers, ticks);
    }

    fn alpha(&self) -> f64 {
        self.session.alpha()
    }

    fn workers(&self) -> u32 {
        self.workers
    }

    fn id(&self) -> &'static str {
        self.id
    }
}

fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1e3
}
