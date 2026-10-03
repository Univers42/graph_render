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

/// Slot 0 is how many of the slots *after it* are in use; the marks follow from slot 1 as
/// `(phase, bytes)` pairs. `SLOTS` counts slot 0, so a full table holds `(SLOTS - 2) / 2`
/// marks — 31, which is four times what a 1M-node build records.
const SLOTS: usize = 64;

/// Phase ids, in the order `gm_build` reaches them. The host reads them by number
/// (`harness/ingest-ceiling.mjs` names them), so these numbers are a measurement's ABI:
/// append, never renumber.
pub(crate) const COPY: u32 = 0;
/// The whole document has been validated and the root's members located; no `Value` tree
/// was ever built, which is the phase that used to hold 3.2x the text.
pub(crate) const PARSE: u32 = 1;
/// Every `NodeRecord`/`EdgeRecord` is built.
pub(crate) const RECORDS: u32 = 2;
/// `index_model` has returned: arena, columns and the three CSRs are all live.
pub(crate) const INDEX_MODEL: u32 = 3;
/// `gm_build` is about to return its handle. Recorded by `gm_build` itself, in
/// `crate::exports`, which compiles only under test or for wasm32, so it does too.
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) const RETURNED: u32 = 4;
/// `Topology::strings().byte_len()` — the arena's string data alone.
pub(crate) const ARENA_TEXT: u32 = 5;
/// `Topology::nodes().byte_len()` + `edges().byte_len()`.
pub(crate) const COLUMNS: u32 = 6;
/// The three adjacencies' `offsets` + `values`.
pub(crate) const CSRS: u32 = 7;

/// A mark table: a fixed number of `(phase, bytes)` pairs, written in the order the phases
/// were reached.
///
/// One `u32` slot per value, read and written only through raw pointers, because the whole
/// point is that the table sits at a known address a host can find in `memory.buffer`. That
/// address is a `usize` and not a `u32` because this module is also compiled and unit-tested
/// natively, where a position-independent binary's static lives above 4 GiB; only the wasm
/// build's `gm_probe_base` narrows it, and there `usize` *is* `u32`.
pub(crate) struct Table(UnsafeCell<[u32; SLOTS]>);

// SAFETY: single-threaded by construction — `gm_build` is the only writer and the ABI runs
// it on one thread with no re-entry (C5's caller owns the buffer and cannot re-enter a build
// it has not returned from). Every access is a whole-`u32` store or load, so a reader can
// never observe a half-written pair.
unsafe impl Sync for Table {}

impl Table {
    /// The table's linear-memory address, for `gm_probe_base`.
    pub(crate) fn base(&self) -> usize {
        self.0.get() as usize
    }

    /// Records phase `phase` at `bytes`. A full table drops the rest rather than growing: a
    /// host reading 64 slots has everything a 1M-node build produces, and a silent overflow
    /// would read as a short run.
    pub(crate) fn mark(&self, phase: u32, bytes: usize) {
        let value = u32::try_from(bytes).unwrap_or(u32::MAX);
        // SAFETY: as `Sync for Table` — one thread, and the two stores below are this
        // statement's, to a slot and then the count that publishes it.
        unsafe {
            let table = self.0.get();
            let len = (*table)[0] as usize;
            if len + 3 <= SLOTS {
                (*table)[len + 1] = phase;
                (*table)[len + 2] = value;
                (*table)[0] = (len + 2) as u32;
            }
        }
    }

    /// Clears the table, so one document's phases are never read as another's.
    pub(crate) fn reset(&self) {
        // SAFETY: as `mark`.
        unsafe {
            *self.0.get() = [0; SLOTS];
        }
    }

    /// The table as a slice, for a reader that knows it is alone.
    ///
    /// # Safety
    ///
    /// No other thread may be marking, or the slice changes under it.
    #[cfg(test)]
    pub(crate) unsafe fn read(&self) -> &[u32; SLOTS] {
        // SAFETY: forwarded from this function's own precondition.
        unsafe { &*self.0.get() }
    }
}

static MARKS: Table = Table(UnsafeCell::new([0; SLOTS]));

/// The linear-memory address of the mark table, for `gm_probe_base`. Compiled where the
/// export that calls it is: under test or for wasm32.
#[cfg(any(test, target_arch = "wasm32"))]
pub fn base() -> usize {
    MARKS.base()
}

/// Records phase `phase` at the current linear memory size, or at `bytes` when given.
pub(crate) fn mark(phase: u32, bytes: Option<usize>) {
    MARKS.mark(phase, bytes.unwrap_or_else(linear_bytes));
}

/// Clears the mark table, for `gm_probe_reset`. Compiled where the export that calls it is.
#[cfg(any(test, target_arch = "wasm32"))]
pub fn reset() {
    MARKS.reset();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Each test owns its table, and it is *leaked* on purpose: the whole point of a
    /// `Table` is that its address does not move, and a local `Table` moves when it is
    /// returned or when the optimiser reuses its stack slot — which is exactly the bug this
    /// shape of test is here to catch, so the test must not have it. `gm_build` and every
    /// `read_records` in this binary write `MARKS`, so a test asserting on the static's
    /// contents would be reading it beside a dozen threads doing the same; 256 bytes leaked
    /// per test is the price of not being that.
    fn table() -> &'static Table {
        Box::leak(Box::new(Table(UnsafeCell::new([0; SLOTS]))))
    }

    #[test]
    fn marks_are_readable_at_a_fixed_address_after_a_trap() {
        let marks = table();
        let address = marks.base();
        assert_ne!(address, 0);
        assert_eq!(address, marks.base(), "the address moved between two reads");
        marks.mark(COPY, 11);
        marks.mark(PARSE, 22);
        // SAFETY: this thread is the only one that has touched `marks`.
        let slots = unsafe { marks.read() };
        assert_eq!(&slots[..5], &[4, COPY, 11, PARSE, 22]);
    }

    #[test]
    fn a_full_table_drops_the_rest_instead_of_growing() {
        let marks = table();
        for phase in 0..(SLOTS as u32) {
            marks.mark(phase, phase as usize);
        }
        marks.mark(COPY, 999);
        // SAFETY: as above.
        let slots = unsafe { marks.read() };
        let filled = slots[0] as usize;
        assert!(
            filled + 1 < SLOTS,
            "the table filled to {filled} of {SLOTS}"
        );
        assert!(
            !slots[1..].contains(&999),
            "the dropped mark was written anyway"
        );
        marks.reset();
        // SAFETY: as above.
        assert_eq!(unsafe { marks.read() }[0], 0);
    }

    #[test]
    fn a_linear_mark_reads_the_current_linear_memory() {
        let marks = table();
        marks.mark(COPY, linear_bytes());
        // SAFETY: as above.
        let slots = unsafe { marks.read() };
        assert_eq!(&slots[..3], &[2, COPY, linear_bytes() as u32]);
    }

    #[test]
    fn a_bytes_over_four_gib_is_saturated_rather_than_wrapped() {
        let marks = table();
        marks.mark(ARENA_TEXT, usize::MAX);
        // SAFETY: as above.
        assert_eq!(unsafe { marks.read() }[2], u32::MAX);
    }
}
