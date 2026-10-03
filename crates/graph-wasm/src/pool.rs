//! Browser threads model (a) (`prompts/perf-plan.md` P3): one shared linear memory and a
//! pool of helper threads, each one a wasm instance over that memory parked in
//! [`Pool::serve`]. [`PoolRunner`] cuts a pass into fixed chunks that the calling thread and
//! its helpers claim from one counter, so a pass costs one wake and one join, a slow core
//! takes fewer chunks instead of holding everyone back, and every chunk writes straight into
//! its own span of the caller's column: no per-worker buffer, no copy back. The pass waits
//! only for the helpers that took it: one the OS has not woken by the time the chunks run out
//! skips it, instead of every pass paying the slowest wake (perf-p3-steal).
//!
//! Target-independent on purpose: std's `Mutex` and `Condvar` are futexes natively and
//! `memory.atomic.wait32` on wasm32 built with `+atomics`, so the tests below drive this
//! same code with `std::thread` helpers. The bytes are [`Serial`]'s by the [`StepRange`]
//! contract (disjoint spans, start-of-pass reads); the tests check it.
//!
//! Caveat: a helper that traps (a panic is an abort on wasm32) never counts itself out of
//! its pass, so the coordinator waits forever, and a blocked coordinator runs no event loop to
//! hear of it. The host owns that: `harness/wasm-threads-helper.mjs` kills the process on a
//! trap. Natively a helper that panics counts itself out as it unwinds ([`Running`]), and the
//! coordinator panics at the end of that pass instead of returning a column with a hole.

use graph_core::exec::{Runner, Serial, StepRange, range_at};
use std::ops::Range;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

/// One pass handed to the helpers: called once per part, with the part's index.
type Part = dyn Fn(u32) + Sync;

/// A [`Part`] whose borrow [`Pool::broadcast`] keeps alive until every helper is done.
#[derive(Clone, Copy)]
struct Job(*const Part);

// SAFETY: the pointee is `Sync`, and `broadcast` does not return (ending the borrow) until
// every helper that was handed the pointer has finished calling it ([`Running`]).
unsafe impl Send for Job {}

struct State {
    generation: u64,
    job: Option<Job>,
    parts: u32,
    /// Helpers inside the job in flight: the only ones the coordinator waits for.
    running: u32,
    /// A helper unwound out of the job (natively; wasm32 aborts), so its chunk is unwritten.
    failed: bool,
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
            running: 0,
            failed: false,
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
        while let Some(running) = self.next(id, &mut seen) {
            // SAFETY: `running` counts this helper into the pass until it drops, so
            // `broadcast` is still waiting and the job's borrow is live.
            unsafe { (*running.job.0)(id) };
        }
    }

    /// Wakes every helper and returns once they have left [`serve`](Self::serve).
    pub fn close(&self) {
        self.lock().closed = true;
        self.wake.notify_all();
    }

    // The next pass this helper has not seen and is a part of, entered under the lock that
    // shows it the job. A pass whose job is already cleared ran out of chunks without this
    // helper, which skips it.
    fn next(&self, id: u32, seen: &mut u64) -> Option<Running<'_>> {
        let mut state = self.lock();
        loop {
            if state.closed {
                return None;
            }
            if state.generation != *seen {
                *seen = state.generation;
                if let Some(job) = state.job.filter(|_| id < state.parts) {
                    return Some(Running::enter(self, &mut state, job));
                }
            }
            state = self
                .wake
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    // Runs `part(0)` here and `part(id)` on every helper `id < parts` that wakes before
    // `part(0)` returns, then waits for those (`Joining`, also on unwind). `part(0)` returns
    // only once every chunk is claimed, so a helper that wakes later has nothing to do.
    fn broadcast(&self, parts: u32, part: &(dyn Fn(u32) + Sync + '_)) {
        // SAFETY: only the trait object's lifetime changes. This function does not return
        // until the job is cleared and `running` is 0, so no helper calls the job after the
        // borrow ends.
        let job = Job(unsafe {
            std::mem::transmute::<*const (dyn Fn(u32) + Sync + '_), *const Part>(part)
        });
        {
            let mut state = self.lock();
            assert!(state.job.is_none(), "one coordinator at a time");
            state.generation += 1;
            state.job = Some(job);
            state.parts = parts;
            debug_assert_eq!(state.running, 0, "a helper is still inside the last pass");
        }
        self.wake.notify_all();
        let _joining = Joining(self);
        part(0);
    }
}

/// A helper inside the job in flight: counted in by [`Running::enter`] under the lock that
/// shows it the job, counted out by its drop, on return and on unwind alike.
struct Running<'p> {
    pool: &'p Pool,
    job: Job,
}

impl<'p> Running<'p> {
    fn enter(pool: &'p Pool, state: &mut State, job: Job) -> Self {
        state.running += 1;
        Self { pool, job }
    }
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        let mut state = self.pool.lock();
        assert!(state.running > 0, "a helper left a pass it never entered");
        state.running -= 1;
        state.failed |= std::thread::panicking();
        if state.running == 0 {
            self.pool.done.notify_one();
        }
    }
}

/// Clears the pass in flight and waits out the helpers inside it, on return and on unwind
/// alike: if `part(0)` panics (natively; wasm32 aborts), `broadcast` must still not end the
/// job's borrow while a helper runs it. Without this a debug assertion in a part was a
/// use-after-free (SIGSEGV). The job is cleared first, under the same lock as the count, so
/// a helper either counted itself in before (and is waited for) or finds no job.
struct Joining<'p>(&'p Pool);

impl Drop for Joining<'_> {
    fn drop(&mut self) {
        let mut state = self.0.lock();
        state.job = None;
        while state.running > 0 {
            state = self
                .0
                .done
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        let failed = std::mem::take(&mut state.failed);
        drop(state);
        assert!(
            !failed || std::thread::panicking(),
            "a helper panicked inside a pass, so its chunk is unwritten"
        );
    }
}

/// A [`Runner`] over a [`Pool`]: up to `workers` threads, capped at the helpers present plus
/// the calling thread, claim the chunks `partition(len, parts × CHUNKS_PER_PART)` one at a
/// time. Any division is the same bytes by the [`StepRange`] contract.
pub struct PoolRunner<'p> {
    /// The pool whose helpers run beside the calling thread.
    pub pool: &'p Pool,
    /// The negative control: with two parts or more, the last chunk writes nothing, so its
    /// span keeps `Default` and the bytes must differ from [`Serial`]'s.
    pub skip_last: bool,
}

/// Chunks per thread in a pass.
///
/// Caveat: static equal parts left the 1M live tick waiting on its slowest core (the host's
/// E-cores and hyperthread siblings): the coordinator spent 32% of the tick blocked on its
/// helpers (perf-p3-scale). 16 bounds that tail at about a sixteenth of a part. Failing
/// input: a kernel whose per-call setup is large next to `len / (16 × parts)` outputs pays it
/// 16 times as often. Direction: slower, never wrong.
const CHUNKS_PER_PART: u32 = 16;

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
        let len = kernel.len();
        let parts = workers.clamp(1, self.pool.helpers() + 1).min(len);
        if parts < 2 {
            return Serial.run(kernel, 1, out);
        }
        // Only a grown tail is written here; each chunk clears its own span on the thread
        // that claims it (the `Runner` contract), so no thread clears the whole column.
        out.resize(len as usize, O::Out::default());
        let chunks = len.min(parts * CHUNKS_PER_PART);
        debug_assert!(chunks >= parts, "a part with no chunk to claim");
        let next = AtomicU32::new(0);
        let column = Column(out.as_mut_ptr());
        let part = |_: u32| {
            // Relaxed: the counter only hands out indices; the writes are published by the
            // pool's mutex when each part finishes.
            loop {
                let chunk = next.fetch_add(1, Ordering::Relaxed);
                if chunk >= chunks {
                    return;
                }
                let range = range_at(len, chunks, chunk);
                // SAFETY: each chunk index is claimed once, `range_at`'s ranges are disjoint
                // and inside `0..len`, and `out` is `len` long and untouched by the caller
                // until `broadcast` returns.
                let span = unsafe { span(column.base(), &range) };
                span.fill(O::Out::default());
                if !(self.skip_last && chunk == chunks - 1) {
                    kernel.step_range(range, span);
                }
            }
        };
        self.pool.broadcast(parts, &part);
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

#[cfg(test)]
mod harness;
#[cfg(test)]
pub(crate) use harness::with_pool;

#[cfg(test)]
mod tests;
