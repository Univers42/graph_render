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
