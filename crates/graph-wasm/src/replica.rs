//! Model **(b)** of `prompts/perf-plan.md`'s P3: **one wasm instance per worker**, each
//! holding its own full copy of the graph, every instance stepping the same stage in
//! lockstep. Inside one gathered pass an instance computes only its own range of nodes,
//! then a blocking all-gather through a `SharedArrayBuffer` fills in the other ranges, and
//! every instance then applies the same full array — so every instance stays byte-identical
//! to the serial run and to every other instance. Model (a), one shared memory with a thread
//! pool, is a separate build; this is not it and must not grow into it.
//!
//! [`Replica`] is the whole of it: a [`Runner`](graph_core::exec::Runner) that replaces the
//! division of a pass's outputs with one slice plus an all-gather. Everything it needs comes
//! from graph-core's own seam — [`StepRange`](graph_core::exec::StepRange)'s promise that
//! element `i` reads only start-of-pass state and writes only `out[i]`, and
//! [`partition`](graph_core::exec::partition)'s promise that any division of `0..n` gives the
//! same bytes — so this file computes nothing. It schedules.
//!
//! **Why the gather is blocking, and why that matters.** A tick hands the runner three
//! kernels (`barnes_hut::BarnesHut::THREADED_PASSES`), and each pass reads the *previous*
//! pass's writes for nodes it does not own — collide reads `x + vx`, and `x`/`vx` were moved
//! by the charge pass's serial apply. One sync per tick would therefore be a sync too late by
//! two passes, and the correct schedule is a sync inside *every* [`Runner::run`]. That is
//! why this model needs [`gm_host_allgather`] and why a host needs
//! `SharedArrayBuffer` + `Atomics.wait`, which in a browser means cross-origin isolation.
//!
//! Feature-gated behind `replicas`, so the shipped `graph_wasm.wasm` still imports nothing
//! (`harness/wasm-run.mjs` and the SDK's `refuseImports` both require that, and the default
//! artifact is not allowed to move).

#![cfg(target_arch = "wasm32")]

mod exports;

use graph_core::exec::{Runner, StepRange, partition};
use std::mem::size_of;
use std::ops::Range;

// The host's all-gather, and the **only** import this build adds: the instances share
// nothing themselves, so the one thing they must borrow is the buffer their spans land in.
//
// `(ptr, elem_bytes, len, lo, hi)`: publish `out[lo .. hi]` — `out` starting at `ptr`,
// `elem_bytes` per element — then read `out[0 .. len]` back in place. `0` is success and
// **any non-zero return is a refusal** (the buffer was too small for `len * elem_bytes`),
// never a partial gather: an instance that read back half a column would apply half a pass
// and drift from its peers without any of them reporting an error. A plain comment rather
// than a doc comment, because rustdoc does not generate documentation for extern blocks.
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn gm_host_allgather(ptr: u32, elem_bytes: u32, len: u32, lo: u32, hi: u32) -> u32;
}

/// One instance of a `ranks`-way run: which instance this is, and how many there are.
///
/// `Copy` because a tick lends it to every one of its passes by value and must not be able
/// to move the pair mid-tick — the schedule is fixed for the whole run.
#[derive(Debug, Clone, Copy)]
pub(super) struct Replica {
    rank: u32,
    ranks: u32,
}

impl Replica {
    /// This instance, `rank` of `ranks`. A `rank >= ranks` is refused by the export that
    /// builds one, so this never sees an instance that does not exist.
    pub(super) const fn new(rank: u32, ranks: u32) -> Self {
        Self { rank, ranks }
    }

    /// The instance count, which is also the `workers` every `run_with` is called with.
    pub(super) const fn ranks(self) -> u32 {
        self.ranks
    }
}

impl Runner for Replica {
    /// `workers` is ignored in favour of `ranks`, and deliberately: the ranks *are* the
    /// wasm instances, one per host thread, so the host's own thread count says nothing
    /// about how many instances there are — in this model a single Node thread drives every
    /// instance's code. `ranks` is fixed for the whole run, so it is the division rather
    /// than a per-call hint, and forwarding `workers` would re-divide the same pass by a
    /// number the caller happened to pass.
    fn run<O: StepRange>(&self, kernel: &O, _workers: u32, out: &mut Vec<O::Out>) {
        let len = kernel.len();
        out.clear();
        // The buffer is the kernel's own length before the first range runs: `step_range`
        // writes at range-relative indices, so a kernel given a short buffer would index
        // past its end rather than write somewhere wrong.
        out.resize(len as usize, O::Out::default());
        let span = self.span(len);
        kernel.step_range(
            span.clone(),
            &mut out[span.start as usize..span.end as usize],
        );
        self.gather(out.as_mut_ptr(), len, span);
    }
}

impl Replica {
    /// This instance's own range of `len` outputs, or an empty one when there are more
    /// instances than outputs: `partition` yields `min(len, ranks)` ranges, so an instance
    /// past that many owns nothing, and `len..len` is the honest statement of that. Such an
    /// instance still gathers — it applies the column its peers built.
    fn span(&self, len: u32) -> Range<u32> {
        partition(len, self.ranks)
            .into_iter()
            .nth(self.rank as usize)
            .unwrap_or(len..len)
    }

    /// Publishes this instance's span and reads the whole column back in place.
    ///
    /// A refusal is a panic, not a return value, and that is the only honest option left:
    /// [`Runner::run`] has no `Result` and no error channel, so a refusal this swallows
    /// would hand every instance a different column and every later pass would compound it
    /// into a silently wrong layout. Stopping is the loud answer; the host's own log says
    /// which rank and which span, and the export that called in recorded `gm_last_error`.
    fn gather<T>(&self, ptr: *mut T, len: u32, span: Range<u32>) {
        let elem = u32::try_from(size_of::<T>()).expect("an output element fits a u32");
        let addr = u32::try_from(ptr as usize).expect("a wasm32 address fits the wire's u32");
        // SAFETY: `ptr` is `out`'s own buffer, `out` was just resized to `len` elements of
        // `O::Out` and nothing has borrowed it since, and this call neither retains the
        // address nor outlives the borrow. `len`, `elem`, `span.start` and `span.end`
        // describe that buffer exactly, and the host copies `len * elem` bytes at `addr`.
        let refused = unsafe { gm_host_allgather(addr, elem, len, span.start, span.end) };
        assert!(
            refused == 0,
            "gm_host_allgather refused (code {refused}): rank {}/{} span {}..{} of {len} \
             elements of {elem} bytes — the shared buffer cannot hold {len} elements",
            self.rank,
            self.ranks,
            span.start,
            span.end
        );
    }
}
