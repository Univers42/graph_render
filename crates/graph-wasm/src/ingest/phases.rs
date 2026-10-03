//! Per-phase linear-memory checkpoints for the ingest scale measurement
//! (`docs/measurements/fix-ingest-scale.md`), recorded where the phases actually run:
//! inside `gm_build` and inside [`read_records`](super::read_records), not at a call
//! boundary a host can already see.
//!
//! The marks live in a `static` at a **fixed linear-memory address**, published by
//! `gm_probe_base`, because a host has to read them for the runs that matter most: a
//! document that traps inside `index_model` leaves its marks behind, and a wasm instance
//! that hit `unreachable` cannot be called again — only its `memory.buffer` can still be
//! read. A returned `u32` per phase would be lost exactly where the measurement needs it.
//!
//! Compiled out of the default artifact: `#[cfg(any(test, feature = "probe"))]`, and the
//! release artifact the sweep runs is built without `--features probe`.
//!
//! **Ponytail:** wasm32's `memory_size(0)` is the page count of *linear* memory, so a mark
//! is the high-water mark of the whole module, not of this build — a phase that frees a
//! gigabyte and allocates none still reads the same number as its predecessor. Failing
//! input: any build that is not the only thing running in the instance (the studio holds
//! handles and snapshots across calls). Direction: observes, never reserves, never frees.
//! Escape hatch: the per-phase *requested* bytes are measured natively by
//! `crate::memory_measure`'s counting allocator, which sees inside `index_model`.

use std::cell::UnsafeCell;

/// Slot 0 is the mark count; the marks follow as `(phase, bytes)` pairs.
const SLOTS: usize = 64;

/// Phase ids, in the order `gm_build` reaches them. The host reads them by number
/// (`harness/ingest-ceiling.mjs` names them), so these numbers are a measurement's ABI:
/// append, never renumber.
pub(crate) const COPY: u32 = 0;
/// The whole document is a `canonical_json::Value` tree; nothing has been read out of it.
pub(crate) const PARSE: u32 = 1;
/// Every `NodeRecord`/`EdgeRecord` is built and the tree is dropped.
pub(crate) const RECORDS: u32 = 2;
/// `index_model` has returned: arena, columns and the three CSRs are all live.
pub(crate) const INDEX_MODEL: u32 = 3;
/// `gm_build` is about to return its handle.
pub(crate) const RETURNED: u32 = 4;
/// `Topology::strings().byte_len()` — the arena's string data alone.
pub(crate) const ARENA_TEXT: u32 = 5;
/// `Topology::nodes().byte_len()` + `edges().byte_len()`.
pub(crate) const COLUMNS: u32 = 6;
/// The three adjacencies' `offsets` + `values`.
pub(crate) const CSRS: u32 = 7;

struct Marks(UnsafeCell<[u32; SLOTS]>);

// SAFETY: every read and write is a single `u32` through a raw pointer, and the only
// caller is `gm_build`, which the ABI runs on one thread with no re-entry (C5's caller
// owns the buffer and cannot re-enter a build it has not returned from).
unsafe impl Sync for Marks {}

static MARKS: Marks = Marks(UnsafeCell::new([0; SLOTS]));

/// The linear-memory address of the mark table, for `gm_probe_base`.
pub fn base() -> u32 {
    MARKS.0.get() as usize as u32
}

/// Records phase `phase` at the current linear-memory size, or `value` when `bytes` is
/// `None`. A full table drops the rest rather than growing: a host reading 64 slots has
/// everything a 1M-node build produces, and a silent overflow would read as a short run.
pub(crate) fn mark(phase: u32, bytes: Option<usize>) {
    let value = bytes.unwrap_or_else(linear_bytes);
    // SAFETY: as `Sync for Marks` — single-threaded, and both halves of the update are
    // this one statement's stores to disjoint slots of the same table.
    unsafe {
        let table = MARKS.0.get();
        let len = (*table)[0] as usize;
        if len + 2 <= SLOTS {
            (*table)[len + 1] = phase;
            (*table)[len + 2] = value.min(u64::from(u32::MAX) as usize) as u32;
            (*table)[0] = (len + 2) as u32;
        }
    }
}

/// Linear memory in bytes: `memory.size()` pages of 64 KiB. Native runs (the unit tests)
/// have no linear memory, so they read `0` and pin the order and the ids instead.
pub(crate) fn linear_bytes() -> usize {
    #[cfg(target_arch = "wasm32")]
    {
        core::arch::wasm32::memory_size(0) as usize * 65536
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0
    }
}

/// Clears the table, so one document's phases are never read as another's.
pub fn reset() {
    // SAFETY: as `mark`.
    unsafe {
        *MARKS.0.get() = [0; SLOTS];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_are_readable_at_a_fixed_address_after_a_trap() {
        reset();
        let address = base();
        assert_ne!(address, 0);
        assert!(u32::try_from(address).is_ok());
        mark(COPY, Some(11));
        mark(PARSE, Some(22));
        // SAFETY: reading the table this thread just wrote, at the address just published.
        let table = unsafe { std::slice::from_raw_parts(address as *const u32, SLOTS) };
        assert_eq!(&table[..5], &[5, COPY, 11, PARSE, 22]);
    }

    #[test]
    fn a_linear_mark_reads_the_current_linear_memory_and_reset_clears_it() {
        reset();
        mark(COPY, None);
        // SAFETY: as above; one mark was written and one is read.
        let table = unsafe { std::slice::from_raw_parts(base() as *const u32, SLOTS) };
        assert_eq!(table[0], 3);
        assert_eq!(table[1], COPY);
        assert_eq!(table[2], linear_bytes());
        reset();
        // SAFETY: as above, after `reset` wrote the whole table.
        let table = unsafe { std::slice::from_raw_parts(base() as *const u32, SLOTS) };
        assert_eq!(table[0], 0);
    }

    #[test]
    fn a_full_table_drops_the_rest_instead_of_growing() {
        reset();
        for phase in 0..(SLOTS as u32) {
            mark(phase, Some(phase as usize));
        }
        mark(COPY, Some(999));
        // SAFETY: as above.
        let table = unsafe { std::slice::from_raw_parts(base() as *const u32, SLOTS) };
        assert_eq!(table[0], SLOTS as u32 - 1);
        assert!(!table[1..].contains(&999));
        reset();
    }
}
