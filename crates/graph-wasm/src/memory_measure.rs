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

#![cfg(test)]

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
        let text = crate::seed_ingest::document(&nodes, &edges);
        let layout = graph_core::registry::find("layout.grid").expect("registered");

        let base = CURRENT.load(Relaxed);
        PEAK.store(base, Relaxed);
        let (in_nodes, in_edges) = crate::ingest::read(text.as_bytes()).expect("valid ingest");
        let topology = graph_core::index_model(&in_nodes, &in_edges).expect("fits");
        let geometry = (layout.run)(&topology).expect("runs");
        let snapshot = graph_core::layout::snapshot(&topology, geometry).expect("fits");
        let bytes = snapshot.to_bytes();
        let peak = PEAK.load(Relaxed) - base;

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
