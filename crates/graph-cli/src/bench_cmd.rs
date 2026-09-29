//! `graph-cli bench`: wall time and layout quality of one registered layout at chosen
//! node counts. Quality is Kruskal stress-1 with the optimal uniform scale, scale-free,
//! so layouts of different extent compare.
//!
//! Ponytail: stress is taken from `SOURCES` evenly spaced BFS sources, not all pairs, so
//! it under-samples a graph whose distant regions hold no source; unreachable pairs are
//! skipped, so a disconnected layout is scored on its components only. Time is one run,
//! wall clock, not a median: a loaded host inflates it.

use graph_contract::geometry::NodeGeometry;
use graph_core::{REFERENCE_DEGREE, registry, run_with, seeded_model};
use std::collections::VecDeque;
use std::process::ExitCode;
use std::time::Instant;

const SOURCES: usize = 16;

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
        for (to, &h) in hops
            .iter()
            .enumerate()
            .filter(|&(to, &h)| to != from && h != u32::MAX)
        {
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
    let scale = de / ee;
    // sum (d - a e)^2 = dd - 2 a de + a^2 ee = dd - de^2/ee at the optimal a.
    ((dd - scale * de) / dd).max(0.0).sqrt()
}

/// Prints one `layout nodes ms stress` row per node count; exit 1 if a run fails.
pub fn run(layout: &str, counts: &[u32], seed: u32) -> ExitCode {
    let Some(entry) = registry::find(layout) else {
        eprintln!("bench: {layout}: not registered");
        return ExitCode::from(2);
    };
    let mut code = ExitCode::SUCCESS;
    for &count in counts {
        let (nodes, edges) = seeded_model(seed, count, REFERENCE_DEGREE);
        let started = Instant::now();
        let run = run_with(&nodes, &edges, entry.id, entry.run);
        let ms = started.elapsed().as_secs_f64() * 1e3;
        match run {
            Ok(run) => {
                let parts = run.snapshot.parts();
                let NodeGeometry::Point { x, y } = &parts.nodes else {
                    println!("{layout} {count} {ms:.1} n/a");
                    continue;
                };
                let s = stress(x, y, &parts.source, &parts.target);
                println!("{layout} {count} {ms:.1} {s:.4}");
            }
            Err(err) => {
                eprintln!("bench: {layout} at {count} nodes: {err}");
                code = ExitCode::from(1);
            }
        }
    }
    code
}
