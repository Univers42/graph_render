//! The measurement behind `docs/measurements/perf-snapshot-build.md`: what
//! `layout::snapshot` costs at a million nodes, and what the bytes cost to write.
//!
//! ```sh
//! scripts/orch/gr cargo test --release -p graph-core --test snapshot_build -- --ignored --nocapture
//! ```
//!
//! **This file is a measurement, not a check.** Every test in it is `#[ignore]`d and
//! prints a markdown row; none of them asserts a number, so a change in the host's load
//! or its clock cannot turn the suite red. It is its own test binary so nothing else in
//! graph-core reads a clock through it.
//!
//! The model is `seeded_model(1, n, REFERENCE_DEGREE)`: the same generator the hash gate
//! runs, which emits about 1.5 edges per node, so `n = 1_000_000` is the 1M nodes /
//! 1.5M edges shape the browser profile was taken at. The topology and the grid layout
//! are built once, outside every timer; only `layout::snapshot` and `Snapshot::to_bytes`
//! are inside one.

use std::time::Instant;

use graph_core::{Geometry, GridParams, Stage, Topology};

/// Nodes the sweep runs at, with their edge counts printed beside them.
const SIZES: [u32; 2] = [1_000, 1_000_000];

/// How many times each size is measured inside one run; the median is what the doc
/// quotes. Three, so one outlier does not become the number.
const REPS: usize = 3;

/// One row per size: `snapshot` and `to_bytes`, medians of [`REPS`].
#[test]
#[ignore = "a measurement, not a check: run alone with --release -- --ignored --nocapture"]
fn snapshot_build_wall_ms() {
    println!("| n | m | reps | snapshot median ms | to_bytes median ms | snapshot bytes |");
    println!("|---|---|---|---|---|---|");
    for n in SIZES {
        let (topology, geometry) = model(n);
        let (snapshots, written, bytes) = measure(&topology, geometry);
        let ids = topology.node_count();
        let edges = topology.edge_count();
        println!(
            "| {ids} | {edges} | {REPS} | {:.1} | {:.1} | {bytes} |",
            median(&snapshots),
            median(&written),
        );
    }
}

/// The topology and the geometry the timers are given, both built outside any timer: the
/// model, its index and the grid layout are not what this file measures.
fn model(n: u32) -> (Topology, Geometry) {
    let (nodes, edges) = graph_core::seeded_model(1, n, graph_core::REFERENCE_DEGREE);
    let topology = graph_core::index_model(&nodes, &edges).expect("the model indexes");
    let geometry = graph_core::Grid::run(&topology, &GridParams::default()).expect("it lays out");
    (topology, geometry)
}

/// [`REPS`] timings of `layout::snapshot`, then [`REPS`] of the bytes it hands back, and
/// how many bytes those were. The geometry is cloned outside the timer, so the clone is
/// never in either number.
fn measure(topology: &Topology, geometry: Geometry) -> (Vec<f64>, Vec<f64>, usize) {
    let mut snapshots = Vec::with_capacity(REPS);
    let mut written = Vec::with_capacity(REPS);
    let mut bytes = 0;
    for _ in 0..REPS {
        let copy = geometry.clone();
        let started = Instant::now();
        let snapshot = graph_core::layout::snapshot(topology, copy).expect("a valid snapshot");
        snapshots.push(started.elapsed().as_secs_f64() * 1e3);
        let started = Instant::now();
        bytes = snapshot.to_bytes().len();
        written.push(started.elapsed().as_secs_f64() * 1e3);
    }
    (snapshots, written, bytes)
}

/// The middle of an odd-length run, sorted; `REPS` is 3, so this is the second.
fn median(samples: &[f64]) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[sorted.len() / 2]
}
