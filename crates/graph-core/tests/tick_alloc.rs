//! How many heap allocations one warm tick makes, Barnes-Hut's and particle-mesh's: a counting global allocator
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

/// The negative control (`GM_MUTATE_TICK_ALLOC=1`): skip the warm-up, so the counted ticks
/// include the motor's own first growth of every scratch buffer. The test must fail under
/// it, which shows the window sees an allocation the motor makes, not only the probe's.
fn warm_ticks() -> u32 {
    match std::env::var_os("GM_MUTATE_TICK_ALLOC") {
        Some(_) => 0,
        None => WARM,
    }
}

/// Allocations per warm tick. Measured 2026-10-01: 176 over 16 ticks on develop 5936309
/// (perf P1), from `partition`'s range list and the walk stack each pass built; 0 after perf
/// P2 (the stackless walks and `exec::ranges`). A change that adds one turns this red.
#[test]
fn a_warm_tick_allocates_nothing() {
    assert_warm_ticks_allocate_nothing(session());
}

/// The same for the particle-mesh tick: its mesh, grid and window are sized when the session
/// is built, and a live session ticks it as often as Barnes-Hut's.
#[test]
fn a_warm_particle_mesh_tick_allocates_nothing() {
    assert_warm_ticks_allocate_nothing(session().with_particle_mesh());
}

fn session() -> ForceSession {
    let (nodes, edges) = graph_core::seeded_model(1, NODES, graph_core::REFERENCE_DEGREE);
    let topology = graph_core::index_model(&nodes, &edges).expect("fits");
    ForceSession::from_frozen(&topology, &ForceParams::default()).expect("valid params")
}

fn assert_warm_ticks_allocate_nothing(mut session: ForceSession) {
    let probe = calls();
    drop(std::hint::black_box(Vec::<u8>::with_capacity(1)));
    assert_eq!(calls() - probe, 1, "the counter counts an allocation");

    session.step(warm_ticks());
    let before = calls();
    for _ in 0..TICKS {
        session.step(1);
    }
    let total = calls() - before;
    println!("allocations over {TICKS} warm ticks at n={NODES}: {total}");
    assert_eq!(
        total, 0,
        "{total} allocations over {TICKS} warm ticks; a warm tick must allocate nothing"
    );
}
