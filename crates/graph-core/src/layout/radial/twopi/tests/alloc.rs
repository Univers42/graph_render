//! How much heap one `layout.twopi` run asks for, counted rather than timed.
//!
//! An edgeless graph of `n` nodes has `n` components, one node each, so it is the shape
//! that makes a per-component cost *the* cost. Finding L-01 is exactly that: `search` and
//! `count_leaves` each allocated node-count-long columns inside the component loop, so the
//! run spent `O(components x n)` where `registry/radial.rs` declares `O(n + m)`. A counted
//! number pins the claim and a timing cannot, because a slowdown is also what a slow host
//! looks like.
//!
//! The counter is per thread, so a test running beside this one cannot leak into it, and
//! it forwards every call to `System`, so arming it changes no address.

use super::super::run;
use super::super::scratch::Scratch;
use crate::layout::coords::probe::graph;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    /// Whether this thread's allocations are counted at all.
    static ARMED: Cell<bool> = const { Cell::new(false) };
    /// Allocations and reallocations seen while armed — a free is not one, so the number
    /// is "times the heap was asked for something", which is what a per-component column
    /// costs one of.
    static CALLS: Cell<i64> = const { Cell::new(0) };
    /// Bytes live while armed, and the high-water mark they reached.
    static LIVE: Cell<i64> = const { Cell::new(0) };
    static PEAK: Cell<i64> = const { Cell::new(0) };
}

struct Counting;

// SAFETY: every call is forwarded to `System` unchanged and the counters only observe, so
// this allocator's contract is exactly `System`'s.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size() as i64, true);
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract, which `System` shares.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // Its own override, because the default one would route through `alloc` and then
        // memset over `System.alloc_zeroed`'s already-zeroed pages.
        note(layout.size() as i64, true);
        // SAFETY: as for `alloc`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        note(-(layout.size() as i64), false);
        // SAFETY: `ptr` came from `System` with this `layout` (see `alloc`/`realloc`).
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size as i64 - layout.size() as i64, true);
        // SAFETY: `ptr` and `layout` are the realloc contract's, as for `dealloc`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// What [`probe`] saw: heap allocations, and the most bytes live at once.
#[derive(Debug)]
pub(super) struct Heap {
    /// Allocations and reallocations.
    pub(super) calls: i64,
    /// The high-water mark of live bytes.
    pub(super) peak: i64,
}

/// Runs `body` with the counters armed on this thread, and reports what it allocated.
///
/// A negative control first: one `black_box` allocation must be seen, or a counter that
/// had stopped counting would report zero and every budget below would pass for free.
pub(super) fn probe(body: impl FnOnce()) -> Heap {
    reset();
    ARMED.with(|armed| armed.set(true));
    drop(std::hint::black_box(Vec::<u8>::with_capacity(1)));
    let sentinel = CALLS.with(Cell::get);
    assert_eq!(sentinel, 1, "the counter does not see an allocation");
    CALLS.with(|calls| calls.set(0));
    LIVE.with(|live| live.set(0));
    PEAK.with(|peak| peak.set(0));
    body();
    ARMED.with(|armed| armed.set(false));
    Heap {
        calls: CALLS.with(Cell::get),
        peak: PEAK.with(Cell::get),
    }
}

fn reset() {
    CALLS.with(|calls| calls.set(0));
    LIVE.with(|live| live.set(0));
    PEAK.with(|peak| peak.set(0));
}

fn note(delta: i64, taken: bool) {
    ARMED.with(|armed| {
        if !armed.get() {
            return;
        }
        if taken {
            CALLS.with(|calls| calls.set(calls.get() + 1));
        }
        LIVE.with(|live| {
            let now = live.get() + delta;
            live.set(now);
            PEAK.with(|peak| {
                if now > peak.get() {
                    peak.set(now);
                }
            });
        });
    });
}

/// A whole edgeless layout, with its topology built outside the counted window.
pub(super) fn edgeless(count: u32) -> Heap {
    let topology = graph(count, &[]);
    probe(|| {
        std::hint::black_box(run(&topology).expect("an edgeless graph lays out"));
    })
}

/// Allocations a run may make, whatever the graph: the columns, the components, the
/// nearest-leaf queue and the output. Set by the run at `n = 4096` and nowhere near it.
const BUDGET: i64 = 256;

/// What four times the nodes may add on top of [`BUDGET`] — geometric growth only, which is
/// a `Vec` doubling about thirteen times.
const GROWTH_SLACK: i64 = 32;

/// `run` on an edgeless graph allocates its node-count-long columns **once**, not once per
/// component.
///
/// Before L-01 each component allocated its own `depth`, `parent`, `children` and `leaves`
/// and its own `span` and `theta`, so 4096 singleton components cost 6 x 4096 columns. The
/// second assertion carries the shape of the claim: four times the nodes must not cost
/// meaningfully more than the four, which is what separates `O(components x n)` from the
/// `O(n + m)` the registry row declares.
#[test]
fn an_edgeless_graph_allocates_its_columns_once_not_once_per_component() {
    let small = edgeless(1_024);
    let large = edgeless(4_096);
    println!(
        "allocations on an edgeless graph: n=1024 -> {}, n=4096 -> {}",
        small.calls, large.calls
    );
    assert!(
        large.calls <= BUDGET,
        "{large:?} at n=4096 exceeds the {BUDGET}-allocation budget; a per-component \
         column is back"
    );
    assert!(
        large.calls <= small.calls + GROWTH_SLACK,
        "four times the nodes cost {} extra allocations ({} -> {}), so the layout is still \
         paying per component",
        large.calls - small.calls,
        small.calls,
        large.calls
    );
}

/// The negative control for both budgets: the layout's own seven columns, built **once per
/// component** — the shape finding L-01 removed — through the same probe and against the
/// same numbers. It has to be over both, or the budget above would be a bound so loose
/// that it could not tell the two shapes apart.
///
/// Compiled in rather than reached through an environment knob, as the Barnes-Hut
/// split-sum control is: the test calls it directly and nothing else has to know it exists.
#[test]
fn one_set_of_columns_per_component_would_blow_both_budgets() {
    let small = per_component(1_024);
    let large = per_component(4_096);
    println!(
        "allocations for one column set per component: n=1024 -> {}, n=4096 -> {}",
        small, large
    );
    assert!(large > BUDGET, "{large} is inside the {BUDGET} budget");
    assert!(
        large > small + GROWTH_SLACK,
        "{small} -> {large} grows within the {GROWTH_SLACK}-allocation slack"
    );
}

/// `count` components, each allocating its own columns, as the layout used to.
fn per_component(count: u32) -> i64 {
    probe(|| {
        for _ in 0..count {
            drop(std::hint::black_box(Scratch::new(count)));
        }
    })
    .calls
}
