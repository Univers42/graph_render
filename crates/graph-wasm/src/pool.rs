//! Browser threads model (a) (`prompts/perf-plan.md` P3): one shared linear memory and a
//! pool of helper threads, each one a wasm instance over that memory parked in
//! [`Pool::serve`]. [`PoolRunner`] gives helper `i` range `i` of a pass and runs range 0 on
//! the calling thread, so a pass costs one wake and one join, and every part writes straight
//! into its own span of the caller's column: no per-worker buffer, no copy back.
//!
//! Target-independent on purpose: std's `Mutex` and `Condvar` are futexes natively and
//! `memory.atomic.wait32` on wasm32 built with `+atomics`, so the tests below drive this
//! same code with `std::thread` helpers. The bytes are [`Serial`]'s by the [`StepRange`]
//! contract (disjoint spans, start-of-pass reads); the tests check it.
//!
//! Caveat: a helper that traps (a panic is an abort on wasm32) never finishes its part,
//! so the coordinator waits forever, and a blocked coordinator runs no event loop to hear of
//! it. The host owns that: `harness/wasm-threads-helper.mjs` kills the process on a trap.
//! Natively a helper that panics hangs its pass the same way; only the coordinator's own part
//! is unwind-safe ([`Joining`]).

use graph_core::exec::{Runner, Serial, StepRange, partition};
use std::ops::Range;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

/// One pass handed to the helpers: called once per part, with the part's index.
type Part = dyn Fn(u32) + Sync;

/// A [`Part`] whose borrow [`Pool::broadcast`] keeps alive until every helper is done.
#[derive(Clone, Copy)]
struct Job(*const Part);

// SAFETY: the pointee is `Sync`, and `broadcast` does not return (ending the borrow) until
// every helper that was handed the pointer has finished calling it.
unsafe impl Send for Job {}

struct State {
    generation: u64,
    job: Option<Job>,
    parts: u32,
    pending: u32,
    helpers: u32,
    closed: bool,
}

/// The helper threads and the one pass they are working on.
pub struct Pool {
    state: Mutex<State>,
    wake: Condvar,
    done: Condvar,
}

impl Default for Pool {
    fn default() -> Self {
        Self::new()
    }
}

impl Pool {
    /// An empty pool; helpers join it by calling [`serve`](Self::serve).
    pub const fn new() -> Self {
        let state = State {
            generation: 0,
            job: None,
            parts: 0,
            pending: 0,
            helpers: 0,
            closed: false,
        };
        Self {
            state: Mutex::new(state),
            wake: Condvar::new(),
            done: Condvar::new(),
        }
    }

    // A poisoned lock means a native test thread panicked mid-part; the counters are still
    // the ones it left, and wasm32 aborts on panic, so it never sees one.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Helpers that have joined. Helper ids are their join order, from 1.
    pub fn helpers(&self) -> u32 {
        self.lock().helpers
    }

    /// Joins as a helper and runs every part addressed to it, until [`close`](Self::close).
    pub fn serve(&self) {
        let (id, mut seen) = {
            let mut state = self.lock();
            state.helpers += 1;
            (state.helpers, state.generation)
        };
        while let Some((job, parts)) = self.next(&mut seen) {
            if id < parts {
                // SAFETY: this helper is counted in `pending`, so `broadcast` is still
                // waiting and the job's borrow is live.
                unsafe { (*job.0)(id) };
                self.finish();
            }
        }
    }

    /// Wakes every helper and returns once they have left [`serve`](Self::serve).
    pub fn close(&self) {
        self.lock().closed = true;
        self.wake.notify_all();
    }

    // The next pass this helper has not seen. A pass it slept through finished without it,
    // which is only possible when it was not one of that pass's parts.
    fn next(&self, seen: &mut u64) -> Option<(Job, u32)> {
        let mut state = self.lock();
        loop {
            if state.closed {
                return None;
            }
            if state.generation != *seen {
                *seen = state.generation;
                if let Some(job) = state.job {
                    return Some((job, state.parts));
                }
            }
            state = self
                .wake
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn finish(&self) {
        let mut state = self.lock();
        state.pending -= 1;
        if state.pending == 0 {
            self.done.notify_one();
        }
    }

    // Runs `part(0)` here and `part(1..parts)` on helpers 1.., then waits for all of them
    // (`Joining`, also on unwind).
    // `parts` is at most `helpers() + 1`, which the caller ensures.
    fn broadcast(&self, parts: u32, part: &(dyn Fn(u32) + Sync + '_)) {
        // SAFETY: only the trait object's lifetime changes. This function does not return
        // until `pending` is 0, so no helper calls the job after the borrow ends.
        let job = Job(unsafe {
            std::mem::transmute::<*const (dyn Fn(u32) + Sync + '_), *const Part>(part)
        });
        {
            let mut state = self.lock();
            assert!(state.job.is_none(), "one coordinator at a time");
            state.generation += 1;
            state.job = Some(job);
            state.parts = parts;
            state.pending = parts - 1;
        }
        self.wake.notify_all();
        let _joining = Joining(self);
        part(0);
    }
}

/// Waits out the pass in flight and clears it, on return and on unwind alike: if `part(0)`
/// panics (natively; wasm32 aborts), `broadcast` must still not end the job's borrow while a
/// helper runs it. Without this a debug assertion in a part was a use-after-free (SIGSEGV).
struct Joining<'p>(&'p Pool);

impl Drop for Joining<'_> {
    fn drop(&mut self) {
        let mut state = self.0.lock();
        while state.pending > 0 {
            state = self
                .0
                .done
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        state.job = None;
    }
}

/// A [`Runner`] over a [`Pool`]: `partition(len, workers)`, capped at the helpers present
/// plus the calling thread.
pub struct PoolRunner<'p> {
    /// The pool whose helpers run parts 1 and up.
    pub pool: &'p Pool,
    /// The negative control: with two parts or more, the last one writes nothing, so its
    /// span keeps `Default` and the bytes must differ from [`Serial`]'s.
    pub skip_last: bool,
}

/// The base of the column the parts write into, shared by every part.
struct Column<T>(*mut T);

// SAFETY: each part forms a slice over its own range only, and the ranges are disjoint.
unsafe impl<T: Send> Sync for Column<T> {}

impl<T> Column<T> {
    // A method rather than `.0`, so a closure captures the `Sync` wrapper, not the pointer.
    fn base(&self) -> *mut T {
        self.0
    }
}

impl Runner for PoolRunner<'_> {
    fn run<O: StepRange>(&self, kernel: &O, workers: u32, out: &mut Vec<O::Out>) {
        let ranges = partition(kernel.len(), workers.clamp(1, self.pool.helpers() + 1));
        if ranges.len() < 2 {
            return Serial.run(kernel, 1, out);
        }
        out.clear();
        out.resize(kernel.len() as usize, O::Out::default());
        let last = ranges.len() - 1;
        let column = Column(out.as_mut_ptr());
        let part = |i: u32| {
            let i = i as usize;
            if self.skip_last && i == last {
                return;
            }
            // SAFETY: `partition`'s ranges are disjoint and inside `0..len`, `out` has
            // `len` elements and is not touched by the caller until `broadcast` returns.
            let span = unsafe { span(column.base(), &ranges[i]) };
            kernel.step_range(ranges[i].clone(), span);
        };
        self.pool.broadcast(ranges.len() as u32, &part);
    }
}

/// `range` of the column at `base`, as its own slice.
///
/// # Safety
/// `base` points to at least `range.end` initialised elements, and no other live slice
/// overlaps `range` while the returned one is used.
unsafe fn span<'a, T>(base: *mut T, range: &Range<u32>) -> &'a mut [T] {
    // SAFETY: the caller's contract above.
    unsafe { std::slice::from_raw_parts_mut(base.add(range.start as usize), range.len()) }
}

/// Runs `body` over a pool served by `helpers` `std::thread`s in place of wasm instances: the
/// same `Mutex`/`Condvar` code the shared-memory build runs. The pool is closed when `body`
/// ends, panicking or not, so the scope can join.
#[cfg(test)]
pub(crate) fn with_pool(helpers: u32, body: impl FnOnce(&Pool)) {
    struct Closing<'p>(&'p Pool);
    impl Drop for Closing<'_> {
        fn drop(&mut self) {
            self.0.close();
        }
    }
    let pool = Pool::new();
    std::thread::scope(|scope| {
        for _ in 0..helpers {
            scope.spawn(|| pool.serve());
        }
        let _closing = Closing(&pool);
        while pool.helpers() < helpers {
            std::thread::yield_now();
        }
        body(&pool);
    });
}

#[cfg(test)]
mod tests;
