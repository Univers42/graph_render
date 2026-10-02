//! The wasm heap grows by an eighth at a time, not by 64 KiB at a time.
//!
//! On wasm32 `System` is dlmalloc, which asks `memory.grow` for only what one request lacks.
//! V8 answers every grow with a garbage collection over the whole JS heap, so a build that
//! grows the memory thousands of times pays for thousands of collections: a 100k-node
//! `gm_build` took 20.7 s under Node, and 1.3 s with this module
//! (`docs/measurements/perf-heap.md`). After any call that grew the memory, [`Geometric`]
//! takes a block of [`headroom`] bytes and frees it at once. dlmalloc cannot hand memory
//! back to wasm, so the block stays in its free top and serves the next requests without
//! another grow.
//!
//! Caveat: the headroom is reserved whether it is used or not, and wasm memory never shrinks,
//! so a module can hold up to an eighth more memory than its peak need (at most
//! [`MAX_HEADROOM`]; +19 MiB, 6%, on the 100k open measured). A workload that grows by one
//! huge block at a time gains nothing.

/// One wasm page.
pub(crate) const PAGE: usize = 64 << 10;
/// The least a grow reserves ahead.
pub(crate) const MIN_HEADROOM: usize = 1 << 20;
/// The most a grow reserves ahead. A quarter with a 256 MiB cap built 16% faster but held 19%
/// more than no reservation, against 6% here; memory is the scarcer resource at 1M nodes.
pub(crate) const MAX_HEADROOM: usize = 64 << 20;

/// What to reserve after the memory grew to `heap` bytes.
pub(crate) fn headroom(heap: usize) -> usize {
    (heap / 8).clamp(MIN_HEADROOM, MAX_HEADROOM)
}

#[cfg(target_arch = "wasm32")]
pub(crate) use wasm::Geometric;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::{PAGE, headroom};
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::arch::wasm32::memory_size;

    /// `System`, plus a reservation after each call that grew the memory.
    pub(crate) struct Geometric;

    fn reserve_after(pages_before: usize) {
        let pages = memory_size::<0>();
        if pages == pages_before {
            return;
        }
        let Ok(layout) = Layout::from_size_align(headroom(pages * PAGE), 1) else {
            return;
        };
        // SAFETY: `layout` is at least `MIN_HEADROOM` bytes, so not zero-sized; the block is
        // never read and is freed below with this same layout.
        let block = unsafe { System.alloc(layout) };
        if !block.is_null() {
            // SAFETY: `block` came from `System.alloc(layout)` just above.
            unsafe { System.dealloc(block, layout) }
        }
    }

    // SAFETY: every call is forwarded unchanged to `System`; `reserve_after` only adds an
    // allocation of its own, which it frees before returning.
    unsafe impl GlobalAlloc for Geometric {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let before = memory_size::<0>();
            // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract, which `System` shares.
            let ptr = unsafe { System.alloc(layout) };
            reserve_after(before);
            ptr
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let before = memory_size::<0>();
            // SAFETY: as for `alloc`.
            let ptr = unsafe { System.alloc_zeroed(layout) };
            reserve_after(before);
            ptr
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: `ptr` came from `System` with this `layout`: every path above forwards to it.
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let before = memory_size::<0>();
            // SAFETY: as for `dealloc`; `new_size` is the caller's, under the same contract.
            let ptr = unsafe { System.realloc(ptr, layout, new_size) };
            reserve_after(before);
            ptr
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_HEADROOM, MIN_HEADROOM, PAGE, headroom};

    /// `memory.grow` calls to serve `total` bytes as `request`-byte allocations, on a model of
    /// dlmalloc that grows by the shortfall in whole pages; `ahead` adds this module's reservation.
    /// Caveat: a model, not dlmalloc: no chunk headers, no free list, no fragmentation.
    fn grows_to(total: usize, request: usize, ahead: bool) -> usize {
        let (mut heap, mut used, mut grows) = (0, 0, 0);
        while used < total {
            if used + request > heap {
                heap += (used + request - heap).div_ceil(PAGE) * PAGE;
                grows += 1;
                if ahead {
                    heap += headroom(heap).div_ceil(PAGE) * PAGE;
                    grows += 1;
                }
            }
            used += request;
        }
        grows
    }

    #[test]
    fn a_two_gib_load_grows_the_memory_a_hundred_times_not_thirty_thousand() {
        let two_gib = 2 << 30;
        assert_eq!(grows_to(two_gib, 4 << 10, false), two_gib / PAGE);
        let ahead = grows_to(two_gib, 4 << 10, true);
        assert!(ahead <= 150, "{ahead} grows");
    }

    #[test]
    fn the_headroom_is_an_eighth_of_the_heap_within_its_bounds() {
        assert_eq!(headroom(0), MIN_HEADROOM);
        assert_eq!(headroom(64 << 20), 8 << 20);
        assert_eq!(headroom(2 << 30), MAX_HEADROOM);
    }
}
