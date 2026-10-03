//! The measurement behind `topology.index`'s `scale_ceiling`
//! (`docs/measurements/p1-topology-memory.md`), kept in the repo so it can be re-run:
//!
//! ```sh
//! docker run --rm -v "$PWD:/w" ge-rust \
//!   cargo test --release -p graph-core --test memory -- --ignored --nocapture
//! ```
//!
//! A counting global allocator wraps `build_synthetic_model(n)`. **held** is the net heap
//! the returned topology owns (the input records are freed before it returns, so they
//! are not in it); **peak** is the highest net heap during the call, records included.
//! This is its own test binary, so the allocator counts nothing but this file.
//!
//! The 3D arms live in the child module [`three_d`], so they count against the same
//! allocator as the rows here. The `#[path]` is explicit because a crate root resolves a
//! submodule in its own directory: a bare `mod three_d;` in `tests/memory.rs` would want
//! `tests/three_d.rs`, which is the same trap `tests/edge_geometry_invariants.rs:30` walks
//! around by wrapping its body in a module named after the file.

#[path = "memory/three_d.rs"]
mod three_d;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

struct Counting;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(bytes: usize) {
    let now = CURRENT.fetch_add(bytes, Relaxed) + bytes;
    PEAK.fetch_max(now, Relaxed);
}

// SAFETY: every call is forwarded unchanged to `System`; the counters only observe.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        grow(layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract, which `System` shares.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size(), Relaxed);
        // SAFETY: `ptr` came from `System` with this `layout` (see `alloc`/`realloc`).
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        match new_size.checked_sub(layout.size()) {
            Some(more) => grow(more),
            None => {
                CURRENT.fetch_sub(layout.size() - new_size, Relaxed);
            }
        }
        // SAFETY: as for `dealloc`; `new_size` is the caller's, under the same contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

#[test]
#[ignore = "a measurement, not a check: run alone with --release -- --ignored --nocapture"]
fn topology_memory_per_node() {
    println!(
        "| n | m | arena bytes | arena strings | node columns | edge columns | 3 CSRs | held | peak | held / node | arena / node |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for n in [1_000_u32, 10_000, 100_000] {
        let base = CURRENT.load(Relaxed);
        PEAK.store(base, Relaxed);
        let t = graph_core::build_synthetic_model(f64::from(n)).expect("fits");
        let held = CURRENT.load(Relaxed) - base;
        let peak = PEAK.load(Relaxed) - base;
        let csr = t.out().byte_len() + t.inbound().byte_len() + t.hierarchy().byte_len();
        let per_node = |bytes: usize| bytes as f64 / f64::from(n);
        println!(
            "| {n} | {} | {} | {} | {} | {} | {csr} | {held} | {peak} | {:.1} B | {:.1} B |",
            t.edge_count(),
            t.strings().byte_len(),
            t.strings().len(),
            t.nodes().byte_len(),
            t.edges().byte_len(),
            per_node(held),
            per_node(t.strings().byte_len()),
        );
        assert!(held > 0 && peak >= held, "the allocator counted the build");
    }
}

/// The measurement behind `layout.grid`'s `scale_ceiling`: the highest net heap while
/// the pipeline runs the topology stage and the grid and writes the snapshot's bytes —
/// what a consumer holds at once. The input records are built before the count starts.
#[test]
#[ignore = "a measurement, not a check: run alone with --release -- --ignored --nocapture"]
fn grid_pipeline_memory_per_node() {
    println!("| n | m | snapshot bytes | peak | peak / node |");
    println!("|---|---|---|---|---|");
    for n in [1_000_u32, 10_000, 100_000] {
        let (nodes, edges) = graph_core::seeded_model(1, n, graph_core::REFERENCE_DEGREE);
        let base = CURRENT.load(Relaxed);
        PEAK.store(base, Relaxed);
        let params = graph_core::GridParams::default();
        let run = graph_core::run_pipeline::<graph_core::Grid>(&nodes, &edges, &params);
        let bytes = run.expect("fits").snapshot.to_bytes();
        let peak = PEAK.load(Relaxed) - base;
        println!(
            "| {n} | {} | {} | {peak} | {:.1} B |",
            edges.len(),
            bytes.len(),
            peak as f64 / f64::from(n),
        );
        assert!(peak > bytes.len(), "the allocator counted the run");
    }
}

/// The measurement behind the four Phase 3 hierarchy/graph layouts' `scale_ceiling`s: the
/// highest net heap while the pipeline runs the topology stage, the named layout, and
/// writes the snapshot's bytes, plus the wall-clock the layout call itself took (never
/// read by graph-core, D8 — this is a measurement script, not the motor). tidy tree,
/// treemap and circular are all O(n)/O(n log n) over the hierarchy substrate, so they are
/// swept to 100 000 nodes like the grid. Circle packing's exact path is comparable, but a
/// random synthetic graph at this density is essentially always non-planar, so it takes
/// the fallback's force relaxation (`layout/circle_packing/fallback.rs`), whose two
/// O(n^2) passes (edge-pull/overlap-push, then settle) dominate; it is swept over a much
/// smaller range so the measurement itself finishes.
/// A layout's bare `run`: `graph_core::Stage::run` at default params, already erased.
type Run = fn(&graph_core::Topology) -> Result<graph_core::Geometry, graph_core::StageError>;

#[test]
#[ignore = "a measurement, not a check: run alone with --release -- --ignored --nocapture"]
fn hierarchy_layout_pipeline_memory_per_node() {
    use graph_core::layout::{circular, tidy_tree, treemap};
    println!("| layout | n | m | snapshot bytes | peak | peak / node | wall |");
    println!("|---|---|---|---|---|---|---|");
    let layouts: [(&str, Run); 3] = [
        ("layout.tree.tidy", tidy_tree::run),
        ("layout.treemap.squarified", treemap::run),
        ("layout.circular.radial", circular::run),
    ];
    for (id, run) in layouts {
        for n in [1_000_u32, 10_000, 100_000] {
            print_measurement(id, run, n);
        }
    }
}

/// Circle packing on its own, much smaller sweep: the fallback's two O(n^2) relaxation
/// passes make 100 000 nodes impractical to even measure, let alone ship.
#[test]
#[ignore = "a measurement, not a check: run alone with --release -- --ignored --nocapture"]
fn circle_packing_pipeline_memory_per_node() {
    use graph_core::layout::circle_packing;
    println!("| layout | n | m | snapshot bytes | peak | peak / node | wall |");
    println!("|---|---|---|---|---|---|---|");
    for n in [300_u32, 1_000, 3_000] {
        print_measurement("layout.packing.circle", circle_packing::run, n);
    }
}

fn print_measurement(id: &'static str, run: Run, n: u32) {
    let (nodes, edges) = graph_core::seeded_model(1, n, graph_core::REFERENCE_DEGREE);
    let base = CURRENT.load(Relaxed);
    PEAK.store(base, Relaxed);
    let started = std::time::Instant::now();
    let run = graph_core::run_with(&nodes, &edges, id, run).expect("fits");
    let wall = started.elapsed();
    let bytes = run.snapshot.to_bytes();
    let peak = PEAK.load(Relaxed) - base;
    println!(
        "| {id} | {n} | {} | {} | {peak} | {:.1} B | {wall:.2?} |",
        edges.len(),
        bytes.len(),
        peak as f64 / f64::from(n),
    );
    assert!(peak > bytes.len(), "the allocator counted the run");
}
