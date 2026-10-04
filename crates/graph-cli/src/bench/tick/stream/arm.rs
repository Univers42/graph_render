//! The native arm of the stream measurement: replay `--from <path.jsonl>` through the export's
//! own body, [`graph_wasm::service`], so this and `harness/wasm-stream-bench.mjs` time the same
//! work — parse the line, append it — rather than two different ones.
//!
//! Inside each batch, two timers run back to back and neither contains the other:
//! `service::extend` (`ingest::read_records` then `Topology::extend`) and `ForceSession::grow`
//! onto the topology that extend just appended to. Outside both: reading the line off disk, the
//! `step(10)` after line 0, and the `step(1)` after each batch. The `step(1)` is untimed on
//! purpose — the contract is `extend` plus `grow`, and a tick inside the timer would fold a
//! whole tick into a batch number.
//!
//! Caveat: a p95 over ten batches is one interpolated value, not a tail (see
//! [`super::stats`]). The `extend` column carries both halves of an append and this file does
//! not separate them: naming parse against index needs graph-core's own timers, which are not
//! on this path. Wall clock on a shared host is inflated, so `/proc/loadavg` is printed at both
//! ends rather than assumed idle.

use super::stats::{max, p95};
use crate::bench::campaign::median;
use crate::bench::tick::{Layout, Plan, Stepper};
use crate::bench::tiers::markdown::loadavg;
use graph_core::Topology;
use graph_wasm::service::{self, Source};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

/// Ticks after line 0, untimed, so the session has a quadtree before the first batch grows it.
const WARM_TICKS: u32 = 10;

/// The per-batch table's header.
const BATCH_HEADER: &str =
    "| batch | nodes after | extend ms | grow ms | sum ms |\n|---:|---:|---:|---:|---:|";

/// The summary table's header.
const SUMMARY_HEADER: &str = "| layout | batch | n | extend median ms | grow median ms | \
sum median ms | sum p95 ms | sum max ms | load start | load end |\n\
|---|---|---|---|---|---|---|---|---|---|";

/// Replay `path` on `plan.layout`'s engine and print the two tables.
pub fn report(plan: &Plan, batch: u32, path: &Path) -> Result<String, String> {
    let load_start = loadavg();
    let mut run = Run::open(plan, path)?;
    let mut batches = Vec::with_capacity(plan.batches as usize);
    for line_no in 1..=plan.batches {
        batches.push(run.batch(line_no)?);
    }
    let replay = Replay {
        layout: engine(plan.layout),
        batch,
        load_start,
        batches,
    };
    Ok(format!(
        "{BATCH_HEADER}\n{}\n{SUMMARY_HEADER}\n{}",
        replay.rows(),
        replay.summary()
    ))
}

/// One replayed run: the stream, the topology it has grown so far, and the session on it.
struct Run<'a> {
    file: BufReader<File>,
    path: &'a Path,
    topology: Topology,
    session: Stepper,
}

impl<'a> Run<'a> {
    /// The untimed opening: line 0 built, a session on it, and the warm ticks that give the
    /// session a quadtree before the first batch grows it.
    fn open(plan: &Plan, path: &'a Path) -> Result<Run<'a>, String> {
        let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut file = BufReader::new(file);
        let topology = head(&mut file, path)?;
        let mut session = Stepper::start(plan, &topology)?;
        session.step(WARM_TICKS);
        Ok(Run {
            file,
            path,
            topology,
            session,
        })
    }

    /// One batch: read it untimed, `extend` and `grow` timed apart, then one untimed tick.
    fn batch(&mut self, line_no: u32) -> Result<Batch, String> {
        let line = read_line(&mut self.file, self.path, line_no)?;
        let started = Instant::now();
        service::extend(&mut self.topology, &line)
            .map_err(|e| format!("line {line_no}: {}", e.name()))?;
        let extend_ms = ms_since(started);
        let started = Instant::now();
        self.session.grow(&self.topology)?;
        let grow_ms = ms_since(started);
        drop(line);
        self.session.step(1);
        Ok(Batch {
            nodes: self.topology.node_count(),
            extend_ms,
            grow_ms,
        })
    }
}

/// Line 0, untimed: the whole graph this run starts from. Its timer is deliberately nobody's —
/// the contract is a batch, and the first build is not one.
///
/// Caveat: at 1M nodes this line is 448 MB of JSON and the build that reads it is seconds, so
/// a run that timed it would report the input, not a batch. Untimed here means unaccounted
/// too: it is not folded into any batch's `extend`.
fn head(file: &mut BufReader<File>, path: &Path) -> Result<Topology, String> {
    let line = read_line(file, path, 0)?;
    service::build(&line, Source::Ingest).map_err(|e| format!("line 0: {}", e.name()))
}

/// Line `line_no`, without its newline. An empty read is a refusal naming the line the file ran
/// out on, not a short batch.
fn read_line(file: &mut BufReader<File>, path: &Path, line_no: u32) -> Result<Vec<u8>, String> {
    let mut line = Vec::new();
    let read = file
        .read_until(b'\n', &mut line)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if read == 0 {
        return Err(format!("{}: line {line_no} is missing", path.display()));
    }
    line.pop();
    Ok(line)
}

/// One batch's two timed halves.
struct Batch {
    nodes: u32,
    extend_ms: f64,
    grow_ms: f64,
}

impl Batch {
    /// What the contract is measured on: the append and the carry, and nothing else.
    fn sum(&self) -> f64 {
        self.extend_ms + self.grow_ms
    }
}

/// The whole run's samples, and the two tables they make.
struct Replay {
    layout: &'static str,
    batch: u32,
    load_start: String,
    batches: Vec<Batch>,
}

impl Replay {
    /// One row per batch, in the order the batches ran.
    fn rows(&self) -> String {
        self.batches
            .iter()
            .enumerate()
            .map(|(i, b)| {
                format!(
                    "| {} | {} | {:.2} | {:.2} | {:.2} |",
                    i + 1,
                    b.nodes,
                    b.extend_ms,
                    b.grow_ms,
                    b.sum()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// One summary row: the batch size, the median of each half, and the median, p95 and max
    /// of the sum.
    fn summary(&self) -> String {
        let sums: Vec<f64> = self.batches.iter().map(Batch::sum).collect();
        let extend: Vec<f64> = self.batches.iter().map(|b| b.extend_ms).collect();
        let grow: Vec<f64> = self.batches.iter().map(|b| b.grow_ms).collect();
        format!(
            "| {} | {} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {} | {} |",
            self.layout,
            self.batch,
            self.nodes(),
            median(extend),
            median(grow),
            median(sums.clone()),
            p95(sums.clone()),
            max(&sums),
            self.load_start,
            loadavg(),
        )
    }

    /// The node count the last batch left behind.
    fn nodes(&self) -> u32 {
        self.batches.last().map_or(0, |b| b.nodes)
    }
}

/// The layout id this run's engine is registered under, so a row names its engine.
fn engine(layout: Layout) -> &'static str {
    match layout {
        Layout::BarnesHut => "layout.force.barnes_hut",
        Layout::ParticleMesh => "layout.force.particle_mesh",
    }
}

/// Milliseconds since `start`.
fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1e3
}
