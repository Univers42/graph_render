//! How many heap allocations one warm Barnes-Hut tick makes: a counting global allocator
//! around `ForceSession::step`, after a warm-up that brings every buffer to capacity.
//!
//! Its own test binary, so the allocator counts nothing but this file, and the counter is
//! per thread, so another test running in parallel cannot leak into it.

use graph_core::layout::force::{ForceParams, ForceSession};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static CALLS: Cell<u64> = const { Cell::new(0) };
}

fn count() {
    CALLS.with(|calls| calls.set(calls.get() + 1));
}

fn calls() -> u64 {
    CALLS.with(Cell::get)
}

struct Counting;

// SAFETY: every call is forwarded unchanged to `System`; the counter only observes.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract, which `System` shares.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `System` with this `layout` (see `alloc`/`realloc`).
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        // SAFETY: as for `dealloc`; `new_size` is the caller's, under the same contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

const NODES: u32 = 2_000;
const WARM: u32 = 16;
const TICKS: u64 = 16;

/// Allocations per warm tick, measured 2026-10-01 on develop 5936309 (perf P1): 176 over 16
/// ticks, from `partition`'s range list and the walk stack each pass builds. A ratchet, never
/// a target: perf P2 brings it to 0, and a change that adds an allocation turns this red.
const CEILING_PER_TICK: u64 = 11;

#[test]
fn a_warm_tick_allocates_no_more_than_the_ceiling() {
    let probe = calls();
    drop(std::hint::black_box(Vec::<u8>::with_capacity(1)));
    assert_eq!(calls() - probe, 1, "the counter counts an allocation");

    let (nodes, edges) = graph_core::seeded_model(1, NODES, graph_core::REFERENCE_DEGREE);
    let topology = graph_core::index_model(&nodes, &edges).expect("fits");
    let mut session =
        ForceSession::from_frozen(&topology, &ForceParams::default()).expect("valid params");
    session.step(WARM);
    let before = calls();
    for _ in 0..TICKS {
        session.step(1);
    }
    let total = calls() - before;
    println!("allocations over {TICKS} warm ticks at n={NODES}: {total}");
    assert!(
        total <= CEILING_PER_TICK * TICKS,
        "{total} allocations over {TICKS} warm ticks; the ceiling is {CEILING_PER_TICK} per tick"
    );
}
