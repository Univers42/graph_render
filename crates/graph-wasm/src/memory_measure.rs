//! The measurement behind the `transport.wasm.columnar` / `sdk.js` ledger rows'
//! `scale_ceiling` (`docs/measurements/phase04-transport.md`): the peak heap the
//! provisional-ingest path holds carrying one seed's model from ingest JSON text
//! (already built, excluded from the count — the same convention
//! `crates/graph-core/tests/memory.rs` uses for its input records) through
//! [`crate::ingest::read`], [`graph_core::index_model`], the registered layout, and the
//! encoded snapshot bytes: exactly the span `gm_build` + `gm_run` + `gm_snapshot_bytes`
//! cover in the real ABI. A counting global allocator wraps it — `graph-core`'s own
//! technique, reused here as a `cfg(test)`-only unit test so it never reaches a release
//! build (unlike `graph-core`'s, which is a separate `tests/` binary, this one has to live
//! inside the crate: `ingest`/`seed_ingest` are `cfg(any(test, wasm32))`-private, so an
//! external integration test cannot reach them at all).
//!
//! ```sh
//! docker run --rm -v "$PWD:/w" ge-rust \
//!   cargo test --release -p graph-wasm --lib -- --ignored --nocapture provisional_ingest
//! ```
//!
//! The counters are per thread, so another test of this binary allocating concurrently
//! does not inflate the peak; a block one thread frees that another allocated moves only
//! the freeing thread's count.
//!
//! Ponytail: the peak sums the bytes the pipeline *requests* (`Layout::size`), not the
//! allocator's alignment padding nor wasm's 64 KiB page granularity, so it under-reports
//! linear-memory growth (`memory.size()`) by up to one page plus padding per allocation.
//! It is the heap the motor asks for, which is what the ledger row compares across n.

#![cfg(test)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

mod ingest_peak;

struct Counting;

// Signed: a thread that frees a block another thread allocated goes below its base.
thread_local! {
    static CURRENT: Cell<isize> = const { Cell::new(0) };
    static PEAK: Cell<isize> = const { Cell::new(0) };
}

// `Layout::size` is at most `isize::MAX`, so the casts are exact. `try_with` fails only
// during this thread's TLS teardown, where there is no measurement left to record.
fn grow(bytes: usize) {
    let _ = CURRENT.try_with(|current| {
        let now = current.get() + bytes as isize;
        current.set(now);
        let _ = PEAK.try_with(|peak| peak.set(peak.get().max(now)));
    });
}

fn shrink(bytes: usize) {
    let _ = CURRENT.try_with(|current| current.set(current.get() - bytes as isize));
}

// SAFETY: every call is forwarded unchanged to `System`; the counters only observe.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        grow(layout.size());
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract, which `System` shares.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        shrink(layout.size());
        // SAFETY: `ptr` came from `System` with this `layout` (see `alloc`/`realloc`).
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        match new_size.checked_sub(layout.size()) {
            Some(more) => grow(more),
            None => shrink(layout.size() - new_size),
        }
        // SAFETY: as for `dealloc`; `new_size` is the caller's, under the same contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// This test only: every other test in this binary allocates through `Counting` too
/// (harmless — every call forwards unchanged to `System`), but only this one resets and
/// reads the counters, and it is `#[ignore]`d so a normal `cargo test` run never depends
/// on running alone.
#[test]
#[ignore = "a measurement, not a check: run alone with --release -- --ignored --nocapture"]
fn provisional_ingest_pipeline_memory_per_node() {
    println!("| n | m | ingest text bytes | snapshot bytes | peak | peak / node |");
    println!("|---|---|---|---|---|---|");
    for n in [1_000_u32, 10_000, 100_000] {
        let (nodes, edges) = graph_core::seeded_model(1, n, graph_core::REFERENCE_DEGREE);
        let text = crate::seed_ingest::document(&nodes, &edges).expect("finite");
        let layout = graph_core::registry::find("layout.grid").expect("registered");

        let ((in_edges, bytes), peak) = measure(|| {
            let (in_nodes, in_edges) = crate::ingest::read(text.as_bytes()).expect("valid");
            let topology = graph_core::index_model(&in_nodes, &in_edges).expect("fits");
            let geometry = (layout.run)(&topology).expect("runs");
            let snapshot = graph_core::layout::snapshot(&topology, geometry).expect("fits");
            (in_edges, snapshot.to_bytes())
        });

        println!(
            "| {n} | {} | {} | {} | {peak} | {:.1} B |",
            in_edges.len(),
            text.len(),
            bytes.len(),
            peak as f64 / f64::from(n),
        );
        assert!(peak > bytes.len(), "the allocator counted the run");
    }
}

/// Peak bytes held live while `run` ran, above what was held before it.
/// This thread's only: another thread's allocations are not counted.
fn measure<T>(run: impl FnOnce() -> T) -> (T, usize) {
    let base = CURRENT.with(Cell::get);
    PEAK.with(|peak| peak.set(base));
    let value = run();
    // PEAK starts at `base` and only rises, so the difference is never negative.
    (value, (PEAK.with(Cell::get) - base).unsigned_abs())
}

/// F-92: a test running on another thread of this binary must not inflate the peak.
#[test]
fn another_threads_allocation_is_not_counted_in_this_threads_peak() {
    let ((), peak) = measure(|| {
        let other = std::thread::spawn(|| drop(std::hint::black_box(vec![0_u8; 64 << 20])));
        other.join().expect("the other thread ran");
    });
    assert!(
        peak < 1 << 20,
        "peak {peak} B counted the other thread's 64 MiB"
    );
}
