//! `gm_alloc`/`gm_free` (`docs/contract/wasm-abi.md` "Allocation safety", C5): the
//! caller's staging buffers for an ingest payload. Fallible — `try_reserve`, never
//! `reserve` (which aborts the process on failure, exactly the trap this export exists
//! to avoid) — and every live allocation is tracked `ptr -> caller length` so `gm_free`
//! and `gm_build` can refuse a pointer/length pair that was never handed out, or handed
//! back twice.
//!
//! [`Allocations`] holds the refusal logic and is target-independent (tested here with
//! synthetic addresses, never a real allocation); only the `extern "C"` exports at the
//! bottom touch the real allocator, and only they need `target_arch = "wasm32"`.

use std::collections::BTreeMap;

/// Byte alignment of every `gm_alloc` buffer: 4, the wire's own word alignment
/// (`docs/contract/binary-layout.md`), so a caller can write `u32`/`f32` values into it
/// directly with no realignment.
pub const ALIGN: usize = 4;

/// Live allocations, `ptr -> the caller's requested length` (the length `gm_free` and
/// `gm_build` must be given back, not necessarily the actual byte size reserved).
#[derive(Debug, Default)]
pub struct Allocations(BTreeMap<u32, u32>);

/// Why `gm_free`/`gm_build` refused a `(ptr, len)` pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimError {
    /// No live allocation starts at this pointer (never issued, or already freed: a
    /// double free is refused the same way as a pointer that was never `gm_alloc`'d).
    Unknown,
    /// A live allocation starts here, but not at this length.
    LengthMismatch,
}

impl Allocations {
    /// The actual bytes to reserve for a request of `len`: never `0`, because
    /// `GlobalAlloc` with a zero-size layout is undefined behaviour. `gm_free` recovers
    /// this same size from the tracked caller length, so the two calls agree.
    pub const fn actual_size(len: u32) -> usize {
        if len == 0 { 1 } else { len as usize }
    }

    /// Records a fresh allocation. `gm_alloc` calls this once it holds a real pointer.
    pub fn record(&mut self, ptr: u32, len: u32) {
        self.0.insert(ptr, len);
    }

    /// Removes `(ptr, len)` if it is exactly one live allocation, or the refusal.
    /// `gm_free` frees only after this succeeds; `gm_build` calls it too (it does not
    /// free — it copies — but a `(ptr, len)` gm_build cannot verify as live is exactly
    /// the caller-error `gm_free` would also refuse).
    pub fn take(&mut self, ptr: u32, len: u32) -> Result<(), ClaimError> {
        match self.0.get(&ptr) {
            None => Err(ClaimError::Unknown),
            Some(&recorded) if recorded != len => Err(ClaimError::LengthMismatch),
            Some(_) => {
                self.0.remove(&ptr);
                Ok(())
            }
        }
    }

    /// Whether `(ptr, len)` is exactly a live allocation, without removing it.
    /// `gm_build` uses this to validate its source before copying out of it.
    pub fn contains(&self, ptr: u32, len: u32) -> bool {
        self.0.get(&ptr) == Some(&len)
    }

    /// How many allocations are live. Exposed for the memory-safety tests only — nothing
    /// in the wasm32 pointer layer calls it, so it is `cfg(test)`-only: a real wasm32
    /// build never carries a method it never calls.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_size_is_never_zero() {
        assert_eq!(Allocations::actual_size(0), 1, "GlobalAlloc(0) is UB");
        assert_eq!(Allocations::actual_size(1), 1);
        assert_eq!(Allocations::actual_size(4096), 4096);
    }

    /// `Layout::from_size_align` (the wasm32 `pointer_layer`'s `layout()`) requires a
    /// power-of-two alignment; this is the value every `gm_alloc` buffer is promised
    /// (`docs/contract/wasm-abi.md`), tested natively since the layout math itself does
    /// not touch memory.
    #[test]
    fn align_is_the_wires_own_word_size_and_a_valid_power_of_two() {
        assert_eq!(
            ALIGN, 4,
            "the wire's own word size, docs/contract/binary-layout.md"
        );
        assert!(
            ALIGN.is_power_of_two(),
            "Layout::from_size_align requires it"
        );
    }

    #[test]
    fn take_refuses_an_unknown_pointer_and_a_length_mismatch() {
        let mut live = Allocations::default();
        assert_eq!(live.take(4, 16), Err(ClaimError::Unknown));
        live.record(4, 16);
        assert!(live.contains(4, 16));
        assert_eq!(
            live.take(4, 8),
            Err(ClaimError::LengthMismatch),
            "wrong length"
        );
        assert!(
            live.contains(4, 16),
            "a refused take does not remove the entry"
        );
        assert_eq!(live.take(4, 16), Ok(()));
        assert!(!live.contains(4, 16), "removed once taken");
    }

    #[test]
    fn a_double_free_is_refused_the_same_way_as_an_unknown_pointer() {
        let mut live = Allocations::default();
        live.record(100, 0);
        assert_eq!(
            live.take(100, 0),
            Ok(()),
            "a zero-length allocation is trackable"
        );
        assert_eq!(
            live.take(100, 0),
            Err(ClaimError::Unknown),
            "the second free of the same pointer finds nothing live"
        );
    }

    /// Review unverified #6: `is_live` (`gm_build`, `read_params`) reads `contains`, which
    /// admits only the exact recorded `(ptr, len)`: never a prefix, an offset or a longer range.
    #[test]
    fn only_the_exact_recorded_range_is_live() {
        let mut live = Allocations::default();
        live.record(64, 16);
        assert!(live.contains(64, 16));
        for (ptr, len) in [(64, 8), (64, 17), (72, 8), (63, 16), (64, 0)] {
            assert!(!live.contains(ptr, len), "({ptr}, {len}) read as live");
        }
    }

    #[test]
    fn len_counts_live_allocations_only() {
        let mut live = Allocations::default();
        assert_eq!(live.len(), 0);
        live.record(1, 4);
        live.record(2, 4);
        assert_eq!(live.len(), 2);
        live.take(1, 4).expect("live");
        assert_eq!(live.len(), 1);
    }
}

#[cfg(any(test, target_arch = "wasm32"))]
mod pointer_layer {
    //! The only part of this module that touches real memory: `gm_alloc` reserves
    //! exactly [`super::ALIGN`]-aligned bytes through the raw global allocator (never
    //! `Vec`, whose byte buffers are only 1-aligned) and hands out the pointer; `gm_free`
    //! reclaims it at the exact [`std::alloc::Layout`] — same size, same alignment —
    //! [`super::Allocations::actual_size`] and [`super::ALIGN`] reconstruct, so the
    //! layout `gm_alloc` reserved is the layout `gm_free` releases.
    use super::{ALIGN, Allocations, ClaimError};
    use crate::errors::{self, Code};
    use std::alloc::{Layout, alloc, dealloc};
    use std::cell::RefCell;

    thread_local! {
        static LIVE: RefCell<Allocations> = RefCell::new(Allocations::default());
    }

    /// The layout `gm_alloc(len)` reserves and `gm_free(ptr, len)` must release: never
    /// zero-sized ([`Allocations::actual_size`]) and always [`ALIGN`]-aligned. `None` only
    /// if `len` is so large the size overflows `isize`, which `gm_alloc` reports as a
    /// failed reservation rather than the layout error going further.
    fn layout(len: u32) -> Option<Layout> {
        Layout::from_size_align(Allocations::actual_size(len), ALIGN).ok()
    }

    /// Reserves `len` bytes, word-aligned, and returns their offset, or `0` if the
    /// reservation fails — never a trap (a null return from the allocator, not an abort:
    /// `std::alloc::alloc`, never `Vec::reserve`/`Box::new`, which abort on failure).
    // SAFETY: `no_mangle` exports this symbol under its Rust name; no other symbol in
    // this module is named `gm_alloc`.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_alloc(len: u32) -> u32 {
        let Some(layout) = layout(len) else {
            errors::set(Code::AllocFailed);
            return 0;
        };
        // SAFETY: `layout` has a non-zero size (`actual_size` is never 0) and a valid
        // power-of-two alignment (`ALIGN` is a `const` power of two), which is exactly
        // what `GlobalAlloc::alloc` requires; a null return is checked immediately below
        // before the pointer is used for anything.
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            errors::set(Code::AllocFailed);
            return 0;
        }
        // SAFETY: `ptr` was just allocated for exactly `layout.size()` bytes and is
        // non-null; zeroing it makes a freshly `gm_alloc`'d buffer's content defined
        // before the caller writes into it, the same guarantee `Vec::resize(_, 0)` gave.
        unsafe { ptr.write_bytes(0, layout.size()) };
        let Ok(addr) = u32::try_from(ptr as usize) else {
            // No live pointer can exceed `u32::MAX` on wasm32 (32-bit address space);
            // reaching here would mean leaking memory the caller could never name back
            // to `gm_free`, so refuse instead of losing track of it.
            // SAFETY: `ptr`/`layout` are exactly what `alloc` just returned/was given.
            unsafe { dealloc(ptr, layout) };
            errors::set(Code::AllocFailed);
            return 0;
        };
        LIVE.with(|live| live.borrow_mut().record(addr, len));
        errors::clear();
        addr
    }

    /// Releases a buffer `gm_alloc` returned. Refuses (leaves memory alone) a
    /// `(ptr, len)` this instance did not just hand out, or already took back — a
    /// double free is refused, never repeated.
    // SAFETY: as `gm_alloc` — `gm_free` is the only symbol with this name.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_free(ptr: u32, len: u32) {
        let claimed = LIVE.with(|live| live.borrow_mut().take(ptr, len));
        match claimed {
            Err(ClaimError::Unknown | ClaimError::LengthMismatch) => {
                errors::set(Code::FreeRefused);
            }
            Ok(()) => {
                // `take` just confirmed `ptr` was recorded at caller-length `len`, and
                // `gm_alloc` always records under the address `layout(len)` reserved, so
                // recomputing the layout from `len` reconstructs exactly what was
                // allocated.
                if let Some(layout) = layout(len) {
                    // SAFETY: `ptr` is the address `gm_alloc` returned for this exact
                    // `layout` (same size, same `ALIGN`), and `take` just removed it from
                    // `LIVE` so no other call can reach this address again — freed once.
                    unsafe { dealloc(ptr as *mut u8, layout) };
                }
                errors::clear();
            }
        }
    }

    /// Whether `(ptr, len)` is a live `gm_alloc` allocation. `gm_build` (`exports.rs`)
    /// validates its source with this before copying out of it, and `gm_build` never
    /// frees it — the caller still owns it (C7).
    pub fn is_live(ptr: u32, len: u32) -> bool {
        LIVE.with(|live| live.borrow().contains(ptr, len))
    }
}

// `gm_alloc`/`gm_free` are not re-exported by name: `#[unsafe(no_mangle)]` alone is what
// makes an `extern "C" fn` a wasm export (the same pattern `gate_exports`'s `gm_topology`
// already relies on), and re-exporting them by their Rust path here would be an import
// this crate itself never uses — exactly the `unused_imports` clippy catches under a
// wasm32, non-test build, where nothing calls them by name.
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) use pointer_layer::is_live;
