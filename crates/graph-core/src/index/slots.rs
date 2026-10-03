//! [`RowBySlot`]: an id's dense row, addressed by the arena slot of its interned id.
//!
//! The id sets were `IndexSet<Interned>`: one hash and one probe into a table as large as the
//! graph for every id admitted and every id looked up. An interned handle already is a small
//! dense integer, so the row sits at that integer in a plain array: one write to admit, one
//! read to look up. At 1M nodes and 2M edges the two sets cost 12% of `gm_build_columns`
//! (`docs/measurements/perf-open-intern.md`).
//!
//! Caveat: the array is as long as the highest slot claimed, not as the rows kept, so every
//! string interned before the last id (labels, groups, the other set's ids) holds 4 unused
//! bytes. A graph of short ids with many distinct long labels pays the most: up to 4 bytes per
//! arena string per set, against the IndexSet's roughly 18 bytes per row.

use crate::arena::Interned;

/// No row at this slot.
const NONE: u32 = u32::MAX;

/// Arena slot → dense row. Rows are below `u32::MAX` (`next_index` refuses that one), so the
/// maximum is free to mean "none".
#[derive(Debug, Clone, Default)]
pub(super) struct RowBySlot(Vec<u32>);

impl RowBySlot {
    /// Room for `rows` rows. A lower bound, never an over-reservation: `rows` distinct
    /// handles reach at least slot `rows - 1`.
    pub(super) fn with_capacity(rows: usize) -> Self {
        Self(Vec::with_capacity(rows))
    }

    /// The row filed under `handle`, if any.
    pub(super) fn row(&self, handle: Interned) -> Option<u32> {
        self.0
            .get(handle.slot())
            .copied()
            .filter(|&row| row != NONE)
    }

    /// Files `row` under `handle`, which must have none yet.
    pub(super) fn file(&mut self, handle: Interned, row: u32) {
        debug_assert!(row != NONE, "a row of u32::MAX reads back as none");
        let slot = handle.slot();
        if slot >= self.0.len() {
            self.0.resize(slot + 1, NONE);
        }
        debug_assert_eq!(self.0[slot], NONE, "a handle filed twice");
        self.0[slot] = row;
    }
}

#[cfg(test)]
mod tests {
    use super::RowBySlot;
    use crate::arena::StringArena;

    #[test]
    fn a_filed_handle_reads_back_and_every_other_reads_none() {
        let mut arena = StringArena::default();
        let [a, b, c] = ["a", "b", "c"].map(|s| arena.intern(s).expect("fits"));
        let mut rows = RowBySlot::default();
        assert_eq!(rows.row(b), None, "past the end of an empty table");
        rows.file(c, 0);
        rows.file(a, 1);
        assert_eq!(
            (rows.row(a), rows.row(b), rows.row(c)),
            (Some(1), None, Some(0))
        );
    }
}
