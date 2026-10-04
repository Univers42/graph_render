//! `graph-cli tick --n 100000 --ticks 10`: the wall time of single live-session ticks on
//! the scale model. `bench` times the whole 112-tick stage, which averages the per-tick
//! cost away; this is the steady workload a profiler (`scripts/orch/profile.sh`) points at.
//!
//! Both layouts tick a `ForceSession`, and both run on `--workers` threads ([`Threads`]; 1 is
//! the serial path). A Barnes-Hut tick's bytes do not depend on the worker count (the threaded
//! passes are gathers over fixed-order ranges), so a tick's thread scaling is read here for
//! either layout without the whole stage `bench --tiers` times. `--warm` ticks run untimed first,
//! so the quadtree and the scratch buffers are at capacity before the first timed tick.
//!
//! `--grow <BATCH>` switches to the other thing a live session is asked to do: carry itself
//! onto a bigger topology, timed beside the indexing that topology costs
//! ([`grow`]).
//!
//! Caveat: a tick's cost follows alpha, because the layout's spread sets the tree's depth,
//! so a short warm measures the early, most expensive ticks; raise `--warm` to measure a
//! settling layout. Wall clock on a loaded host is inflated, which is why the load average
//! is printed beside the numbers rather than assumed idle.

pub mod grow;
mod passes;

use super::campaign::median;
use super::scale::{MAX_SCALE_NODES, scale_model};
use super::tiers::markdown::loadavg;
use crate::exec_native::Threads;
use graph_core::exec::Runner;
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
    /// Threads a tick's passes split across, for either layout.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=256))]
    pub workers: u32,
    /// Grow mode: carry the session from the topology without the last `BATCH` nodes onto
    /// the whole model and time that carry, instead of timing ticks.
    #[arg(long, value_name = "BATCH")]
    pub grow: Option<u32>,
    /// Collision radius this run overrides the frozen `collideRadius` with, so the collide
    /// pass's own share of a tick can be timed instead of argued about.
    ///
    /// Caveat: this is a measurement knob on a bench command, not a layout parameter: absent
    /// it the session is built from `ForceParams::default()` and nothing about a production
    /// tick changes. A radius of 0 still runs the pass (it is `manyBody`'s `distanceMin` an
    /// exact overlap needs), so the row it prints is the cost of the walk and the build
    /// without the overlaps, not the cost of a tick with no collide pass at all.
    #[arg(long, value_name = "RADIUS")]
    pub collide_radius: Option<f64>,
    /// Also print where the timed ticks went, pass by pass, and what no pass covers.
    #[arg(long)]
    pub passes: bool,
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
    let samples = time_ticks(plan, &mut session);
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

/// `plan.ticks` single ticks' wall times; with `--passes`, the pass table on standard error.
fn time_ticks(plan: &Plan, session: &mut Stepper) -> Vec<f64> {
    let timed = passes::Timed::default();
    let samples: Vec<f64> = (0..plan.ticks)
        .map(|_| {
            let tick = Instant::now();
            if plan.passes {
                session.step_on(&timed, 1);
            } else {
                session.step(1);
            }
            ms_since(tick)
        })
        .collect();
    if plan.passes {
        let total = std::time::Duration::from_secs_f64(samples.iter().sum::<f64>() / 1e3);
        eprintln!("{}\n", timed.table(plan.ticks, total));
    }
    samples
}

/// A session of either layout, stepped one tick at a time.
struct Stepper {
    session: Box<ForceSession>,
    /// Worker count the session is stepped with; 1 is the serial path.
    workers: u32,
    id: &'static str,
}

impl Stepper {
    fn start(plan: &Plan, topology: &Topology) -> Result<Stepper, String> {
        let mut params = ForceParams::default();
        if let Some(radius) = plan.collide_radius {
            params.collide_radius = radius;
        }
        let session = ForceSession::from_frozen(topology, &params).map_err(|e| e.to_string())?;
        Ok(match plan.layout {
            Layout::BarnesHut => Stepper {
                session: Box::new(session),
                workers: plan.workers,
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
        self.step_on(&Threads, ticks);
    }

    fn step_on(&mut self, runner: &impl Runner, ticks: u32) {
        self.session.step_with(runner, self.workers, ticks);
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
